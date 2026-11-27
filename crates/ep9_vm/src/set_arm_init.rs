//! The process init of the SET-15 arm switch (M15-P1; plan v20 4.2; ALLOC-SET design 1.2,
//! 6.2). The set branch only.
//!
//! ```text
//!  Isolate::new_with ── set_arm_init() ── ep9_platform::set_arm::init(sha_slot)   once per process
//!                    │                       sha_slot: the SHA-256 slot, SRC_HASH on only
//!                    ── Heap::new
//!                    ── Runtime::new ── Runtime::set = runtime_bits()               once per isolate
//!  a seam (slack::ctor_fit, shape_cache::on, slow::call::char_code_arm_on,
//!          ops::to_bool_arm) ── on(rt, its bit) = Runtime::set & the bit
//! ```
//!
//! SRC-HASH (design 6.2; plan v20 4.2: the set switch sets the body slot in arm S): in an arm
//! with its bit (P, S, A) [`sha_slot`] sets the SHA-256 body slot of `ep9_bytecode` from the
//! SHA256 pick of `ep9_platform::simd`; in arm 0 the slot stays unset and every hash runs the
//! scalar loop of today, whatever `EP9_SIMD` forces (the arm bit first, then the force: design
//! 6.4). The slot is process state, so no test sets it: the tests read [`sha_body`], and each
//! body is tested by its name in `ep9_platform` and `ep9_bytecode`. A compile before the first
//! isolate (`ep9_cli` compiles the script first) hashes with the scalar body in every arm.

/// The first `ep9_platform::set_arm` call of the process (`Isolate::new_with`, before
/// `Heap::new`): reads `EP9_SET_ARM` once and runs [`sha_slot`] once.
pub(crate) fn set_arm_init() {
    ep9_platform::set_arm::init(sha_slot);
}

/// The process work of the arm (design 6.2): the SHA-256 body slot of `ep9_bytecode` gets
/// [`sha_body`], once. Nothing in `ep9_vm` sets the slot but this call.
fn sha_slot(bits: u8) {
    if let Some(body) = sha_body(bits) {
        let _ = ep9_bytecode::sha256::set_body(body);
    }
}

/// The SHA-256 body of the slot for the arm bits `bits`: the SHA256 pick of this process
/// (`ep9_platform::simd::sha256_slot_body`) when the SRC-HASH bit is on, else `None` (the slot
/// stays unset: the scalar loop). The pick is also `None` when it is the scalar body.
fn sha_body(bits: u8) -> Option<ep9_bytecode::sha256::BlockBody> {
    if bits & ep9_platform::set_arm::SRC_HASH != 0 {
        ep9_platform::simd::sha256_slot_body()
    } else {
        None
    }
}

/// The arm bits that `Runtime::new` copies into `Runtime::set`: the process value. In a test
/// build the S bits, the value the member tests were written for (each member always on at its
/// role head); a test picks another arm with `Runtime::set_arm_for_test`, never with the
/// process variable.
#[inline]
pub(crate) fn runtime_bits() -> u8 {
    if cfg!(test) {
        ep9_platform::set_arm::ARM_S
    } else {
        ep9_platform::set_arm::bits()
    }
}

/// A seam's test: true if `bit` (a member of `ep9_platform::set_arm`) is on in the arm word of
/// `rt` (`Runtime::set`). One load from the runtime; no static, no lock. On aarch64 a full
/// 32-bit read (volatile: LLVM does not narrow it to a byte), so one `ldr` with an immediate
/// offset reaches it: a byte load this far into the Runtime needs an offset register, which
/// LLVM keeps in a saved register of a loop (a spill in arm 0). Other targets: a plain read.
#[inline(always)]
pub(crate) fn on(rt: &crate::runtime::Runtime, bit: u8) -> bool {
    #[cfg(target_arch = "aarch64")]
    let w = {
        // SAFETY: the pointer of a `Cell<u32>` of a live `Runtime`: valid and aligned, and no
        // other thread writes it (a Runtime is one thread's).
        unsafe { core::ptr::read_volatile(rt.set.as_ptr()) }
    };
    #[cfg(not(target_arch = "aarch64"))]
    let w = rt.set.get();
    w & u32::from(bit) != 0
}

