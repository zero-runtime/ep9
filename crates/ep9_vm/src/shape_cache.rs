//! The shape lookup cache (M14-P1 SHAPE-CACHE; route SHAPE-LOOKUP-CACHE): a per-isolate table
//! from (shape, key) to the answer of the descriptor scan `shape::lookup_with`, the descriptor
//! index or "absent". V8's `DescriptorLookupCache` form: (map, name) -> descriptor number,
//! found and not-found both kept, emptied at a GC.
//!
//! ```text
//!  caller (has rt) ──► lookup(rt, shape, key, how)
//!                        │ seam off, or how = Content ─────────────────► scan (shape::lookup_with)
//!                        │ e = table[index(ShapeId, key)]
//!                        ├─ e.key == key and e.shape == ShapeId: HIT
//!                        │     res = ABSENT ─► None                  (no load of the shape)
//!                        │     res = i      ─► Some(Found { i, details read now from the cell })
//!                        │     debug build: the result == the scan    (the shadow test)
//!                        └─ else MISS ─► r = scan ─► e = (key, ShapeId, r) ─► r
//!  every GC (a weak source of the heap) ─────────────────────────────► every entry empty
//!  a details write in place (shape.rs: set_field_bits_in_family,
//!  descriptors_for_child) ──► flush(rt, cause) ─────────────────────► every entry empty
//! ```
//!
//! Rules:
//! - **What an entry depends on:** the count of the shape, the key at each index below that
//!   count, and which cells a ShapeId and a key address name. Entries below a descriptor
//!   array's length never change (`shape.rs`, the prefix rule); a growth copy keeps the same
//!   first entries; an append past a shape's count is not seen by that shape. So no write of
//!   the shape code changes a cached (found, index) of a live (shape, key).
//! - **The value is the index only.** The details (attributes, field index, representation
//!   and const bits) are read from the DESCRIPTORS cell at every hit: 2 write sites change the
//!   representation and const bits of a live entry in place (`set_field_bits_in_family` and
//!   the re-share in `descriptors_for_child`), and a hit must see the new bits.
//! - **Flushed at every GC** (a weak source, run in every collection, minor and major, after
//!   the mark and before the sweep frees a cell): a freed SHAPE cell, atom or symbol leaves an
//!   address that a new cell can take, so no entry outlives a collection. **And at the 2
//!   details sites** (DL M14-P1-ALLOC-GATE, O2): a second layer, since a hit reads the details
//!   anyway. A later in-place write of a count, or of a key below a length, must call
//!   [`flush`] too; the shadow test fails if it does not.
//! - A flush costs one flag test when nothing was inserted since the last flush, else one
//!   store per entry. A new table is zeroed memory (no store; zero pages until first use).
//! - Only `Probe::Bits` keys (an atom or a symbol) are cached: a hit is then a bit compare, as
//!   the scan's compare is. A `Content` probe takes the scan.
//! - It is a memo of the scan and nothing else: it decides no shape, capacity, transition or
//!   field representation, reads no feedback and no execution mode, holds no Value across a
//!   GC point (the probe and the insert do not allocate; the scan is not a GC point), adds no
//!   root slot and is in no export (facts, layout), as the stub cache (`stub_cache.rs`).
//! - The seam ([`on`], ALLOC-SET design 1.3) is the one switch of the member; the role head
//!   has it always on.
//!
//! Counters (tests, debug builds and feature `ic-stats`; none in a release build): calls,
//! hits (found, absent), misses, `Content` probes, scans, inserts, flushes by cause, shadow
//! checks. With `ic-stats` they print to stderr when the isolate's thread ends:
//!
//! ```text
//! ep9 ic-stats: shape-cache entries=1024 table=16384B calls=N hit=N (found=N absent=N) miss=N content=N off=N scans=N inserts=N
//! ep9 ic-stats: shape-cache flush gc=N/N a1=N/N a2=N/N shadow=N
//! ```
//!
//! (`calls` = hits + scans: every lookup that reached the cache or the scan; a flush column is
//! flushes that emptied entries / flushes asked for.)

use core::cell::Cell;
use core::mem::MaybeUninit;
use std::rc::Rc;

use ep9_heap::Heap;

use crate::key::Probe;
use crate::runtime::Runtime;
use crate::shape::{self, Found};

/// log2 of the number of entries (plan v19 §4.3: 1,024; the SHAPE-LOOKUP-COUNT simulation:
/// 90.3% call hits at 1,024, 82-83% at 256, with no flush).
const BITS: u32 = 10;
/// The number of entries.
pub(crate) const ENTRIES: usize = 1 << BITS;
/// The bytes of one isolate's table: 16 bytes per entry, 4 entries per 64-byte line.
pub(crate) const TABLE_BYTES: usize = ENTRIES * core::mem::size_of::<Entry>();
const _: () = assert!(core::mem::size_of::<Entry>() == 16 && TABLE_BYTES == 16 << 10);

