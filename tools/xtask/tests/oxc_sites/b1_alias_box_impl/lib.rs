// Review round 2, A-B1: the impl with the `Fn` bound is for a type alias of a box of the
// field type (`type HH<F> = Box<Hook<F>>`). A call through `self.f` is no edge.

pub struct Hook<F> {
    f: F,
}

type HH<F> = Box<Hook<F>>;

pub trait Tr {
    fn go(&self) -> u32;
}

impl<F: Fn(u32) -> u32> Tr for HH<F> {
    fn go(&self) -> u32 {
        (self.f)(0)
    }
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