/// The first statement of each side of a seam that ends in 2 tail calls (`if on { side(true);
/// f_on(..) } else { side(false); f_off(..) }`): an empty `asm!` block, with a text of its own
/// per side (2 equal blocks would move above the branch), so LLVM cannot fold a side into a
/// conditional tail jump (x86-64 `jne f_on`). Then each arm pays the same seam: the bit test,
/// one branch and one jump (x86-64 and aarch64), with no frame. No instructions, no memory, no
/// flags.
#[inline(always)]
pub(crate) fn side(on: bool) {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    // SAFETY: the blocks have no instructions (the texts are comments); they read and write
    // nothing.
    unsafe {
        if on {
            core::arch::asm!("/* set arm on */", options(nomem, nostack, preserves_flags))
        } else {
            core::arch::asm!(
                "/* set arm off */",
                options(nomem, nostack, preserves_flags)
            )
        }
    };
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    let _ = on;
}

#[cfg(test)]
mod tests {
    use ep9_bytecode::VerifiedUnit;
    use ep9_platform::set_arm::{
        ARM_0, ARM_A, ARM_P, ARM_S, CHARCODE, CTOR_FIT, MEMBERS, SHAPE_CACHE, SRC_HASH, SWEEP_LEAN,
        TO_BOOL,
    };

    use crate::call::{CHAR_CODE_ARM_HITS, HOST_PROPERTY_HITS};
    use crate::interp::ops::{TO_BOOL_ARM_HITS, to_bool_arm};
    use crate::tests::{Asm, config, isolate, on_big_stack, take_output, unit};
    use crate::{Dispatch, atoms, kinds, object::attr, runtime::val, shape, shape_cache, slack};

    /// Each arm and the `Runtime` members it turns on: (bits, CTOR-FIT, SHAPE-CACHE, CHARCODE,
    /// TO-BOOL). The last 6 rows are not arms: each member reads its own bit and no other, and
    /// the reserved SWEEP-LEAN bit turns nothing on.
    const ARMS: [(u8, bool, bool, bool, bool); 10] = [
        (ARM_0, false, false, false, false),
        (ARM_P, true, true, false, false),
        (ARM_A, true, true, true, true),
        (ARM_S, true, true, true, true),
        (CTOR_FIT, true, false, false, false),
        (SHAPE_CACHE, false, true, false, false),
        (CHARCODE, false, false, true, false),
        (TO_BOOL, false, false, false, true),
        (SWEEP_LEAN | SRC_HASH, false, false, false, false),
        (!MEMBERS, false, false, false, false),
    ];

    /// The process member and the reserved bit of each arm: (bits, SWEEP-LEAN, SRC-HASH).
    /// SWEEP-LEAN is out of the set: its bit is off in every arm. SRC-HASH: P, S and A.
    const PROCESS_ARMS: [(u8, bool, bool); 4] = [
        (ARM_0, false, false),
        (ARM_P, false, true),
        (ARM_A, false, true),
        (ARM_S, false, true),
    ];

    #[test]
    fn a_test_runtime_starts_in_arm_s() {
        on_big_stack(|| {
            let iso = isolate(config(Dispatch::DEFAULT, false));
            assert_eq!(iso.rt.set.get(), u32::from(ARM_S));
            assert_eq!(ARM_S, MEMBERS & !SWEEP_LEAN);
            assert!(slack::ctor_fit(&iso.rt));
            assert!(shape_cache::on(&iso.rt));
            assert!(to_bool_arm(&iso.rt));
        });
    }

