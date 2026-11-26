//! Slack (owner M5-P2 SLACK; the H17 "Slack tracking" row): the in-object capacity of the
//! objects that `new F` makes.
//!
//! ```text
//!  call::ordinary_create_from_constructor(F)
//!    proto = F.prototype ──> instance_capacity(F, proto)
//!                              Some(n) ─> object::new_object_with_capacity(proto, n)  root (proto, OBJECT, n)
//!                              None    ─> object::new_object(proto)                   root (proto, OBJECT, GENEROUS)
//!    n = the distinct names of F's own `this.<name> =` stores          (scan of F's code)
//!      + the data properties of proto whose value is not callable (the instances' likely
//!        shadows: `F.prototype.left = null`) and that F does not store, at most GENEROUS
//!    chosen at F's first construct and kept in F's FunctionInfo (`slack_capacity`)
//!  construct_ic::miss caches the root shape of that first instance; a hit allocates from it
//! ```
//!
//! Rules:
//! - One capacity per constructor (FunctionInfo), chosen at its first construct from its
//!   code and the prototype it had then: every instance of `new F` starts from the same root
//!   shape, so the ICs see one shape family per constructor, as before. A prototype that
//!   gains a data field later does not change it.
//! - The count is read from the bytecode, so no instance runs first with another capacity.
//!   A run-time tracker that sizes the later instances after the first N (V8 slack tracking)
//!   would start a second root shape family: measured in M5-P2, the T0 ICs then see two
//!   shapes per constructor, which cost more than the smaller objects saved (richards +12.7%,
//!   deltablue +9.1% instructions). V8 shrinks its map tree in place instead; here the
//!   capacity lives in each shape's `counts` word (`shape.rs`), and a live object never
//!   shrinks (H21 P5).
//! - [`None`] (the generous capacity, today's 8) when the count is unknown: `this` leaves
//!   the frame (a call or a store that reads it, a closure, a context slot, a direct eval),
//!   a keyed store, a derived constructor, no store at all, or more names than [`MAX_NAMES`].
//!   A field added after the constructor returns goes out of line (props.rs SLOTS), as a
//!   ninth field does today.
//! - The count is a size hint, never a correctness input: any capacity is a right one. A
//!   linear scan does not follow jumps, so a register that holds `this` on one path only may
//!   count as holding it, or not.
//! - It reads no execution policy (rule 40, M-R9): the bytecode and the prototype only, so
//!   every preset and both feedback modes make the same shapes (`tests::slack`).
//! - The FunctionInfo keeps the chosen capacity (one byte), so a later cold construct (a
//!   megamorphic site, `Reflect.construct`, a site with no vector) reads it and runs no scan
//!   (measured before: 8,285 instructions per cold construct of deltablue's `Variable` with
//!   a scan each time). The memo is per function, so it never reads another function's
//!   constant indices (an M5-P2 cache keyed by a hash of the code bytes could, on a
//!   collision). It holds no Value (rule 8).
//! - Not a GC point; it allocates nothing (fixed arrays).
//! - CTOR-FIT (f-design L1, ESCAPE_SLACK 1; always on): the first construct also counts the
//!   names of the constructor chain (`_super.call(this, x)` in ES5 class code) and keeps going
//!   past a leave of `this` ([`chain_capacity`]); that count is used only when it is above
//!   [`GENEROUS`]. The seam [`ctor_fit`] picks it; the old count ([`own_capacity`]) stays
//!   beside it. The scan is a run-time read, so no bytecode, unit or layout changes.

use ep9_bytecode::{Instructions, Opcode, OperandKind, Register, RuntimeId};

use crate::function_info::fi_flags;
use crate::kinds::{self as kind, is_callable};
use crate::object::{self, cell, obj, rd};
use crate::runtime::Runtime;
use crate::shape::{self, slf};

/// The in-object capacity of a new object whose prototype is not %Object.prototype% when no
/// count is known (`object::new_object`; the capacity before M5-P2).
pub(crate) const GENEROUS: u8 = 8;

/// The most distinct `this.<name> =` names one scan keeps; more gives [`None`].
pub(crate) const MAX_NAMES: usize = 64;

/// The registers a scan follows (locals and parameters below this index each).
const TRACKED_REGISTERS: u32 = 64;

/// `FunctionInfo::slack_capacity` of a function whose count is unknown ([`GENEROUS`]).
pub(crate) const NO_COUNT: u8 = u8::MAX;

