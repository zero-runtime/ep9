// Review A1 (probe p7): a `Guard` made only in the tokens of a macro of another crate that do
// not parse as expressions (`vec![Guard; n]`); the guards drop at the end of `parse_x`, and
// `Guard::drop` calls `parse_x` again.

#[derive(Clone)]
pub struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        parse_x(1);
    }
}

fn parse_x(n: usize) {
    let _v = vec![Guard; n];
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
