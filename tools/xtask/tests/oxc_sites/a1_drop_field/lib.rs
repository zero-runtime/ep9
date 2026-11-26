// Review A1 (probe p2): an implicit drop of a field value. `parse_x` writes `self.holder.g`,
// so the old `Guard` drops, and `Guard::drop` calls `parse_x` again. No fn names `Guard`.

pub struct Guard<'p, C> {
    p: &'p mut ParserImpl<C>,
}

impl<C> Drop for Guard<'_, C> {
    fn drop(&mut self) {
        self.p.parse_x();
    }
}

pub struct Holder<'p, C> {
    g: Option<Guard<'p, C>>,
}

impl<C> ParserImpl<C> {
    fn parse_x(&mut self) {
        self.holder.g = None;
    }

    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
