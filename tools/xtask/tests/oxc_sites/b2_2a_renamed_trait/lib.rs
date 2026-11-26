// B2-2 (a): a call through a renamed trait (`use .. as`). `parse_x` calls `R::go`, and `R` is
// the trait `Tr` under another name. `ParserImpl::go` calls `parse_x` again. No fn has a site.

pub trait Tr {
    fn go(&mut self) -> u32;
}

use crate::Tr as R;

impl<C> Tr for ParserImpl<C> {
    fn go(&mut self) -> u32 {
        self.parse_x()
    }
}

impl<C> ParserImpl<C> {
    fn parse_x(&mut self) -> u32 {
        R::go(self)
    }

    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
