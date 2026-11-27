//! The arm switch of the SET-15 read (M15-P1; plan v20 4.2; ALLOC-SET design 1): `EP9_SET_ARM`,
//! read once per process. The set branch only: the merge head has no such module.
//!
//! ```text
//!  EP9_SET_ARM   arm   members on                                  bits
//!  0, not set,   0     none (today's path)                         ARM_0 = 0
//!  other text          (C is gone: P replaces C and A)
//!  P             P     the month-14 members: CTOR-FIT,             ARM_P = CTOR_FIT | SHAPE_CACHE
//!                      SHAPE-CACHE, SRC-HASH (0x0B)                        | SRC_HASH
//!  S             S     every member (the decision arm; 0x3B)       ARM_S = ARM_P | CHARCODE | TO_BOOL
//!  A             S     A = S (no member is jit only)               ARM_A = ARM_S
//!  0 again       0'    none: the A/A arm, the same binary and the same value as 0; the box
//!                      runner tells the 2 runs apart by its own arm label (a0 / a0p), never
//!                      by this value, so the environment is the same in both
//!
//!  bits()  ── one OnceLock: the one engine read of the name (the test charcode_arm.rs too)
//!    ├─ ep9_vm::set_arm_init (Isolate::new_with, before Heap::new) ── init(first): first(bits)
//!    │    once; first sets the SHA-256 body slot of ep9_bytecode when SRC_HASH is on
//!    └─ Runtime::new copies the byte into Runtime::set; a seam tests one bit of it
//! ```
//!
//! The bit of each member is fixed here. SWEEP-LEAN is out of the set (its x86 line was not
//! read): its bit stays reserved, in no arm, and nothing reads it. A member that is out at the
//! cut has a bit that does nothing; every member of the set has its seam body.
//! The value never reaches `ExecOptions`, `ExecPolicy`, a fact, a unit or a bytecode stream
//! (M-R9). Tests never set the variable: a test of `ep9_vm` sets the byte of its own `Runtime`.

use std::ffi::OsStr;
use std::sync::{Once, OnceLock};

/// The environment variable of the switch.
pub const NAME: &str = "EP9_SET_ARM";

/// CTOR-FIT: the chain count of the first construct (`ep9_vm` `slack::ctor_fit`).
pub const CTOR_FIT: u8 = 1 << 0;
/// SHAPE-CACHE: the shape lookup cache in front of the descriptor scan
/// (`ep9_vm` `shape_cache::on`).
pub const SHAPE_CACHE: u8 = 1 << 1;
/// SWEEP-LEAN: no dead-cell header read and the SIMD bitmap pass of the sweep. Out of the set
/// (its x86 line was not read): the bit is reserved, off in every arm, and nothing reads it.
pub const SWEEP_LEAN: u8 = 1 << 2;
/// SRC-HASH: the SHA-256 body slot of `ep9_bytecode` gets the SHA256 pick of
/// `ep9_platform::simd` (set once per process by `ep9_vm` `set_arm_init`); off, the slot stays
/// unset and every hash runs the scalar loop of today.
pub const SRC_HASH: u8 = 1 << 3;
/// CHARCODE: the `charCodeAt` arm of the 1-argument method call path (`ep9_vm`
/// `interp::slow::call::char_code_arm_on`).
pub const CHARCODE: u8 = 1 << 4;
/// TO-BOOL: the cell arm of the ToBoolean jumps (`ep9_vm` `interp::ops::to_bool_arm`).
pub const TO_BOOL: u8 = 1 << 5;

/// Every member bit (SWEEP-LEAN too: a reserved bit, in no arm).
pub const MEMBERS: u8 = CTOR_FIT | SHAPE_CACHE | SWEEP_LEAN | SRC_HASH | CHARCODE | TO_BOOL;

/// Arm 0: no member on (arm 0' is a second run of it).
pub const ARM_0: u8 = 0;
/// Arm P: the month-14 members of the set on (SWEEP-LEAN is out), the new members off: 0x0B.
pub const ARM_P: u8 = CTOR_FIT | SHAPE_CACHE | SRC_HASH;
/// Arm S: every member of the set on (the decision arm): 0x3B.
pub const ARM_S: u8 = ARM_P | CHARCODE | TO_BOOL;
/// Arm A: no member is jit only, so A = S (plan v20 4.2).
pub const ARM_A: u8 = ARM_S;

/// The arm bits of one value of [`NAME`]: `P`, `S` and `A` (exactly 1 byte each) name their
/// arm; anything else (`0`, no value, `C`, any other text) is arm 0.
pub fn bits_of(value: Option<&OsStr>) -> u8 {
    match value.map(OsStr::as_encoded_bytes) {
        Some(b"P") => ARM_P,
        Some(b"S") => ARM_S,
        Some(b"A") => ARM_A,
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
        assert_eq!(v("P"), CTOR_FIT | SHAPE_CACHE | SRC_HASH);
        assert_eq!(v("P"), 0x0B);
        assert_eq!(v("S"), MEMBERS & !SWEEP_LEAN);
        assert_eq!(v("S"), 0x3B);
        // No member is jit only: A = S.
        assert_eq!(v("A"), v("S"));
        // C is gone (P replaces C and A); 0' is the value 0 again, never a value of its own.
        for other in [
            "", "C", "c", "p", "s", "a", "SS", "PP", " S", "S ", "P ", "1", "B", "0'",
        ] {
            assert_eq!(v(other), ARM_0, "{other:?}");
        }
    }

    #[test]
    fn the_arms_differ_only_in_their_members() {
        assert_eq!(ARM_0 & MEMBERS, 0);
        // P: the month-14 members of the set (CTOR-FIT, SHAPE-CACHE, SRC-HASH), no new one.
        assert_eq!(ARM_P, CTOR_FIT | SHAPE_CACHE | SRC_HASH);
        assert_eq!(ARM_P & (CHARCODE | TO_BOOL), 0);
        // S: every member of the set; S / P = the 2 new members.
        assert_eq!(ARM_S, MEMBERS & !SWEEP_LEAN);
        assert_eq!(ARM_S & !ARM_P, CHARCODE | TO_BOOL);
        assert_eq!(ARM_A, ARM_S);
        // SWEEP-LEAN is out: its bit is reserved and off in every arm.
        for arm in [ARM_0, ARM_P, ARM_S, ARM_A] {
            assert_eq!(arm & SWEEP_LEAN, 0, "{arm:#x}");
        }
        assert_eq!((ARM_0, ARM_P, ARM_S, ARM_A), (0x00, 0x0B, 0x3B, 0x3B));
        // One bit per member, and the layout that the members' seam bodies read.
        let bits = [
            CTOR_FIT,
            SHAPE_CACHE,
            SWEEP_LEAN,
            SRC_HASH,
            CHARCODE,
            TO_BOOL,
        ];
        assert!(bits.iter().all(|b| b.count_ones() == 1));
        assert_eq!(bits.iter().fold(0, |a, b| a | b), MEMBERS);
        assert_eq!(MEMBERS.count_ones(), 6);
        assert_eq!(bits, [1, 2, 4, 8, 16, 32]);
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
