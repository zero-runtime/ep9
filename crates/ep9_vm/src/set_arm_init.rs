//! The process init of the ALLOC-SET arm switch (M14-P1; ALLOC-SET design 1.2, 6.2). The set
//! branch only.
//!
//! ```text
//!  Isolate::new_with ── set_arm_init() ── ep9_platform::set_arm::init(sha_slot)   once per process
//!                    ── Heap::new
//!                    ── Runtime::new ── Runtime::set = runtime_bits()               once per isolate
//!  a seam (slack::ctor_fit, shape_cache::on) ── on(rt, its bit) = Runtime::set & the bit
//! ```
//!
//! SRC-HASH is not in this S (the cut: S = {CTOR-FIT, SHAPE-CACHE}) and `ep9_bytecode::sha256`
//! has no body slot at this base, so [`sha_slot`] sets nothing: every arm hashes with the scalar
//! body of today.

/// The first `ep9_platform::set_arm` call of the process (`Isolate::new_with`, before
/// `Heap::new`): reads `EP9_SET_ARM` once and runs [`sha_slot`] once.
pub(crate) fn set_arm_init() {
    ep9_platform::set_arm::init(sha_slot);
}

/// The process work of the arm (design 6.2: the SHA-256 body slot when the SRC-HASH bit is on).
/// This S has no SRC-HASH bit and no slot, so it does nothing.
fn sha_slot(_bits: u8) {}

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

/// A seam's test: true if `bit` (a member of `ep9_platform::set_arm`) is on in the arm byte of
/// `rt` (`Runtime::set`). One byte load from the runtime; no static, no lock.
#[inline(always)]
pub(crate) fn on(rt: &crate::runtime::Runtime, bit: u8) -> bool {
    rt.set.get() & bit != 0
}

#[cfg(test)]
mod tests {
    use ep9_platform::set_arm::{ARM_0, ARM_A, ARM_C, ARM_S, CTOR_FIT, SHAPE_CACHE};

    use crate::tests::{config, isolate, on_big_stack};
    use crate::{Dispatch, atoms, kinds, object::attr, runtime::val, shape, shape_cache, slack};

    /// Each arm and the members it turns on: (bits, CTOR-FIT, SHAPE-CACHE).
    const ARMS: [(u8, bool, bool); 4] = [
        (ARM_0, false, false),
        (ARM_C, true, false),
        (ARM_A, true, true),
        (ARM_S, true, true),
    ];

    #[test]
    fn a_test_runtime_starts_in_arm_s() {
        on_big_stack(|| {
            let iso = isolate(config(Dispatch::DEFAULT, false));
            assert_eq!(iso.rt.set.get(), ARM_S);
            assert_eq!(ARM_S, CTOR_FIT | SHAPE_CACHE);
            assert!(slack::ctor_fit(&iso.rt));
            assert!(shape_cache::on(&iso.rt));
        });
    }

    #[test]
    fn each_arm_turns_on_its_members_only() {
        on_big_stack(|| {
            for (bits, ctor, cache) in ARMS {
                let iso = isolate(config(Dispatch::DEFAULT, false));
                iso.rt.set_arm_for_test(bits);
                assert_eq!(slack::ctor_fit(&iso.rt), ctor, "arm bits {bits}");
                assert_eq!(shape_cache::on(&iso.rt), cache, "arm bits {bits}");
            }
        });
    }

    #[test]
    fn the_test_setter_is_per_runtime() {
        on_big_stack(|| {
            let a = isolate(config(Dispatch::DEFAULT, false));
            let b = isolate(config(Dispatch::DEFAULT, false));
            a.rt.set_arm_for_test(ARM_0);
            assert!(!slack::ctor_fit(&a.rt) && !shape_cache::on(&a.rt));
            assert!(slack::ctor_fit(&b.rt) && shape_cache::on(&b.rt));
        });
    }

    /// Arm 0 and arm C run today's lookup: the scan, and no entry in the cache. Arm S caches.
    #[test]
    fn only_an_arm_with_shape_cache_fills_the_cache() {
        on_big_stack(|| {
            for (bits, _, cache) in ARMS {
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
                assert_eq!(rt.shape_cache.len(), 0, "arm bits {bits}");
                let s = roots.get(i).to_bits();
                for _ in 0..2 {
                    assert_eq!(shape::lookup(rt, s, key).map(|f| f.index), Some(0));
                    assert_eq!(shape::lookup(rt, s, atoms::intern_str(rt, "absent")), None);
                }
                assert_eq!(rt.shape_cache.len() != 0, cache, "arm bits {bits}");
            }
        });
    }
}
