// Review A2 (probe p5): the impl gives the `Fn` bound to a parameter inside the argument of
// the self type (`Hook<Box<F>>`), not to the argument itself. A call through `self.f` is no
// edge.

pub struct Hook<T> {
    f: T,
}

impl<F: Fn(u32) -> u32> Hook<Box<F>> {
    fn call(&self, n: u32) -> u32 {
        (self.f)(n)
    }
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
