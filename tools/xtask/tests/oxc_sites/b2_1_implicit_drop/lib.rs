// B2-1: a recursion that an implicit call closes. `parse_x` makes a `Guard`, and the drop of
// the guard at the end of its scope calls `Guard::drop`, which calls `parse_x` again. No fn has
// a site, so this is an unbounded recursion. The code writes no call into `Guard::drop`.

pub struct Guard<'p, C> {
    p: &'p mut ParserImpl<C>,
}

impl<C> Drop for Guard<'_, C> {
    fn drop(&mut self) {
        self.p.parse_x();
    }
}

impl<C> ParserImpl<C> {
    fn parse_x(&mut self) {
        if self.more() {
            let _g = Guard { p: self };
        }
    }

    fn more(&self) -> bool {
        true
    }

    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
