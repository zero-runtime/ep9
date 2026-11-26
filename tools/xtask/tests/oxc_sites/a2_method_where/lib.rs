// Review A2 (probe p3): the `Fn` bound of the field type is on a method of the impl (a
// `where` clause), not on the struct or the impl. A call through `self.f` is no edge.

pub struct Hook<F> {
    f: F,
}

impl<F> Hook<F> {
    fn call(&self, n: u32) -> u32
    where
        F: Fn(u32) -> u32,
    {
        (self.f)(n)
    }
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
