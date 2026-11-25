// Review round 2, A-B1: a fn takes a slice of the field type with an `Fn` bound on the
// argument (`&[Hook<F>]`). A call through `hs[0].f` is no edge.

pub struct Hook<F> {
    f: F,
}

fn call<F: Fn(u32) -> u32>(hs: &[Hook<F>]) -> u32 {
    (hs[0].f)(0)
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
