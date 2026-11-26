// Review A5 (probe v5): the `Fn` bound is on an impl of a type alias of the struct, not of the
// struct name. A call through `self.f` is no edge.

pub struct Hook<F> {
    f: F,
}

type HH<F> = Hook<F>;

impl<F: Fn(u32) -> u32> HH<F> {
    fn call(&self, n: u32) -> u32 {
        (self.f)(n)
    }
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
