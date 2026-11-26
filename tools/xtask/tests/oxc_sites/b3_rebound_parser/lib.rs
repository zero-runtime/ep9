// Review round 2, A-B3: a top-level `let` binds the parser name `p` again, with another
// config, before the check. The gate `C` is the config of the old `p`, not of the new one:
// `NoTokensParserConfig` does not stop, and `parse_a` calls itself.

impl<C: Config> ParserImpl<C> {
    fn parse_a(p: &mut ParserImpl<C>) -> u32 {
        let p: &mut ParserImpl<NoTokensParserConfig> = p.no_tokens();
        if C::CHECKS_RECURSION && !p.config.may_recurse() {
            return p.recursion_stopped();
        }
        Self::parse_a(p)
    }

    fn recursion_stopped(&mut self) -> u32 {
        0
    }

    fn no_tokens(&mut self) -> u32 {
        0
    }

    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
