//! S2a part 2: the concurrent sweep (parallel.rs part 2) and the GC-stress control of the
//! parallel paths (route GCPAR-STRESS). Each test turns the concurrent sweep on for its own
//! thread (`test_controls::with_sweeper`) and sets 2 helpers (`with_helpers`), so it does not
//! depend on the host's core count; the other unit tests keep the lazy sweep.
//!
//! | Test | Covers |
//! |---|---|
//! | `the_concurrent_sweep_frees_what_the_lazy_sweep_frees` | S0, S1 major, S1 minor, a shape space: the cells kept (the model), the freed cells and bytes, the pages and the committed bytes equal the lazy sweep's; the sweeper thread swept the pages |
//! | `allocation_takes_the_pages_the_sweeper_swept` | the allocations file the sweeper's pages and reuse them: no new page |
//! | `a_checked_sweep_on_the_sweeper_*` | the verify after a GC with a checked job: no false error, the kept count; a planted fault on a page the sweeper swept fails the verify |
//! | `a_panic_in_a_size_function_on_the_sweeper_reaches_the_heaps_thread` | the panic comes out of `finish_sweeping`; the heap still works |
//! | `a_panic_in_the_join_keeps_the_job_registered` | a panic of the heap's thread in the join leaves the job registered until the sweeper thread is done (Codex r1 GCPAR2 1): the next GC joins it, and the drop waits for it |
//! | `a_heap_dropped_during_a_concurrent_sweep` | the drop waits for the sweeper thread (it never touches the unmapped cage) |
//! | `two_heaps_sweep_concurrently` | two heaps on two threads, one sweeper thread: both keep the right cells |
//! | `the_random_mutators_with_the_concurrent_sweep` | the random mutators of stress.rs (S0 GC-stress, S1 checked GC-stress, S1 budget policy) |
//! | `gc_stress_reaches_the_parallel_mark_and_the_concurrent_sweep` | GCPAR-STRESS: under GC-stress the GCs that `STRESS_PARALLEL_EVERY` picks mark in parallel and sweep concurrently; without GC-stress the same small heap does neither |
//! | `no_concurrent_sweep_*` | 0 helpers; fewer pages than the start threshold |
//! | `field_size_kinds_sweep_as_their_size_functions` | V9 SWEEP-NOFNPTR: kinds of the 3 `FieldSize` forms (int32 slot, word0 flags, masked u32) keep and free the same cells and bytes as the same kinds with a size function, in the lazy and the concurrent sweep; the verify passes |

#![cfg(not(miri))]

use core::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::panic::{AssertUnwindSafe, catch_unwind};

use super::parallel::{Rng, build, graph_heap};
use super::*;
use crate::parallel::{SWEEP_START_PAGES, test_controls};

/// What one GC and its whole sweep left.
#[derive(Debug, PartialEq, Eq)]
struct Left {
    live: Vec<bool>,
    freed_cells: u64,
    freed_bytes: u64,
    small_pages: usize,
    committed_bytes: usize,
}

/// One GC of a random graph of `n` cells and its whole sweep, lazy or concurrent (the sweeper
/// thread sweeps every page: the test waits for it). Checks the cells kept against the model.
fn gc_and_sweep(
    seed: u64,
    n: usize,
    generational: bool,
    minor: bool,
    shapes: bool,
    concurrent: bool,
) -> (Left, u64) {
    let heap = graph_heap(generational, false, shapes);
    let mut rng = Rng(seed);
    let mut g = build(&heap, &mut rng, n, shapes);
    let live = g.reachable();
    let gc = || {
        if minor {
            heap.collect_minor_with_words(&[]);
        } else {
            heap.collect_with_words(&[]);
        }
    };
    let what = format!(
        "seed {seed:#x} gen {generational} minor {minor} shapes {shapes} concurrent {concurrent}"
    );
    if concurrent {
        test_controls::with_helpers(2, || test_controls::with_sweeper(1, false, gc));
        assert!(heap.sweeping_concurrently(), "{what}: no concurrent sweep");
        assert!(
            heap.wait_for_sweeper(),
            "{what}: the sweeper thread did not end in 10 s"
        );
    } else {
        gc();
        assert!(!heap.sweeping_concurrently(), "{what}");
    }
    heap.finish_sweeping();
    assert!(!heap.sweeping_concurrently(), "{what}: the sweep ended");
    g.check(&heap, &live, &what);
    let st = heap.stats();
    let left = Left {
        live: (0..g.cells.len()).map(|i| live.contains(&i)).collect(),
        freed_cells: st.freed_cells,
        freed_bytes: st.freed_bytes,
        small_pages: st.small_pages,
        committed_bytes: st.committed_bytes,
    };
    let pages = heap.sweeper_pages();
    g.drop_roots(&heap);
    (left, pages)
}