#[cfg(test)]
std::thread_local! {
    /// Tests: the scans [`instance_capacity`] ran (first constructs).
    pub(crate) static SCANS: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

/// Where the scan found `this`: the accumulator and two register masks.
#[derive(Default)]
struct Holders {
    acc: bool,
    locals: u64,
    params: u64,
}

impl Holders {
    fn holds(&self, r: Register) -> bool {
        if r.is_receiver() {
            return true;
        }
        if let Some(i) = r.local_index() {
            return i < TRACKED_REGISTERS && self.locals & (1 << i) != 0;
        }
        if let Some(i) = r.parameter_index() {
            return i < TRACKED_REGISTERS && self.params & (1 << i) != 0;
        }
        false
    }

    /// Records that `r` now holds `this` (`on`) or not. False if the scan cannot follow it.
    fn set(&mut self, r: Register, on: bool) -> bool {
        if r.is_receiver() {
            // A write of the receiver register: not followed.
            return false;
        }
        let (mask, i) = if let Some(i) = r.local_index() {
            (&mut self.locals, i)
        } else if let Some(i) = r.parameter_index() {
            (&mut self.params, i)
        } else {
            return !on;
        };
        if i >= TRACKED_REGISTERS {
            return !on;
        }
        if on {
            *mask |= 1 << i;
        } else {
            *mask &= !(1 << i);
        }
        true
    }
}

/// The distinct constant-pool indices of the names `code` stores on `this`
/// (`SetNamedProperty` / `DefineNamedOwnProperty` on a register that holds it), or [`None`]
/// when `this` may leave the frame or the scan cannot follow it. A linear scan: it does not
/// follow jumps, so a register that holds `this` on one path only counts as holding it (a
/// count, never a correctness input). Not a GC point.
pub(crate) fn this_store_names(code: &[u8], names: &mut [u32; MAX_NAMES]) -> Option<usize> {
    scan_this_stores::<false>(code, names).map(|(n, _)| n)
}

/// The scan of [`this_store_names`] in two modes. `ESCAPE = false`: today's scan; a leave of
/// `this` gives [`None`]. `ESCAPE = true` (CTOR-FIT, f-design L1 (1)): a leave of
/// `this` sets `escaped` (the second value) and the scan goes on. In both modes more than
/// [`MAX_NAMES`] names or a decode fault give [`None`]. Not a GC point.
pub(crate) fn scan_this_stores<const ESCAPE: bool>(
    code: &[u8],
    names: &mut [u32; MAX_NAMES],
) -> Option<(usize, bool)> {
    let mut h = Holders::default();
    let mut n = 0;
    let mut escaped = false;
    // A leave of `this`: today's scan stops; the escape mode marks it and goes on.
    macro_rules! leave {
        () => {
            if ESCAPE {
                escaped = true;
            } else {
                return None;
            }
        };
    }
    for insn in Instructions::new(code) {
        let insn = insn.ok()?;
        let op = insn.opcode;
        let info = op.info();
        match op {
            Opcode::Ldar => {
                h.acc = h.holds(insn.register(0));
                continue;
            }
            Opcode::Star => {
                if !h.set(insn.register(0), h.acc) {
                    leave!();
                }
                continue;
            }
            Opcode::Mov => {
                let on = h.holds(insn.register(0));
                if !h.set(insn.register(1), on) {
                    leave!();
                }
                continue;
            }
            Opcode::Return => continue,
            Opcode::SetNamedProperty | Opcode::DefineNamedOwnProperty => {
                // Operands: object register, name constant, slot; the value in acc.
                if h.acc {
                    leave!();
                }
                if h.holds(insn.register(0)) {
                    let k = insn.unsigned(1);
                    if !names[..n].contains(&k) {
                        if n == MAX_NAMES {
                            return None;
                        }
                        names[n] = k;
                        n += 1;
                    }
                }
                continue;
            }
            Opcode::GetNamedProperty => {
                // Reads `this.<name>`: `this` stays in the frame.
                h.acc = false;
                continue;
            }
            Opcode::GetNamedPropertyStar => {
                // The pair `GetNamedProperty` then `Star dst` (operand 3; STAR-DST probe).
                h.acc = false;
                if !h.set(insn.register(3), false) {
                    leave!();
                }
                continue;
            }
            _ => {}
        }
        if h.acc && info.acc.reads() {
            leave!();
        }
        let count = insn.operand_count();
        let mut i = 0;
        while i < count {
            let k = insn.operand_kind(i);
            match k {
                OperandKind::Reg | OperandKind::RegInOut => {
                    let r = insn.register(i);
                    if h.holds(r) {
                        leave!();
                    }
                }
                OperandKind::RegOut => {
                    if !h.set(insn.register(i), false) {
                        leave!();
                    }
                }
                OperandKind::RegList | OperandKind::RegOutList => {
                    let first = insn.register(i).to_operand();
                    let len = insn.unsigned(i + 1) as i32;
                    for j in 0..len {
                        let r = Register::from_operand(first + j);
                        if k == OperandKind::RegList && h.holds(r) {
                            leave!();
                        }
                        if k == OperandKind::RegOutList && !h.set(r, false) {
                            leave!();
                        }
                    }
                    i += 1;
                }
                OperandKind::RegPair | OperandKind::RegOutPair | OperandKind::RegOutTriple => {
                    let first = insn.register(i).to_operand();
                    let len = if k == OperandKind::RegOutTriple { 3 } else { 2 };
                    for j in 0..len {
                        let r = Register::from_operand(first + j);
                        if k == OperandKind::RegPair && h.holds(r) {
                            leave!();
                        }
                        if k != OperandKind::RegPair && !h.set(r, false) {
                            leave!();
                        }
                    }
                }
                OperandKind::Runtime
                    if RuntimeId::from_u16(insn.operand(i) as u16)
                        == Some(RuntimeId::ResolvePossiblyDirectEval) =>
                {
                    leave!();
                }
                _ => {}
            }
            i += 1;
        }
        if info.acc.writes() {
            h.acc = false;
        }
    }
    Some((n, escaped))
}

/// The value of the own data property at field `field` of the fast-mode object `o` with
/// shape `s`. Not a GC point.
fn field_value(o: u64, s: u64, field: usize) -> u64 {
    let (cap, k) = shape::field_layout(s);
    let c = cell(o);
    if field < cap {
        rd(c, obj::FIXED + 8 * (object::fixed_count(k) + field))
    } else {
        rd(cell(rd(c, obj::SLOTS)), slf::DATA + 8 * (field - cap))
    }
}

/// True if the own property `i` of `proto` (shape `s`) is a data property whose value is not
/// callable: a likely instance field (`F.prototype.left = null`). Not a GC point.
fn is_proto_field(proto: u64, s: u64, i: usize) -> bool {
    let d = shape::details_at(s, i);
    !d.is_accessor() && !is_callable(field_value(proto, s, usize::from(d.field())))
}

/// The in-object capacity of the instances `new f` makes (module docs), or [`None`] for
/// [`GENEROUS`]: chosen at `f`'s first construct, with the prototype `proto` it has then,
/// and kept in its FunctionInfo. Not a GC point.
pub(crate) fn instance_capacity(rt: &Runtime, f: u64, proto: u64) -> Option<usize> {
    if !object::is_function(f) {
        return None;
    }
    let fi = object::function_info(f);
    let need = fi_flags::BYTECODE | fi_flags::CONSTRUCTOR;
    if fi.flags & need != need || fi.flags & fi_flags::DERIVED != 0 || fi.code.is_null() {
        return None;
    }
    match fi.slack_capacity.get() {
        0 => {}
        NO_COUNT => return None,
        c => return Some(usize::from(c - 1)),
    }
    let r = first_capacity(rt, fi, proto);
    // The capacity is at most MAX_NAMES + GENEROUS (72), or CHAIN_CAP_MAX (253) for a CTOR-FIT
    // chain count, so capacity + 1 fits below NO_COUNT.
    fi.slack_capacity.set(match r {
        Some(c) => u8::try_from(c + 1)
            .ok()
            .filter(|&b| b != NO_COUNT)
            .unwrap_or(NO_COUNT),
        None => NO_COUNT,
    });
    r
}

/// [`instance_capacity`] at the first construct of `fi`: the scan and the prototype count.
/// With CTOR-FIT on ([`ctor_fit`]), the chain count of [`chain_capacity`] at
/// [`ESCAPE_SLACK`] when it is above [`GENEROUS`]; else the old count ([`own_capacity`]).
/// Not a GC point.
fn first_capacity(
    rt: &Runtime,
    fi: &crate::function_info::FunctionInfo,
    proto: u64,
) -> Option<usize> {
    #[cfg(test)]
    SCANS.with(|c| c.set(c.get() + 1));
    // SAFETY: a bytecode FunctionInfo's code is its unit function's bytecode, which the unit
    // keeps at this address while the info lives (`FunctionInfo::bytecode`).
    let code = unsafe { core::slice::from_raw_parts(fi.code, fi.code_len as usize) };
    if ctor_fit(rt)
        && let Some(n) = chain_capacity(rt, fi, code, proto, ESCAPE_SLACK)
        && n > usize::from(GENEROUS)
    {
        return Some(n);
    }
    own_capacity(rt, fi, code, proto)
}

/// The CTOR-FIT seam (ALLOC-SET design 1.3, contract rules 1-3): true = [`first_capacity`]
/// tries the chain count first; false = the old count only. This role head: always true, so
/// the test is a constant and the compiler removes the old branch. The set integrator changes
/// only this body (a byte of `rt`); the merge head deletes the seam and keeps the chain path.
/// It reads no execution policy (no preset, no feedback mode, no unit: rule 40, M-R9). Tests
/// turn it off per thread with [`ctor_fit_override`].
#[inline(always)]
pub(crate) fn ctor_fit(rt: &Runtime) -> bool {
    let _ = rt;
    #[cfg(test)]
    if let Some(on) = ctor_fit_override::ON.get() {
        return on;
    }
    true
}

/// The old count (CTOR-FIT off): the scan of `code` (the code of `fi`) and the prototype
/// count. Not a GC point.
fn own_capacity(
    rt: &Runtime,
    fi: &crate::function_info::FunctionInfo,
    code: &[u8],
    proto: u64,
) -> Option<usize> {
    let mut names = [0u32; MAX_NAMES];
    let n = this_store_names(code, &mut names)?;
    if n == 0 {
        return None;
    }
    let mut extra = 0;
    if kind::is_object(proto) {
        let ps = shape::shape_of(proto);
        if !shape::is_dictionary(ps) {
            extra = (0..shape::count(ps))
                .filter(|&i| is_proto_field(proto, ps, i))
                .count();
            // A name the constructor stores and the prototype has counts once.
            for &k in &names[..n] {
                if extra == 0 {
                    break;
                }
                // SAFETY: `k` is a verified String constant index of this function.
                let key = unsafe { (*fi.consts.add(k as usize)).bits() };
                if let Some(found) = shape::lookup(rt, ps, key)
                    && is_proto_field(proto, ps, found.index)
                {
                    extra -= 1;
                }
            }
        }
    }
    Some((n + extra.min(usize::from(GENEROUS))).min(obj::MAX_INOBJECT))
}

// ---- CTOR-FIT (f-design L1; ESCAPE_SLACK 1) -------------------------------------------------

/// The most prototype objects [`chain_capacity`] reads (f-design L1 (2): 8 levels).
pub(crate) const CHAIN_LEVELS: u32 = 8;

/// The largest capacity [`chain_capacity`] gives: capacity + 1 must stay below [`NO_COUNT`]
/// in the FunctionInfo byte.
pub(crate) const CHAIN_CAP_MAX: usize = NO_COUNT as usize - 2;

/// The room [`first_capacity`] adds to a chain count when a scan saw `this` leave the frame
/// (f-design L1 (3); plan v19 §4.2: ESCAPE_SLACK 1, the arm the month-13 probe read as arm 1).
pub(crate) const ESCAPE_SLACK: usize = 1;

/// The distinct names (key bits) of a constructor chain ([`chain_names`]).
pub(crate) struct ChainNames {
    keys: [u64; CHAIN_CAP_MAX],
    n: usize,
    /// A scan saw `this` leave the frame (a call such as `_super.call(this, x)`).
    pub escaped: bool,
    /// The prototype objects read.
    pub levels: u32,
}

impl ChainNames {
    /// The names, in the order the scans found them.
    pub(crate) fn keys(&self) -> &[u64] {
        &self.keys[..self.n]
    }

    /// Adds the names of the scan result `names` of `fi` that are new. False if there is no
    /// room.
    fn add(&mut self, fi: &crate::function_info::FunctionInfo, names: &[u32]) -> bool {
        for &k in names {
            // Unit strings are atoms (one atom per text, H19), so equal names have equal bits.
            let key = fi.constant(k);
            if self.keys().contains(&key) {
                continue;
            }
            if self.n == CHAIN_CAP_MAX {
                return false;
            }
            self.keys[self.n] = key;
            self.n += 1;
        }
        true
    }
}

/// The names of `fi`'s `this.<name> =` stores and of its constructor chain (f-design L1
/// (1), (2)), or [`None`] when a scan cannot give them (more than [`MAX_NAMES`] names in one
/// code, a decode fault, more than [`CHAIN_CAP_MAX`] names in all).
///
/// ```text
///  fi's code ──scan (escape mode)──> names
///  cur = proto                                   (F.prototype at the first construct)
///  while levels < 8 and cur is an object:
///    stop at a dictionary-mode cur
///    c = own data property `constructor` of cur  stop if none or an accessor
///    stop unless c is a bytecode constructor that is not DERIVED (a host function stops)
///    names += scan (escape mode) of c's code     (distinct key bits; fi itself once)
///    cur = [[Prototype]] of cur
/// ```
///
/// Escape mode: a leave of `this` (`_super.call(this, x)`) sets `escaped`; the scan goes on.
/// Not a GC point; it allocates nothing and runs no JS (own data properties only).
pub(crate) fn chain_names(
    rt: &Runtime,
    fi: &crate::function_info::FunctionInfo,
    code: &[u8],
    proto: u64,
) -> Option<ChainNames> {
    let mut ch = ChainNames {
        keys: [0; CHAIN_CAP_MAX],
        n: 0,
        escaped: false,
        levels: 0,
    };
    let mut names = [0u32; MAX_NAMES];
    let (n, esc) = scan_this_stores::<true>(code, &mut names)?;
    ch.escaped |= esc;
    if !ch.add(fi, &names[..n]) {
        return None;
    }
    let ctor_key = rt.atom(crate::runtime::Atom::Constructor);
    let mut cur = proto;
    while ch.levels < CHAIN_LEVELS && kind::is_object(cur) {
        let s = shape::shape_of(cur);
        if shape::is_dictionary(s) {
            break;
        }
        let Some((c, attrs)) = crate::props::own_get(cur, ctor_key) else {
            break;
        };
        if attrs & object::attr::ACCESSOR != 0 || !object::is_function(c) {
            break;
        }
        let cfi = object::function_info(c);
        let need = fi_flags::BYTECODE | fi_flags::CONSTRUCTOR;
        if cfi.flags & need != need || cfi.flags & fi_flags::DERIVED != 0 || cfi.code.is_null() {
            break;
        }
        ch.levels += 1;
        if !core::ptr::eq(cfi, fi) {
            // SAFETY: as in `first_capacity`.
            let ccode = unsafe { core::slice::from_raw_parts(cfi.code, cfi.code_len as usize) };
            let (n, esc) = scan_this_stores::<true>(ccode, &mut names)?;
            ch.escaped |= esc;
            if !ch.add(cfi, &names[..n]) {
                return None;
            }
        }
        cur = shape::proto(s);
    }
    Some(ch)
}

/// The CTOR-FIT count of `new F` (f-design L1 (3)): `n_chain` = the chain names
/// ([`chain_names`]) + the prototype-field extra (today's rule: the non-callable data
/// properties of `proto` that the chain does not store, at most [`GENEROUS`]) + `slack` when
/// a scan saw `this` leave the frame; at most [`CHAIN_CAP_MAX`]. [`None`] when the chain
/// names are unknown. The caller uses it only when it is above [`GENEROUS`] (so no object
/// gets less in-object room than today). Not a GC point.
pub(crate) fn chain_capacity(
    rt: &Runtime,
    fi: &crate::function_info::FunctionInfo,
    code: &[u8],
    proto: u64,
    slack: usize,
) -> Option<usize> {
    let ch = chain_names(rt, fi, code, proto)?;
    let mut extra = 0;
    if kind::is_object(proto) {
        let ps = shape::shape_of(proto);
        if !shape::is_dictionary(ps) {
            extra = (0..shape::count(ps))
                .filter(|&i| {
                    is_proto_field(proto, ps, i) && !ch.keys().contains(&shape::key_at(ps, i))
                })
                .count();
        }
    }
    let slack = if ch.escaped { slack } else { 0 };
    Some((ch.keys().len() + extra.min(usize::from(GENEROUS)) + slack).min(CHAIN_CAP_MAX))
}

/// Tests only: the value of the seam [`ctor_fit`] on this thread (`None`: the role-head value,
/// true).
#[cfg(test)]
pub(crate) mod ctor_fit_override {
    use core::cell::Cell;

    std::thread_local! {
        pub(crate) static ON: Cell<Option<bool>> = const { Cell::new(None) };
    }
}
