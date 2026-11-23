//! gc-lint case: a closure that uses a `Local` only in a match guard (syn 3 keeps the guard in
//! the arm's pattern, `Pat::Guard`), and an arm binding that shadows it in its guard.

use crate::abi::Local;

pub fn used_in_a_guard<'s>(local: Local<'s>) -> Box<dyn Fn(u32) -> u32 + 's> {
    Box::new(move |n| match n { // FLAG
        0 if local.is_number() => 1,
        _ => 0,
    })
}

/// The arm binds its own `local`: no finding.
pub fn shadowed_by_the_arm<'s>(local: Local<'s>, o: Option<u32>) -> Box<dyn Fn() -> u32 + 's> {
    assert!(local.is_number());
    Box::new(move || match o {
        Some(local) if local > 0 => local,
        _ => 0,
    })
}