#[test]
fn the_concurrent_sweep_frees_what_the_lazy_sweep_frees() {
    let cases = [
        (false, false, false),
        (true, false, false),
        (true, true, false),
        (true, false, true),
    ];
    for (k, &(generational, minor, shapes)) in cases.iter().enumerate() {
        let seed = 0x5EE9_0000 + k as u64;
        let (lazy, none) = gc_and_sweep(seed, 40_000, generational, minor, shapes, false);
        assert_eq!(none, 0);
        let (conc, pages) = gc_and_sweep(seed, 40_000, generational, minor, shapes, true);
        assert_eq!(
            lazy, conc,
            "case {k}: the concurrent sweep left another heap"
        );
        assert!(lazy.freed_cells > 0, "case {k}: the GC freed nothing");
        assert_eq!(
            pages as usize,
            lazy_pages(seed, generational, shapes),
            "case {k}: the sweeper swept every page"
        );
    }
}

/// The small pages of the graph of [`gc_and_sweep`] before its GC.
fn lazy_pages(seed: u64, generational: bool, shapes: bool) -> usize {
    let heap = graph_heap(generational, false, shapes);
    let g = build(&heap, &mut Rng(seed), 40_000, shapes);
    let n = heap.stats().small_pages;
    g.drop_roots(&heap);
    n
}

#[test]
fn allocation_takes_the_pages_the_sweeper_swept() {
    let heap = test_heap(|_| {});
    // 8 pages of 32-byte nodes; one in 1024 stays (a global), so every page is recyclable.
    let mut keep = Vec::new();
    for i in 0..8 * 8192u32 {
        let c = new_node(&heap, i, 2);
        if i % 1024 == 0 {
            keep.push(heap.new_global(c.to_value()));
        }
    }
    let before = heap.stats();
    test_controls::with_helpers(2, || {
        test_controls::with_sweeper(1, false, || heap.collect_with_words(&[]))
    });
    assert!(
        heap.wait_for_sweeper(),
        "the sweeper thread did not end in 10 s"
    );
    let freed = heap.stats().freed_bytes;
    assert!(freed > 1 << 20, "{freed} bytes freed");
    // Garbage of half the freed bytes: the allocations file the sweeper's pages and fill them.
    let mut bytes = 0;
    while bytes < freed / 2 {
        new_node(&heap, 0, 2);
        bytes += 32;
    }
    let after = heap.stats();
    assert_eq!(after.collections, 1);
    assert!(
        heap.sweeper_pages() > 0,
        "the allocations filed the sweeper's pages"
    );
    assert!(
        after.small_pages <= before.small_pages && after.committed_bytes <= before.committed_bytes,
        "the allocations took new pages: {before:?} -> {after:?}"
    );
    heap.finish_sweeping();
    heap.verify().expect("verifier");
    for g in keep {
        heap.drop_global(g);
    }
}

/// The verify after a GC joins the checked job at once; `with_sweeper(_, true)` makes the
/// join wait until the sweeper thread has taken every page, so the sweeper's checked sweep
/// gives the kept count and the errors.
#[test]
fn a_checked_sweep_on_the_sweeper_keeps_the_verify_clean() {
    for generational in [false, true] {
        let heap = graph_heap(generational, true, false);
        let mut g = build(
            &heap,
            &mut Rng(0x5EE9_2000 + u64::from(generational)),
            40_000,
            false,
        );
        let live = g.reachable();
        test_controls::with_helpers(2, || {
            test_controls::with_sweeper(1, true, || heap.collect_with_words(&[]))
        });
        assert!(
            heap.sweeper_pages() > 0,
            "gen {generational}: the sweeper swept no page"
        );
        g.check(&heap, &live, "checked");
        g.drop_roots(&heap);
    }
}

#[test]
fn a_checked_sweep_on_the_sweeper_finds_a_planted_fault() {
    let heap = test_heap(|c| c.verify_after_gc = true);
    // As in verify.rs: a dead node whose size reaches into its live neighbour.
    let dead = new_node(&heap, 1, 0);
    let live = new_node(&heap, 2, 0);
    assert_eq!(live.addr(), dead.addr() + 16);
    let g = heap.new_global(live.to_value());
    // SAFETY: `dead` is a NODE cell that nothing reaches; offset 8 is its meta word.
    unsafe { dead.field::<u64>(8).write((1u64 << 32) | 1) };
    let r = catch_unwind(AssertUnwindSafe(|| {
        test_controls::with_helpers(2, || {
            test_controls::with_sweeper(1, true, || heap.collect_with_words(&[]))
        })
    }));
    let p = r.expect_err("the verify after the GC did not fail");
    let msg = p.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(msg.contains("overlaps the next cell"), "{msg}");
    assert_eq!(heap.sweeper_pages(), 1, "the sweeper thread swept the page");
    // SAFETY: `live` is rooted; its word0 is a header.
    let kind = unsafe { live.header() }.kind();
    assert_eq!(kind, NODE.get(), "the live header was zapped");
    heap.drop_global(g);
}

/// A kind whose size function panics on the sweeper thread while `PANIC_ON_SWEEPER` is set.
const SWEEP_PANICKER: KindId = KindId::new(42).unwrap();
static PANIC_ON_SWEEPER: AtomicBool = AtomicBool::new(false);

