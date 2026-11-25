// B2-4: an ALLOW entry hides a real recursion. The entry (JSXClosing::dummy, JSXClosing::dummy)
// is for the edge that `Dummy::dummy(allocator)` makes (an oxc_ast impl of the same name).
// Here `JSXClosing::dummy` calls itself by `Self::dummy`: another use, the same edge. The
// source has no `Dummy::dummy` call, so the entry also removes no edge of its own use.

pub struct JSXClosing;

impl JSXClosing {
    fn dummy(n: u32) -> u32 {
        if n == 0 { 0 } else { Self::dummy(n - 1) }
    }
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
