// Review round 2, A-B4: the `Fn` bound is on an impl of a `use .. as` name of the struct, not
// of the struct name. A call through `self.f` is no edge.

mod m {
    pub struct Hook<F> {
        pub f: F,
    }
}

use m::Hook as HH;

impl<F: Fn(u32) -> u32> HH<F> {
    fn call(&self, n: u32) -> u32 {
        (self.f)(n)
    }
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
