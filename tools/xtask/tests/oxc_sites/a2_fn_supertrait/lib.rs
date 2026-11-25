// Review A2 (probe p4): the bound of the field type is a trait of the sources with `Fn` as a
// supertrait. A call through `self.f` is no edge.

pub trait Cb: Fn(u32) -> u32 {}

pub struct Hook<F: Cb> {
    f: F,
}

impl<F: Cb> Hook<F> {
    fn call(&self, n: u32) -> u32 {
        (self.f)(n)
    }
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
