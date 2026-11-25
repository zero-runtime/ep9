// B2-3: a site form whose gate is the const of a fixed config type, not of the generic
// parameter. `NoTokensParserConfig::CHECKS_RECURSION` is the trait default `false`, so the
// check never runs for the config that checks (L1Config), and `parse_a` recurses with no stop.

impl<C: Config> ParserImpl<C> {
    fn parse_a(&mut self) -> u32 {
        if NoTokensParserConfig::CHECKS_RECURSION && !self.config.may_recurse() {
            return self.recursion_stopped();
        }
        self.parse_a()
    }

    fn recursion_stopped(&mut self) -> u32 {
        0
    }

    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