    /// The seams that this module can name (`slow::call` is private: CHARCODE is read through
    /// a run, [`the_new_members_take_their_arm_only_with_their_bit`]).
    #[test]
    fn each_arm_turns_on_its_members_only() {
        on_big_stack(|| {
            for (bits, ctor, cache, _, to_bool) in ARMS {
                let iso = isolate(config(Dispatch::DEFAULT, false));
                iso.rt.set_arm_for_test(bits);
                assert_eq!(slack::ctor_fit(&iso.rt), ctor, "arm bits {bits:#x}");
                assert_eq!(shape_cache::on(&iso.rt), cache, "arm bits {bits:#x}");
                assert_eq!(to_bool_arm(&iso.rt), to_bool, "arm bits {bits:#x}");
            }
        });
    }

    #[test]
    fn the_test_setter_is_per_runtime() {
        on_big_stack(|| {
            let a = isolate(config(Dispatch::DEFAULT, false));
            let b = isolate(config(Dispatch::DEFAULT, false));
            a.rt.set_arm_for_test(ARM_0);
            assert!(!slack::ctor_fit(&a.rt) && !shape_cache::on(&a.rt) && !to_bool_arm(&a.rt));
            assert!(slack::ctor_fit(&b.rt) && shape_cache::on(&b.rt) && to_bool_arm(&b.rt));
        });
    }

    /// `"R".cc(0)` with `cc` = the host function of `charCodeAt` (a `CallProperty1`), then
    /// `ToBoolean(cc)` (a function: a cell that is always true): prints 82, then true.
    fn new_members_unit() -> VerifiedUnit {
        let mut s = Asm::script(0);
        let l = s.b.new_register_list(2);
        let (f, slot) = (s.f, s.call);
        s.lda_str("R");
        s.b.star(l.get(0)).lda_smi(0).star(l.get(1));
        s.lda_global("cc");
        s.b.star(f).call_property1(f, l.get(0), l.get(1), slot);
        s.print_acc();
        s.lda_global("cc");
        s.b.to_boolean();
        s.print_acc();
        s.b.lda_undefined().return_();
        unit("", vec![s.finish()])
    }

    /// CHARCODE and TO-BOOL by the arm byte only (no test override): with its bit the arm
    /// answers (the `charCodeAt` arm, no host call; the cell arm of `ToBoolean`); without it
    /// the old path runs (the host call of `call_property`'s old body; the cold `ToBoolean`
    /// body), with the same output. Arm 0 is today's path for both.
    #[test]
    fn the_new_members_take_their_arm_only_with_their_bit() {
        for &d in Dispatch::ALL {
            for (bits, _, _, charcode, to_bool) in ARMS {
                let label = format!("{} arm bits {bits:#x}", d.name());
                let (out, arm, host, cell) = on_big_stack(move || {
                    let mut iso = isolate(config(d, false));
                    iso.rt.set_arm_for_test(bits);
                    iso.register_host_function("cc", 1, crate::abi::strings::char_code_at);
                    take_output();
                    CHAR_CODE_ARM_HITS.with(|h| h.set(0));
                    HOST_PROPERTY_HITS.with(|h| h.set(0));
                    TO_BOOL_ARM_HITS.with(|h| h.set(0));
                    // Below the call record window of the isolate start (x86-64: a call in
                    // the window takes `slow::call`, not `call_property`).
                    iso.cx.budget = crate::interp::BUDGET / 2;
                    let r = iso.with_cx_checkpoint(|cx| {
                        cx.run_script(new_members_unit(), "set_arm.js").map(|_| ())
                    });
                    assert!(r.is_ok(), "{:?}", r.err());
                    (
                        take_output(),
                        CHAR_CODE_ARM_HITS.with(|h| h.get()),
                        HOST_PROPERTY_HITS.with(|h| h.get()),
                        TO_BOOL_ARM_HITS.with(|h| h.get()),
                    )
                });
                assert_eq!(out, ["82", "true"], "{label}");
                assert_eq!(
                    (arm, host),
                    if charcode { (1, 0) } else { (0, 1) },
                    "{label}"
                );
                assert_eq!(cell, u64::from(to_bool), "{label}");
            }
        }
    }

