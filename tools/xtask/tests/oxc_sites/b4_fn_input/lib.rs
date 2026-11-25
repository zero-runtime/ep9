// Review round 2, A-B4: a free fn takes the field type with an `Fn` bound on the argument
// (`&Hook<F>`). A call through `h.f` is no edge.

pub struct Hook<F> {
    f: F,
}

fn call<F: Fn(u32) -> u32>(h: &Hook<F>) -> u32 {
    (h.f)(0)
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
