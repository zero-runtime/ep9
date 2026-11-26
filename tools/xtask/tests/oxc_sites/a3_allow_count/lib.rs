// Review A3 (probe p6): the ALLOW entry (parse_regex_pattern -> ParserImpl::parse by
// `Method(None, "parse")`) is for the one call `LiteralParser::new(..).parse()`. A second call
// of the same form, `me.parse()`, is a real recursion into `ParserImpl::parse`.

impl<C> ParserImpl<C> {
    fn parse(&mut self) -> u32 {
        self.parse_regex_pattern()
    }

    fn parse_regex_pattern(&mut self) -> u32 {
        let n = LiteralParser::new(self.source()).parse();
        let me = &mut *self;
        n + me.parse()
    }

    fn source(&self) -> u32 {
        0
    }

    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