/// The result of an entry for a key that the shape does not have.
const ABSENT: u32 = u32::MAX;

/// One entry. An empty entry has key 0, which no key has (a key is a cell value).
#[repr(C, align(16))]
struct Entry {
    /// The key's Value bits (an atom or a symbol).
    key: Cell<u64>,
    /// The ShapeId: the low 32 bits of the shape value (the shape region starts at the cage
    /// base, which is aligned to `shape::CAGE_ALIGN`).
    shape: Cell<u32>,
    /// The descriptor index, or [`ABSENT`].
    res: Cell<u32>,
}

/// Why the table was emptied (the counters).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cause {
    /// A collection (the weak source).
    Gc = 0,
    /// Site A1: `shape::set_field_bits_in_family` changed a details word in place.
    FieldFamily = 1,
    /// Site A2: `descriptors_for_child` widened the field bits of a re-shared entry in place.
    Reshare = 2,
}

/// The isolate's shape lookup cache (`Runtime::shape_cache`). The heap's weak source shares
/// the table and the flag (both `Rc`).
pub(crate) struct ShapeCache {
    /// A fixed-size array, so the masked index needs no bounds check on the hit path.
    entries: Rc<[Entry; ENTRIES]>,
    /// True if an entry was inserted since the last flush.
    used: Rc<Cell<bool>>,
}

impl ShapeCache {
    /// The empty cache of a new isolate; registers the flush with `heap` (a weak source, which
    /// runs in every collection). Not a GC point.
    pub(crate) fn new(heap: &Heap) -> ShapeCache {
        let all: Rc<[MaybeUninit<Entry>]> = Rc::new_zeroed_slice(ENTRIES);
        // SAFETY: an `Entry` is `Cell<u64>` and two `Cell<u32>` (and padding: none), and the
        // all-zero bit pattern is a valid value of each, so each zeroed element is an
        // initialized empty entry.
        let all: Rc<[Entry]> = unsafe { all.assume_init() };
        let Ok(entries): Result<Rc<[Entry; ENTRIES]>, _> = all.try_into() else {
            unreachable!("ENTRIES entries")
        };
        let used = Rc::new(Cell::new(false));
        let (e, u) = (Rc::clone(&entries), Rc::clone(&used));
        heap.add_weak_source(move |_| empty(&e[..], &u, Cause::Gc));
        ShapeCache { entries, used }
    }

    /// The number of entries in use (tests).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.key.get() != 0).count()
    }
}

/// The seam of the member (ALLOC-SET design 1.3): true if lookups go through the cache. The
/// set branch: the SHAPE-CACHE bit of the arm byte of `rt` (arms A and S; tests: unless
/// [`without`] runs); the set integrator changed only this body.
#[inline(always)]
pub(crate) fn on(rt: &Runtime) -> bool {
    #[cfg(test)]
    if OFF.with(Cell::get) {
        return false;
    }
    crate::set_arm_init::on(rt, ep9_platform::set_arm::SHAPE_CACHE)
}

#[cfg(test)]
thread_local! {
    /// Tests: true while [`without`] runs on this thread.
    static OFF: Cell<bool> = const { Cell::new(false) };
}

/// Tests: runs `f` with the seam off on this thread (every lookup scans), so a test that
/// counts the scans of an IC path (`shape::SCANS`) reads that path, not the cache state.
#[cfg(test)]
pub(crate) fn without<R>(f: impl FnOnce() -> R) -> R {
    let was = OFF.with(|c| c.replace(true));
    let r = f();
    OFF.with(|c| c.set(was));
    r
}

/// Empties every entry if one was inserted since the last flush.
fn empty(entries: &[Entry], used: &Cell<bool>, cause: Cause) {
    let did = used.replace(false);
    #[cfg(any(test, debug_assertions, feature = "ic-stats"))]
    stats::flush(cause, did);
    #[cfg(not(any(test, debug_assertions, feature = "ic-stats")))]
    let _ = cause;
    if !did {
        return;
    }
    for e in entries {
        e.key.set(0);
    }
}

/// Empties the cache of `rt` after a details word changed in place (the sites A1 and A2 of
/// `shape.rs`). One flag test when nothing was inserted since the last flush; no arm test (with
/// the seam off nothing is inserted). Not a GC point.
pub(crate) fn flush(rt: &Runtime, cause: Cause) {
    let c = &rt.shape_cache;
    empty(&c.entries[..], &c.used, cause);
}

/// The entry index of (ShapeId `id`, `key`): the form the SHAPE-LOOKUP-COUNT simulation
/// measured. ShapeIds are 64-byte aligned offsets and keys 8-byte aligned cell addresses.
#[inline(always)]
fn index(id: u32, key: u64) -> usize {
    let h = (u64::from(id) >> 3) ^ key.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29);
    (h as usize) & (ENTRIES - 1)
}

