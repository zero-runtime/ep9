//! The arm switch of the ALLOC-SET read (M14-P1; ALLOC-SET design 1): `EP9_SET_ARM`, read once
//! per process. The set branch only: the merge head has no such module.
//!
//! ```text
//!  EP9_SET_ARM   arm   members on                    bits
//!  0, not set,   0     none (today's path)           0
//!  other text
//!  C             C     CTOR-FIT                      CTOR_FIT
//!  A             A     S without JIT-LEAN            = S (JIT-LEAN is out of this S)
//!  S             S     every member of the cut       CTOR_FIT | SHAPE_CACHE
//!
//!  bits()  ── one OnceLock: the only environment read of the name in the workspace
//!    ├─ ep9_vm::set_arm_init (Isolate::new_with, before Heap::new) ── init(first): first(bits) once
//!    └─ Runtime::new copies the byte into Runtime::set; a seam tests one bit of it
//! ```
//!
//! The cut of this read is S = {CTOR-FIT, SHAPE-CACHE}: a member that is not in S has no bit
//! here and no code in the set branch. The value never reaches `ExecOptions`, `ExecPolicy`, a
//! fact, a unit or a bytecode stream (M-R9). Tests never set the variable: a test of `ep9_vm`
//! sets the byte of its own `Runtime`.

use std::ffi::OsStr;
use std::sync::{Once, OnceLock};

/// The environment variable of the switch.
pub const NAME: &str = "EP9_SET_ARM";

/// CTOR-FIT: the chain count of the first construct (`ep9_vm` `slack::ctor_fit`).
pub const CTOR_FIT: u8 = 1 << 0;
/// SHAPE-CACHE: the shape lookup cache in front of the descriptor scan
/// (`ep9_vm` `shape_cache::on`).
pub const SHAPE_CACHE: u8 = 1 << 1;

/// Arm 0: no member on.
pub const ARM_0: u8 = 0;
/// Arm C: CTOR-FIT alone (a part arm).
pub const ARM_C: u8 = CTOR_FIT;
/// Arm S: every member of the cut (the decision arm).
pub const ARM_S: u8 = CTOR_FIT | SHAPE_CACHE;
/// Arm A: S without JIT-LEAN. JIT-LEAN is out of this S, so A = S (design 1.1).
pub const ARM_A: u8 = ARM_S;

/// The arm bits of one value of [`NAME`]: `C`, `A` and `S` (exactly 1 byte each) name their
/// arm; anything else (`0`, no value, any other text) is arm 0.
pub fn bits_of(value: Option<&OsStr>) -> u8 {
    match value.map(OsStr::as_encoded_bytes) {
        Some(b"C") => ARM_C,
        Some(b"A") => ARM_A,
        Some(b"S") => ARM_S,
        _ => ARM_0,
    }
}

/// The arm bits of this process: [`NAME`] read at the first call, then the same value.
pub fn bits() -> u8 {
    static BITS: OnceLock<u8> = OnceLock::new();
    *BITS.get_or_init(|| bits_of(std::env::var_os(NAME).as_deref()))
}

/// [`bits`], and `first(bits)` at the first call of the process only (one `Once`). `ep9_vm`
/// hands in the process work of the arm (design 1.2, 6.2).
pub fn init(first: fn(u8)) -> u8 {
    static FIRST: Once = Once::new();
    let b = bits();
    FIRST.call_once(|| first(b));
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn each_value_names_its_arm_and_any_other_text_is_arm_0() {
        let v = |s: &str| bits_of(Some(OsString::from(s).as_os_str()));
        assert_eq!(bits_of(None), ARM_0);
        assert_eq!(v("0"), ARM_0);
        assert_eq!(v("C"), CTOR_FIT);
        assert_eq!(v("S"), CTOR_FIT | SHAPE_CACHE);
        // JIT-LEAN is out of this S: A = S.
        assert_eq!(v("A"), v("S"));
        for other in ["", "c", "s", "a", "SS", " S", "S ", "1", "B", "0'"] {
            assert_eq!(v(other), ARM_0, "{other:?}");
        }
    }

    #[test]
    fn the_arms_differ_only_in_their_members() {
        assert_eq!(ARM_0 & (CTOR_FIT | SHAPE_CACHE), 0);
        assert_eq!(ARM_C & SHAPE_CACHE, 0);
        assert_ne!(ARM_C & CTOR_FIT, 0);
        assert_eq!(ARM_S, CTOR_FIT | SHAPE_CACHE);
        assert_eq!(CTOR_FIT & SHAPE_CACHE, 0);
    }

    #[test]
    fn init_runs_first_once_and_returns_the_process_bits() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static RUNS: AtomicU32 = AtomicU32::new(0);
        fn first(b: u8) {
            assert_eq!(b, bits());
            RUNS.fetch_add(1, Ordering::Relaxed);
        }
        assert_eq!(init(first), bits());
        assert_eq!(init(first), bits());
        assert_eq!(RUNS.load(Ordering::Relaxed), 1);
    }
}