fn no_trace(_: CellPtr, _: &mut Tracer<'_>) {}

fn sweep_panicker_size(_: CellPtr) -> usize {
    if PANIC_ON_SWEEPER.load(Relaxed) && std::thread::current().name() == Some("ep9-gc-sweeper") {
        std::panic::resume_unwind(Box::new("test sweeper size panic"));
    }
    24
}

#[test]
fn a_panic_in_a_size_function_on_the_sweeper_reaches_the_heaps_thread() {
    let heap = test_heap(|_| {});
    heap.register_kind(
        SWEEP_PANICKER,
        CellKind {
            name: "test-sweep-panicker",
            size: CellSize::Dynamic(sweep_panicker_size),
            trace: no_trace,
        },
    )
    .unwrap();
    let keep = new_node(&heap, 1, 1);
    let g = heap.new_global(keep.to_value());
    // Dead panicking cells on every page: the sweep reads their size to free them.
    for _ in 0..40_000 {
        heap.alloc(SWEEP_PANICKER, 24).expect("alloc");
    }
    PANIC_ON_SWEEPER.store(true, Relaxed);
    let r = test_controls::with_helpers(2, || {
        test_controls::with_sweeper(1, true, || {
            heap.collect_with_words(&[]);
            assert!(heap.sweeping_concurrently());
            catch_unwind(AssertUnwindSafe(|| heap.finish_sweeping()))
        })
    });
    PANIC_ON_SWEEPER.store(false, Relaxed);
    let p = r.expect_err("the sweeper's panic did not reach the heap's thread");
    assert_eq!(p.downcast_ref::<&str>(), Some(&"test sweeper size panic"));
    assert!(!heap.sweeping_concurrently());
    // The heap still works: the next GC keeps the rooted node and frees the rest.
    heap.collect_with_words(&[]);
    assert!(heap.is_cell(keep.addr()));
    heap.verify().expect("verifier");
    heap.drop_global(g);
}

/// A kind whose size function panics on the heap's thread while `PANIC_ON_HEAP_THREAD` is set,
/// and is slow on the sweeper thread (20 µs per call), so the sweeper thread is still busy
/// with its page when the join on the heap's thread panics.
const JOIN_PANICKER: KindId = KindId::new(43).unwrap();
static PANIC_ON_HEAP_THREAD: AtomicBool = AtomicBool::new(false);

fn join_panicker_size(_: CellPtr) -> usize {
    if PANIC_ON_HEAP_THREAD.load(Relaxed) {
        if std::thread::current().name() == Some("ep9-gc-sweeper") {
            std::thread::sleep(std::time::Duration::from_micros(20));
        } else {
            std::panic::resume_unwind(Box::new("test join size panic"));
        }
    }
    24
}

#[test]
fn a_panic_in_the_join_keeps_the_job_registered() {
    let heap = test_heap(|_| {});
    heap.register_kind(
        JOIN_PANICKER,
        CellKind {
            name: "test-join-panicker",
            size: CellSize::Dynamic(join_panicker_size),
            trace: no_trace,
        },
    )
    .unwrap();
    let keep = new_node(&heap, 1, 1);
    let g = heap.new_global(keep.to_value());
    // Dead panicking cells on every page: the sweep reads their size to free them.
    for _ in 0..40_000 {
        heap.alloc(JOIN_PANICKER, 24).expect("alloc");
    }
    PANIC_ON_HEAP_THREAD.store(true, Relaxed);
    let r = test_controls::with_helpers(2, || {
        test_controls::with_sweeper(1, false, || {
            heap.collect_with_words(&[]);
            assert!(heap.sweeping_concurrently());
            catch_unwind(AssertUnwindSafe(|| heap.finish_sweeping()))
        })
    });
    let p = r.expect_err("the join on the heap's thread did not panic");
    assert_eq!(p.downcast_ref::<&str>(), Some(&"test join size panic"));
    // The sweeper thread may still sweep a page of the job: the job stays registered, so the
    // next join and the heap's drop wait for it.
    let registered = heap.sweeping_concurrently();
    PANIC_ON_HEAP_THREAD.store(false, Relaxed);
    assert!(registered, "the job left the heap before the join ended");
    // The heap still works: the next GC joins the job, keeps the rooted node and frees the
    // rest.
    heap.collect_with_words(&[]);
    heap.finish_sweeping();
    assert!(!heap.sweeping_concurrently());
    assert!(heap.is_cell(keep.addr()));
    heap.verify().expect("verifier");
    heap.drop_global(g);
}

#[test]
fn a_heap_dropped_during_a_concurrent_sweep() {
    for round in 0..20u64 {
        let heap = graph_heap(round % 2 == 1, false, false);
        let g = build(&heap, &mut Rng(0x5EE9_3000 + round), 40_000, false);
        test_controls::with_helpers(2, || {
            test_controls::with_sweeper(1, false, || heap.collect_with_words(&[]))
        });
        assert!(heap.sweeping_concurrently());
        g.drop_roots(&heap);
        // The drop closes the job and waits for the sweeper thread's page.
        drop(heap);
    }
    // The sweeper thread still works.
    let (_, pages) = gc_and_sweep(0x5EE9_3100, 20_000, false, false, false, true);
    assert!(pages > 0);
}

#[test]
fn two_heaps_sweep_concurrently() {
    let run = |seed: u64| {
        move || {
            for round in 0..4u64 {
                let (left, pages) =
                    gc_and_sweep(seed + round, 30_000, round % 2 == 0, false, false, true);
                assert!(
                    pages > 0 && left.freed_cells > 0,
                    "seed {seed:#x} round {round}"
                );
            }
        }
    };
    let a = std::thread::spawn(run(0x5EE9_4000));
    let b = std::thread::spawn(run(0x5EE9_5000));
    a.join().expect("heap A");
    b.join().expect("heap B");
}

#[test]
fn the_random_mutators_with_the_concurrent_sweep() {
    test_controls::with_helpers(2, || {
        test_controls::with_sweeper(1, false, || {
            super::stress::run_with(0x5EE9_6000, 1500, 3, false);
            super::stress::run_with(0x5EE9_6001, 1500, 1, true);
            super::stress::run_with(0x5EE9_6002, 6000, 0, true);
        })
    });
}

/// A small generational heap (2,000 rooted nodes, under `SWEEP_START_PAGES` pages and over
/// `STRESS_START_AFTER` cells): the parallel drains and the concurrent sweeps that `n`
/// allocations start, with GC-stress 1 (every allocation collects) or without it (a full GC
/// every 50 allocations). `sweep_from`: the sweep start threshold in pages.
fn small_heap_parallel_paths(gc_stress: bool, n: u32, sweep_from: usize) -> (u64, u64) {
    let heap = test_heap(|c| {
        c.generational = true;
        c.verify_after_gc = true;
    });
    let head = new_node(&heap, 0, 1);
    let g = heap.new_global(head.to_value());
    let mut tail = head;
    for i in 1..2_000 {
        let c = new_node(&heap, i, 1);
        store(&heap, tail, 0, c.to_value());
        tail = c;
    }
    assert!(heap.stats().small_pages < SWEEP_START_PAGES);
    if gc_stress {
        heap.set_gc_stress(1);
    }
    let drains = test_controls::PARALLEL_DRAINS.get();
    let jobs = heap.concurrent_jobs();
    test_controls::with_helpers(2, || {
        test_controls::with_sweeper(sweep_from, false, || {
            for i in 0..n {
                new_node(&heap, 2_000 + i, 0);
                if !gc_stress && i % 50 == 0 {
                    heap.collect();
                }
            }
        })
    });
    heap.set_gc_stress(0);
    let out = (
        test_controls::PARALLEL_DRAINS.get() - drains,
        heap.concurrent_jobs() - jobs,
    );
    heap.verify().expect("verifier");
    heap.drop_global(g);
    out
}

/// Route GCPAR-STRESS: GC-stress marks never reached the 64 K-cell start threshold, so the
/// GC-stress runs (CI test262-gc) did not run the parallel mark. Now every
/// `STRESS_PARALLEL_EVERY`th stress GC (the first one too) uses both parallel paths.
#[test]
fn gc_stress_reaches_the_parallel_mark_and_the_concurrent_sweep() {
    let n = 3 * crate::parallel::STRESS_PARALLEL_EVERY;
    let (drains, jobs) = small_heap_parallel_paths(true, n, SWEEP_START_PAGES);
    assert!(drains >= 3, "{drains} parallel drains in {n} GC-stress GCs");
    assert!(jobs >= 3, "{jobs} concurrent sweeps in {n} GC-stress GCs");
    let (drains, jobs) = small_heap_parallel_paths(false, n, SWEEP_START_PAGES);
    assert_eq!((drains, jobs), (0, 0), "a small heap without GC-stress");
}

/// The other GC-stress GCs sweep lazily, also over the start threshold: their verify joins a
/// job at once, so a job in each of them only costs a wake per allocation. The picked GCs of
/// 192: the first one and the majors at 64, 128 and 192.
#[test]
fn gc_stress_sweeps_concurrently_only_in_the_picked_gcs() {
    let n = 3 * crate::parallel::STRESS_PARALLEL_EVERY;
    let (_, jobs) = small_heap_parallel_paths(true, n, 1);
    assert!(
        (3..=4).contains(&jobs),
        "{jobs} concurrent sweeps in {n} GC-stress GCs (from 1 page)"
    );
}

#[test]
fn no_concurrent_sweep_with_0_helpers() {
    let heap = graph_heap(false, false, false);
    let g = build(&heap, &mut Rng(0x5EE9_7000), 40_000, false);
    test_controls::with_helpers(0, || {
        test_controls::with_sweeper(1, false, || heap.collect_with_words(&[]))
    });
    assert!(
        !heap.sweeping_concurrently(),
        "EP9_GC_HELPERS=0 is the lazy sweep"
    );
    assert!(heap.unswept_pages() > 0);
    g.drop_roots(&heap);
}

#[test]
fn no_concurrent_sweep_below_the_start_threshold() {
    let heap = graph_heap(false, false, false);
    let g = build(&heap, &mut Rng(0x5EE9_8000), 2_000, false);
    assert!(heap.stats().small_pages < SWEEP_START_PAGES);
    test_controls::with_helpers(2, || {
        test_controls::with_sweeper(SWEEP_START_PAGES, false, || heap.collect_with_words(&[]))
    });
    assert!(!heap.sweeping_concurrently(), "a small heap sweeps lazily");
    g.drop_roots(&heap);
}

/// The kinds of [`field_size_kinds_sweep_as_their_size_functions`]: `n` in an int32 slot at
/// +8, in the word0 flags (low 8 bits), or in the u32 at +8 (low 31 bits).
const FIELD_INT32: KindId = KindId::new(50).unwrap();
const FIELD_FLAGS: KindId = KindId::new(51).unwrap();
const FIELD_WORD32: KindId = KindId::new(52).unwrap();

fn int32_n_size(c: CellPtr) -> usize {
    // SAFETY: the heap calls size functions with live cells of the kind; +8 is a slot.
    16 + 8 * unsafe { c.slot(8) }.bits() as u32 as i32 as usize
}

fn flags_n_size(c: CellPtr) -> usize {
    // SAFETY: as above.
    16 + 8 * usize::from(unsafe { c.header() }.flags() & 0x00FF)
}

fn word32_n_size(c: CellPtr) -> usize {
    // SAFETY: as above; +8 is inside the 16-byte minimum.
    16 + 8 * (unsafe { c.field::<u32>(8).read() } & 0x7FFF_FFFF) as usize
}

/// A heap with the 3 test kinds: of the field forms if `fields`, else of the size functions.
fn field_heap(fields: bool) -> Heap {
    use crate::FieldSize;
    let heap = test_heap(|_| {});
    let kinds: [(KindId, &str, CellSize, CellSize); 3] = [
        (
            FIELD_INT32,
            "test-field-int32",
            CellSize::Field(FieldSize::int32(8, 16, 8)),
            CellSize::Dynamic(int32_n_size),
        ),
        (
            FIELD_FLAGS,
            "test-field-flags",
            CellSize::Field(FieldSize::flags(16, 8, 0x00FF)),
            CellSize::Dynamic(flags_n_size),
        ),
        (
            FIELD_WORD32,
            "test-field-word32",
            CellSize::Field(FieldSize::word32(8, 0x7FFF_FFFF, 16, 8)),
            CellSize::Dynamic(word32_n_size),
        ),
    ];
    for (id, name, field, function) in kinds {
        let size = if fields { field } else { function };
        heap.register_kind(
            id,
            CellKind {
                name,
                size,
                trace: no_trace,
            },
        )
        .unwrap();
    }
    heap
}

/// Allocates `cells` cells of the 3 kinds with `n` in 0..40 (16 to 328 bytes, so many cross
/// a 256-byte line), with other bits set beside each field, roots one in 7, collects once
/// (lazy or concurrent) and sweeps every page. Returns what was left and the sum of the
/// rounded sizes of the cells it did not root.
fn field_gc(fields: bool, concurrent: bool, cells: u32) -> (Left, u64) {
    let heap = field_heap(fields);
    let mut rng = Rng(0x5EE9_F1E1);
    let mut roots = Vec::new();
    let mut live = Vec::new();
    let mut dead_bytes = 0u64;
    for i in 0..cells {
        let n = rng.below(40) as u32;
        let kind = [FIELD_INT32, FIELD_FLAGS, FIELD_WORD32][rng.below(3)];
        let size = 16 + 8 * n as usize;
        let c = heap.alloc(kind, size).expect("alloc");
        // SAFETY: `c` is a new cell of `size` bytes (at least 16); the writes are its own
        // fields, before the next GC point.
        unsafe {
            if kind == FIELD_INT32 {
                c.field::<u64>(8).write(Value::int32(n as i32).to_bits());
            } else if kind == FIELD_FLAGS {
                c.header().set_flags(0xA500 | n as u16);
            } else {
                c.field::<u32>(8).write(n | 1 << 31);
                c.field::<u32>(12).write(0xFFFF_FFFF);
            }
        }
        if i % 7 == 0 {
            roots.push(heap.new_global(c.to_value()));
            live.push(true);
        } else {
            dead_bytes += size as u64;
            live.push(false);
        }
    }
    let before = heap.stats();
    let gc = || heap.collect_with_words(&[]);
    if concurrent {
        test_controls::with_helpers(2, || test_controls::with_sweeper(1, false, gc));
        assert!(heap.sweeping_concurrently(), "no concurrent sweep");
        assert!(
            heap.wait_for_sweeper(),
            "the sweeper thread did not end in 10 s"
        );
    } else {
        gc();
        assert!(!heap.sweeping_concurrently());
    }
    heap.finish_sweeping();
    heap.verify().expect("verifier");
    for &g in &roots {
        let v = heap.global(g);
        assert!(heap.is_cell(CellPtr::from_value(v).expect("a cell").addr()));
    }
    let st = heap.stats();
    let left = Left {
        live,
        freed_cells: st.freed_cells - before.freed_cells,
        freed_bytes: st.freed_bytes - before.freed_bytes,
        small_pages: st.small_pages,
        committed_bytes: st.committed_bytes,
    };
    for g in roots {
        heap.drop_global(g);
    }
    (left, dead_bytes)
}

#[test]
fn field_size_kinds_sweep_as_their_size_functions() {
    let cells = 40_000;
    for concurrent in [false, true] {
        let (by_fields, dead_bytes) = field_gc(true, concurrent, cells);
        let (by_functions, _) = field_gc(false, concurrent, cells);
        assert_eq!(
            by_fields, by_functions,
            "concurrent {concurrent}: the field forms left another heap"
        );
        assert_eq!(
            by_fields.freed_bytes, dead_bytes,
            "concurrent {concurrent}: the freed bytes are the sizes of the dead cells"
        );
        let dead = by_fields.live.iter().filter(|&&l| !l).count() as u64;
        assert_eq!(by_fields.freed_cells, dead, "concurrent {concurrent}");
    }
}

// ---- SWEEP-LEAN (M14-P1; ALLOC-SET design 1.3; plan v19 4.3) --------------------------------
//
// | Test | Covers |
// |---|---|
// | `every_sweep_bitmap_body_equals_the_scalar_body` | each SWEEP body this CPU has (called by its function), on random pages, both keep values: the same bitmaps, line marks, line ends and counts as the scalar body; the scalar body against a bit-by-bit model |
// | `the_lean_sweep_with_each_body_leaves_the_free_lists_of_the_walk` | S0, S1 major, S1 minor: the lazy sweep with today's walk, then lean with each body forced: the same cells kept, freed cells, pages and committed bytes, the same addresses for the next allocations (the free lists), no `freed_bytes` from small cells, and a verify (with the shadow pass) passes |
// | `the_checked_sweep_compares_the_body_on_every_page` | the verify after a GC with each body forced: the shadow pass runs and finds no difference; a wrong body result is an error |
// | `sweep_simd_report` | the SWEEP pick follows `EP9_SIMD` (a forced body that the CPU has is taken) and prints "SIMD-REPORT-SWEEP" |

use crate::heap::sweep_lean_test;
use ep9_platform::simd::{SWEEP_LINES, SWEEP_WORDS, SweepBody, SweepCounts, sweep_scalar};

/// The names that `EP9_REQUIRE_SIMD_PATHS` lists (the x86 box: a missing body fails).
fn required_paths() -> Vec<String> {
    std::env::var("EP9_REQUIRE_SIMD_PATHS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The SWEEP bodies this CPU and build have, scalar first. A body that
/// `EP9_REQUIRE_SIMD_PATHS` names must be there.
fn sweep_bodies() -> Vec<SweepBody> {
    let have: Vec<SweepBody> = SweepBody::ALL
        .into_iter()
        .filter(|b| b.bitmaps().is_some())
        .collect();
    for name in required_paths() {
        if let Some(b) = SweepBody::ALL.into_iter().find(|b| b.name() == name) {
            assert!(
                have.contains(&b),
                "EP9_REQUIRE_SIMD_PATHS names sweep body {name}: missing"
            );
        }
    }
    assert_eq!(have[0], SweepBody::Scalar);
    if cfg!(target_arch = "aarch64") {
        assert!(
            have.contains(&SweepBody::Neon),
            "aarch64 without the NEON sweep body"
        );
    }
    if cfg!(target_arch = "x86_64") {
        assert!(
            have.contains(&SweepBody::Sse2),
            "x86-64 without the SSE2 sweep body"
        );
    }
    have
}

/// One random word of a page bitmap: empty, one half only, sparse, dense or full.
fn random_word(rng: &mut Rng) -> u64 {
    let kind = rng.below(8);
    let mut bits = |n: usize| (0..n).fold(0u64, |w, _| w | 1 << rng.below(64));
    match kind {
        0 | 1 => 0,
        2 => bits(1),
        3 => bits(3) & 0xffff_ffff,
        4 => bits(3) & !0xffff_ffff,
        5 => bits(12),
        6 => bits(48),
        _ => u64::MAX,
    }
}

/// A random page: VO words, mark words (mostly inside VO; `strays`: some outside it).
fn random_page(rng: &mut Rng, strays: bool) -> (Vec<u64>, Vec<u64>) {
    let vo: Vec<u64> = (0..SWEEP_WORDS).map(|_| random_word(rng)).collect();
    let mk = vo
        .iter()
        .map(|&v| {
            let m = v & random_word(rng);
            if strays && rng.below(16) == 0 {
                m | random_word(rng)
            } else {
                m
            }
        })
        .collect();
    (vo, mk)
}

/// What one body left on a copy of a page.
#[derive(Debug, PartialEq, Eq)]
struct BodyOut {
    vo: Vec<u64>,
    mk: Vec<u64>,
    lines: Vec<u8>,
    ends: Vec<u8>,
    counts: SweepCounts,
}

fn run_body(body: SweepBody, vo: &[u64], mk: &[u64], keep: u64) -> BodyOut {
    let f = body.bitmaps().expect("a body this CPU has");
    let (mut vo, mut mk) = (vo.to_vec(), mk.to_vec());
    let (mut lines, mut ends) = (vec![7u8; SWEEP_LINES], vec![7u8; SWEEP_LINES]);
    // SAFETY: 4 separate vectors of the sizes of the `SweepBitmaps` contract; the body came
    // from `SweepBody::bitmaps`.
    let counts = unsafe {
        f(
            vo.as_mut_ptr(),
            mk.as_mut_ptr(),
            keep,
            lines.as_mut_ptr(),
            ends.as_mut_ptr(),
        )
    };
    BodyOut {
        vo,
        mk,
        lines,
        ends,
        counts,
    }
}

/// The pass of `ep9_platform::simd` (SWEEP section) bit by bit.
fn model(vo: &[u64], mk: &[u64], keep: u64) -> BodyOut {
    let mut out = BodyOut {
        vo: vec![0; SWEEP_WORDS],
        mk: mk.to_vec(),
        lines: vec![0; SWEEP_LINES],
        ends: vec![0; SWEEP_LINES],
        counts: SweepCounts::default(),
    };
    for w in 0..SWEEP_WORDS {
        let (v, m) = (vo[w], mk[w]);
        for bit in 0..64 {
            let (vb, mb) = (v >> bit & 1 != 0, m >> bit & 1 != 0);
            out.counts.live += u32::from(vb && mb);
            out.counts.dead += u32::from(vb && !mb);
        }
        for h in 0..2 {
            let has = |x: u64, b: usize| (x >> (32 * h + b)) & 1 != 0;
            let live: Vec<usize> = (0..32).filter(|&b| has(v & m, b)).collect();
            let starts: Vec<usize> = (0..32).filter(|&b| has(v, b)).collect();
            out.lines[2 * w + h] = u8::from(!live.is_empty());
            out.ends[2 * w + h] = u8::from(!live.is_empty() && live.last() == starts.last());
        }
        out.vo[w] = v & m;
        if v != 0 {
            out.mk[w] = v & m & keep;
        }
    }
    out
}

#[test]
fn every_sweep_bitmap_body_equals_the_scalar_body() {
    let bodies = sweep_bodies();
    let mut rng = Rng(0x5EE9_1EA4);
    let mut pages = 0;
    for case in 0..300 {
        let (vo, mk) = random_page(&mut rng, case % 3 == 0);
        for keep in [0, u64::MAX] {
            let want = run_body(SweepBody::Scalar, &vo, &mk, keep);
            if case < 60 {
                assert_eq!(
                    want,
                    model(&vo, &mk, keep),
                    "case {case} keep {keep:#x}: scalar vs the model"
                );
            }
            for &b in &bodies[1..] {
                assert_eq!(
                    run_body(b, &vo, &mk, keep),
                    want,
                    "case {case} keep {keep:#x}: {}",
                    b.name()
                );
            }
            pages += 1;
        }
    }
    // The edge pages: all empty, all full, and every word with only its top or bottom bit.
    for (vo, mk) in [
        (vec![0; SWEEP_WORDS], vec![0; SWEEP_WORDS]),
        (vec![u64::MAX; SWEEP_WORDS], vec![u64::MAX; SWEEP_WORDS]),
        (vec![u64::MAX; SWEEP_WORDS], vec![0; SWEEP_WORDS]),
        (
            vec![1 << 63 | 1 << 31; SWEEP_WORDS],
            vec![1 << 31; SWEEP_WORDS],
        ),
        (vec![1 | 1 << 32; SWEEP_WORDS], vec![1 << 32; SWEEP_WORDS]),
    ] {
        for keep in [0, u64::MAX] {
            let want = run_body(SweepBody::Scalar, &vo, &mk, keep);
            assert_eq!(
                want,
                model(&vo, &mk, keep),
                "edge page: scalar vs the model"
            );
            for &b in &bodies[1..] {
                assert_eq!(run_body(b, &vo, &mk, keep), want, "edge page: {}", b.name());
            }
            pages += 1;
        }
    }
    let names: Vec<&str> = bodies.iter().map(|b| b.name()).collect();
    eprintln!("SWEEP-BODIES {} pages {pages} each", names.join(","));
}

/// What one GC and its whole lazy sweep left, and where the next allocations went.
#[derive(Debug, PartialEq, Eq)]
struct LeanLeft {
    live: Vec<bool>,
    freed_cells: u64,
    small_pages: usize,
    committed_bytes: usize,
    /// The addresses of the next allocations (from the first graph cell): the free lists the
    /// sweep left.
    next: Vec<usize>,
}

/// One GC of a random graph of `n` cells on a heap with no zap, its lazy sweep, then 3000
/// allocations (no GC among them), with the lean walk off (`body` None: today's walk) or on
/// with `body` forced. Returns what it left and the small-cell bytes it freed.
fn lean_gc(
    seed: u64,
    n: usize,
    generational: bool,
    minor: bool,
    body: Option<SweepBody>,
) -> (LeanLeft, u64) {
    sweep_lean_test::with(body.is_some(), body, || {
        let heap = test_heap(|c| {
            c.zap = false;
            c.generational = generational;
        });
        let mut rng = Rng(seed);
        let mut g = build(&heap, &mut rng, n, false);
        let live = g.reachable();
        let before = heap.stats();
        if minor {
            heap.collect_minor_with_words(&[]);
        } else {
            heap.collect_with_words(&[]);
        }
        heap.finish_sweeping();
        let what = format!("seed {seed:#x} gen {generational} minor {minor} body {body:?}");
        g.check(&heap, &live, &what);
        let st = heap.stats();
        // Each heap has its own cage: the addresses are taken from the first cell of the graph.
        let origin = g.cells[0].addr();
        let next = (0..3000u32)
            .map(|i| {
                new_node(&heap, i, (i % 9) as usize)
                    .addr()
                    .wrapping_sub(origin)
            })
            .collect();
        assert_eq!(
            heap.stats().collections,
            st.collections,
            "{what}: a GC among the allocations"
        );
        // A checked GC: the verify and its shadow pass of the body find nothing.
        heap.set_verify_after_gc(true);
        heap.collect_with_words(&[]);
        let left = LeanLeft {
            live: (0..g.cells.len()).map(|i| live.contains(&i)).collect(),
            freed_cells: st.freed_cells - before.freed_cells,
            small_pages: st.small_pages,
            committed_bytes: st.committed_bytes,
            next,
        };
        g.drop_roots(&heap);
        (left, st.freed_bytes - before.freed_bytes)
    })
}

#[test]
fn the_lean_sweep_with_each_body_leaves_the_free_lists_of_the_walk() {
    let bodies = sweep_bodies();
    let cases = [(false, false), (true, false), (true, true)];
    for (k, &(generational, minor)) in cases.iter().enumerate() {
        let seed = 0x5EE9_2000 + k as u64;
        let (walk, walk_bytes) = lean_gc(seed, 30_000, generational, minor, None);
        assert!(
            walk.freed_cells > 0 && walk_bytes > 0,
            "case {k}: the GC freed nothing"
        );
        for &b in &bodies {
            let (lean, lean_bytes) = lean_gc(seed, 30_000, generational, minor, Some(b));
            let name = b.name();
            assert_eq!(
                lean.live, walk.live,
                "case {k}: body {name}: other cells kept"
            );
            assert_eq!(
                (lean.freed_cells, lean.small_pages, lean.committed_bytes),
                (walk.freed_cells, walk.small_pages, walk.committed_bytes),
                "case {k}: body {name}: freed cells, pages, committed bytes"
            );
            let first = (0..walk.next.len()).find(|&i| lean.next[i] != walk.next[i]);
            assert_eq!(
                first, None,
                "case {k}: body {name}: the first allocation at another address"
            );
            assert_eq!(lean, walk, "case {k}: body {name}");
            // The lean walk reads no dead header, so the small cells add no bytes (the graph
            // has no large object).
            assert_eq!(lean_bytes, 0, "case {k}: body {}", b.name());
        }
    }
}

/// A wrong body for the shadow test: the scalar pass, then the line of the first live start
/// is marked free.
///
/// # Safety
///
/// The contract of `SweepBitmaps`.
unsafe fn wrong_body(
    vo: *mut u64,
    mk: *mut u64,
    keep: u64,
    lines: *mut u8,
    ends: *mut u8,
) -> SweepCounts {
    // SAFETY: the caller's contract.
    unsafe {
        let c = sweep_scalar(vo, mk, keep, lines, ends);
        if let Some(l) = (0..SWEEP_LINES).find(|&l| lines.add(l).read() != 0) {
            lines.add(l).write(0);
        }
        c
    }
}

/// One checked heap: a random graph, a major and a minor GC (each verified, with the shadow
/// pass on every page), then a verify. Returns the verify result and the shadow passes.
fn checked_gcs() -> (Result<(), String>, u64) {
    let shadows = sweep_lean_test::shadows();
    let heap = graph_heap(true, true, false);
    let mut rng = Rng(0x5EE9_3000);
    let mut g = build(&heap, &mut rng, 20_000, false);
    let live = g.reachable();
    let gcs = catch_unwind(AssertUnwindSafe(|| {
        heap.collect_with_words(&[]);
        heap.collect_minor_with_words(&[]);
    }));
    let r = match gcs {
        Ok(()) => {
            g.check(&heap, &live, "checked");
            heap.verify().map(|_| ()).map_err(|e| e.to_string())
        }
        Err(p) => Err(p
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_else(|| "a panic".to_owned())),
    };
    g.drop_roots(&heap);
    (r, sweep_lean_test::shadows() - shadows)
}

#[test]
fn the_checked_sweep_compares_the_body_on_every_page() {
    for b in sweep_bodies() {
        let (r, shadows) = sweep_lean_test::with(true, Some(b), checked_gcs);
        assert_eq!(r, Ok(()), "body {}", b.name());
        assert!(shadows >= 3, "body {}: {shadows} shadow passes", b.name());
    }
    let (r, _) = sweep_lean_test::with_fn(wrong_body, checked_gcs);
    let e = r.expect_err("a wrong body passes the checked sweep");
    assert!(e.contains("another line mark"), "{e}");
}

/// The SWEEP pick of the process follows `EP9_SIMD` (`sweep=<body>`, or a bare body name):
/// a forced body that this CPU has is the pick. Prints the report line.
#[test]
fn sweep_simd_report() {
    let line = ep9_platform::simd::sweep_report();
    eprintln!("{line}");
    assert!(line.starts_with(&format!(
        "SIMD-REPORT-SWEEP target={} sweep=",
        std::env::consts::ARCH
    )));
    let forced = std::env::var("EP9_SIMD").ok().and_then(|s| {
        s.split(',')
            .map(str::trim)
            .find_map(|i| match i.split_once('=') {
                Some(("sweep", b)) => Some(b.trim().to_owned()),
                Some(_) => None,
                None => SweepBody::ALL
                    .iter()
                    .any(|b| b.name() == i)
                    .then(|| i.to_owned()),
            })
    });
    if let Some(name) = forced
        && let Some(b) = SweepBody::ALL.into_iter().find(|b| b.name() == name)
        && b.bitmaps().is_some()
    {
        assert_eq!(
            ep9_platform::simd::sweep(),
            b,
            "EP9_SIMD forces sweep={name}"
        );
    }
}
