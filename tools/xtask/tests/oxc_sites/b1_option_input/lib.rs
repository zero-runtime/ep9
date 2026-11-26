// Review round 2, A-B1: a fn takes the field type inside another type (`Option<Hook<F>>`).
// A call through `h.f` is no edge.

pub struct Hook<F> {
    f: F,
}

fn call<F: Fn(u32) -> u32>(h: Option<Hook<F>>) -> u32 {
    h.map_or(0, |h| (h.f)(0))
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