    /// Arm 0 runs today's lookup: the scan, and no entry in the cache. P and S cache.
    #[test]
    fn only_an_arm_with_shape_cache_fills_the_cache() {
        on_big_stack(|| {
            for (bits, _, cache, ..) in ARMS {
                let iso = isolate(config(Dispatch::DEFAULT, false));
                let rt = &*iso.rt;
                rt.set_arm_for_test(bits);
                let roots = rt.heap.scratch_roots();
                let key = atoms::intern_str(rt, "set_arm_key");
                let root = shape::root_shape_for(rt, shape::null_proto_info(rt), kinds::OBJECT, 4);
                let i = match shape::add(rt, root, key, attr::DEFAULT) {
                    shape::Added::Shape(s) => roots.push(val(s)),
                    shape::Added::Dictionary => panic!("unexpected dictionary mode"),
                };
                // A collection empties the table (the weak source).
                rt.heap.collect();
                assert_eq!(rt.shape_cache.len(), 0, "arm bits {bits:#x}");
                let s = roots.get(i).to_bits();
                for _ in 0..2 {
                    assert_eq!(shape::lookup(rt, s, key).map(|f| f.index), Some(0));
                    assert_eq!(shape::lookup(rt, s, atoms::intern_str(rt, "absent")), None);
                }
                assert_eq!(rt.shape_cache.len() != 0, cache, "arm bits {bits:#x}");
            }
        });
    }

    /// SRC-HASH reads the process value, not `Runtime::set`, and the reserved SWEEP-LEAN bit
    /// is in no arm. The SHA body of the slot: the pick in an arm with SRC-HASH (P, S, A), none
    /// (the scalar loop) in arm 0. No test sets the slot or the process variable.
    #[test]
    fn each_arm_has_its_process_members_only() {
        let pick = ep9_platform::simd::sha256_slot_body().map(|f| f as usize);
        for (bits, sweep, sha) in PROCESS_ARMS {
            assert_eq!(bits & SWEEP_LEAN != 0, sweep, "arm bits {bits:#x}");
            assert_eq!(bits & SRC_HASH != 0, sha, "arm bits {bits:#x}");
            let body = super::sha_body(bits).map(|f| f as usize);
            assert_eq!(body, if sha { pick } else { None }, "arm bits {bits:#x}");
        }
        // The SRC-HASH bit alone decides: every other member bit gives no body.
        assert!(super::sha_body(MEMBERS & !SRC_HASH).is_none());
        assert_eq!(super::sha_body(SRC_HASH).map(|f| f as usize), pick);
    }

    /// The body of each arm gives the scalar digest (the corpus line in short: the corpus
    /// digest test runs the scalar body and the pick). Arm 0 hashes with the scalar loop.
    #[test]
    fn the_sha_body_of_each_arm_gives_the_scalar_digest() {
        use ep9_bytecode::sha256::{scalar_blocks, sha256_with, to_hex};
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let data: Vec<u8> = (0..4099u32).map(|i| (i * 31 + 7) as u8).collect();
        let want = sha256_with(scalar_blocks, &data);
        for (bits, ..) in PROCESS_ARMS {
            let body = super::sha_body(bits).unwrap_or(scalar_blocks);
            assert_eq!(
                to_hex(&sha256_with(body, b"abc")),
                abc,
                "arm bits {bits:#x}"
            );
            assert_eq!(sha256_with(body, &data), want, "arm bits {bits:#x}");
        }
        assert!(super::sha_body(ARM_0).is_none());
    }

    /// The slot of this process after its first isolate: set exactly when the process arm has
    /// SRC-HASH and the pick is a SIMD body (`EP9_SET_ARM` unset or 0: unset). Only
    /// `set_arm_init` sets the slot in this test binary; the test reads it and sets nothing.
    #[test]
    fn the_slot_is_set_only_by_an_arm_with_src_hash() {
        on_big_stack(|| drop(isolate(config(Dispatch::DEFAULT, false))));
        let bits = ep9_platform::set_arm::bits();
        assert_eq!(
            ep9_bytecode::sha256::body_is_set(),
            super::sha_body(bits).is_some(),
            "process arm bits {bits:#x}"
        );
        if bits & SRC_HASH == 0 {
            assert!(!ep9_bytecode::sha256::body_is_set());
        }
    }
}
