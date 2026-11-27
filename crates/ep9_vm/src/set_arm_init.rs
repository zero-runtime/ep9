//! The process init of the SHA-256 body slot (M15-P1 SRC-HASH; ALLOC-SET design 3.1 item 1,
//! 6.2): one `Once`, at the call site of the set branch's arm init, with no arm test.
//!
//! ```text
//!  Isolate::new_with ── set_arm_init() ── the SHA-256 slot of ep9_bytecode   once per process
//!                    ── Heap::new             = the SHA256 pick of ep9_platform::simd
//! ```
//!
//! The slot is process state, so no test sets it: the tests read [`sha_body`], and each body is
//! tested by its name in `ep9_platform` and `ep9_bytecode`. A compile before the first isolate
//! (`ep9_cli` compiles the script first) hashes with the scalar body.

use std::sync::Once;

/// The first call of the process (`Isolate::new_with`, before `Heap::new`): sets the SHA-256
/// body slot of `ep9_bytecode` to [`sha_body`], once. Nothing in `ep9_vm` sets the slot but this
/// call.
pub(crate) fn set_arm_init() {
    static FIRST: Once = Once::new();
    FIRST.call_once(|| {
        if let Some(body) = sha_body() {
            let _ = ep9_bytecode::sha256::set_body(body);
        }
    });
}

/// The SHA-256 body of the slot: the SHA256 pick of this process
/// (`ep9_platform::simd::sha256_slot_body`), or `None` when the pick is the scalar body (the
/// slot stays unset: the scalar loop).
fn sha_body() -> Option<ep9_bytecode::sha256::BlockBody> {
    ep9_platform::simd::sha256_slot_body()
}

#[cfg(test)]
mod tests {
    use crate::Dispatch;
    use crate::tests::{config, isolate, on_big_stack};

    /// The body of the slot gives the scalar digest (the corpus line in short: the corpus
    /// digest test runs the scalar body and the pick).
    #[test]
    fn the_sha_body_gives_the_scalar_digest() {
        use ep9_bytecode::sha256::{scalar_blocks, sha256_with, to_hex};
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let data: Vec<u8> = (0..4099u32).map(|i| (i * 31 + 7) as u8).collect();
        let body = super::sha_body().unwrap_or(scalar_blocks);
        assert_eq!(to_hex(&sha256_with(body, b"abc")), abc);
        assert_eq!(sha256_with(body, &data), sha256_with(scalar_blocks, &data));
    }

    /// The slot of this process after its first isolate: set exactly when the pick is a SIMD
    /// body. Only `set_arm_init` sets the slot in this test binary; the test reads it and sets
    /// nothing.
    #[test]
    fn the_slot_holds_the_pick_after_the_first_isolate() {
        on_big_stack(|| drop(isolate(config(Dispatch::DEFAULT, false))));
        assert_eq!(
            ep9_bytecode::sha256::body_is_set(),
            super::sha_body().is_some()
        );
    }
}