/// `shape::lookup_with(shape, key, how)` through the cache of `rt`: the own property `key` of
/// objects with the fast shape `shape`. Not a GC point.
#[inline(always)]
pub(crate) fn lookup(rt: &Runtime, shape: u64, key: u64, how: Probe) -> Option<Found> {
    if !on(rt) || matches!(how, Probe::Content) {
        #[cfg(any(test, debug_assertions, feature = "ic-stats"))]
        stats::bump(if on(rt) { stats::CONTENT } else { stats::OFF });
        return shape::lookup_with(shape, key, how);
    }
    let id = shape as u32;
    let e = &rt.shape_cache.entries[index(id, key)];
    if e.key.get() == key && e.shape.get() == id {
        let r = e.res.get();
        let hit = if r == ABSENT {
            None
        } else {
            Some(shape::found_at(shape, r as usize))
        };
        #[cfg(any(test, debug_assertions, feature = "ic-stats"))]
        stats::bump(if hit.is_some() {
            stats::FOUND
        } else {
            stats::ABSENT
        });
        #[cfg(debug_assertions)]
        shadow(shape, key, how, hit);
        return hit;
    }
    miss(rt, e, id, shape, key, how)
}

/// A miss: the scan, then the entry for (ShapeId `id`, `key`) holds its answer.
#[inline]
fn miss(rt: &Runtime, e: &Entry, id: u32, shape: u64, key: u64, how: Probe) -> Option<Found> {
    #[cfg(any(test, debug_assertions, feature = "ic-stats"))]
    stats::bump(stats::MISS);
    let r = shape::lookup_with(shape, key, how);
    debug_assert!(key != 0 && id != 0);
    e.key.set(key);
    e.shape.set(id);
    e.res.set(r.map_or(ABSENT, |f| f.index as u32));
    rt.shape_cache.used.set(true);
    r
}

/// The shadow test (debug builds): a hit must equal the scan, index and details.
#[cfg(debug_assertions)]
#[cold]
#[inline(never)]
fn shadow(shape: u64, key: u64, how: Probe, hit: Option<Found>) {
    let scan = shape::scan(shape, key, how);
    assert_eq!(
        hit, scan,
        "shape cache: a hit is not the scan (shape {shape:#x}, key {key:#x})"
    );
    stats::bump(stats::SHADOW);
}

/// The counters (tests, debug builds and feature `ic-stats`), per thread (the isolate's
/// thread). With `ic-stats` they print when the thread ends (module doc).
#[cfg(any(test, debug_assertions, feature = "ic-stats"))]
pub(crate) mod stats {
    use core::cell::Cell;

    use super::Cause;

    pub(crate) const FOUND: usize = 0;
    pub(crate) const ABSENT: usize = 1;
    pub(crate) const MISS: usize = 2;
    pub(crate) const CONTENT: usize = 3;
    pub(crate) const OFF: usize = 4;
    pub(crate) const SCAN: usize = 5;
    pub(crate) const SHADOW: usize = 6;
    /// Flushes asked for, by cause ([`Cause`] as an offset).
    pub(crate) const ASKED: usize = 7;
    /// Flushes that emptied entries, by cause.
    pub(crate) const DONE: usize = 10;
    const N: usize = 13;

    pub(crate) struct Counters {
        n: [Cell<u64>; N],
    }

    #[cfg(feature = "ic-stats")]
    impl Drop for Counters {
        fn drop(&mut self) {
            let n: Vec<u64> = self.n.iter().map(Cell::get).collect();
            if n.iter().all(|&x| x == 0) {
                return;
            }
            let hits = n[FOUND] + n[ABSENT];
            eprintln!(
                "ep9 ic-stats: shape-cache entries={} table={}B calls={} hit={hits} (found={} \
                 absent={}) miss={} content={} off={} scans={} inserts={}",
                super::ENTRIES,
                super::TABLE_BYTES,
                hits + n[SCAN],
                n[FOUND],
                n[ABSENT],
                n[MISS],
                n[CONTENT],
                n[OFF],
                n[SCAN],
                n[MISS],
            );
            eprintln!(
                "ep9 ic-stats: shape-cache flush gc={}/{} a1={}/{} a2={}/{} shadow={}",
                n[DONE],
                n[ASKED],
                n[DONE + 1],
                n[ASKED + 1],
                n[DONE + 2],
                n[ASKED + 2],
                n[SHADOW],
            );
        }
    }

    thread_local! {
        static C: Counters = const { Counters { n: [const { Cell::new(0) }; N] } };
    }

    #[inline]
    pub(crate) fn bump(i: usize) {
        C.with(|c| c.n[i].set(c.n[i].get() + 1));
    }

    pub(crate) fn flush(cause: Cause, did: bool) {
        bump(ASKED + cause as usize);
        if did {
            bump(DONE + cause as usize);
        }
    }

    /// The counter `i` of this thread (tests).
    #[cfg(test)]
    pub(crate) fn get(i: usize) -> u64 {
        C.with(|c| c.n[i].get())
    }
}
