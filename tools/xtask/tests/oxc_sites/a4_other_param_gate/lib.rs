// Review A4 (probes p8, v8): the gate is the const of a type parameter `D` of the fn, not of
// the config `C` of the parser (`ParserImpl<C>`). `parse_a::<C>` enters with the real config,
// then calls `parse_a::<NoTokensParserConfig>`, whose `D::CHECKS_RECURSION` is `false`: no
// stop.

impl<C: Config> ParserImpl<C> {
    fn parse_a<D: Config>(&mut self) -> u32 {
        if D::CHECKS_RECURSION && !self.config.may_recurse() {
            return self.recursion_stopped();
        }
        self.parse_a::<NoTokensParserConfig>()
    }

    fn recursion_stopped(&mut self) -> u32 {
        0
    }

    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
