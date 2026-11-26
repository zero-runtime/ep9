// B2-2 (c): a field whose type is a type parameter with an `Fn` bound. A call through
// `(self.f)(p)` is a call through a fn value that the graph cannot see (as a fn pointer field
// is), so the run must stop.

pub struct Hook<F: Fn(u32) -> u32> {
    f: F,
}

impl<F: Fn(u32) -> u32> Hook<F> {
    fn call(&self, n: u32) -> u32 {
        (self.f)(n)
    }
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
