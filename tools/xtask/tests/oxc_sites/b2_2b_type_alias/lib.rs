// B2-2 (b): a parameter whose type is a type alias of the parser. `walk` calls `p.parse_x()`
// on a `&mut P<C>`, and `P` is `ParserImpl` under another name. `parse_x` calls `walk` again.
// No fn has a site.

type P<C> = ParserImpl<C>;

fn walk<C>(p: &mut P<C>) -> u32 {
    p.parse_x()
}

impl<C> ParserImpl<C> {
    fn parse_x(&mut self) -> u32 {
        walk(self)
    }

    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
