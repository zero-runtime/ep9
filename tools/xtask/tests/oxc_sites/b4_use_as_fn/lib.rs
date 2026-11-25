// Review round 2, A-B4: the bound of the field type is `Fn` by a `use .. as` name. A call
// through `self.f` is no edge.

use std::ops::Fn as Cb;

pub struct Hook<F: Cb(u32) -> u32> {
    f: F,
}

impl<F: Cb(u32) -> u32> Hook<F> {
    fn call(&self, n: u32) -> u32 {
        (self.f)(n)
    }
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
