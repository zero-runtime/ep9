// Route OXC-SITES-LINT-GAPS-2 (A2 "impl Fn args"), a form of the oxc 0.160.0 pin: a parser fn
// takes `impl FnMut(&mut Self) -> Option<T>` (list.rs `collect_list`, function.rs) or
// `impl FnOnce(Token) -> bool` (cursor.rs `eat_if_next`) and calls it. The closure or the fn
// value that a caller passes runs inside that call, so the caller recurses through it.
struct ParserImpl<C> {
    config: C,
}

impl<C> ParserImpl<C> {
    fn collect_list<T>(&mut self, mut next: impl FnMut(&mut Self) -> Option<T>) -> Vec<T> {
        let mut v = Vec::new();
        while let Some(x) = next(self) {
            v.push(x);
        }
        v
    }

    // A closure argument: its body is the code of `parse_a`.
    fn parse_a(&mut self) -> Option<u32> {
        let v = self.collect_list(|p| p.parse_a());
        v.first().copied()
    }

    // A fn value argument.
    fn parse_b(&mut self) -> Option<u32> {
        let v = self.collect_list(Self::parse_b);
        v.first().copied()
    }

    fn eat_if_next(&mut self, accept: impl FnOnce(u32) -> bool) -> bool {
        accept(1)
    }

    // A closure that does not recurse: no cycle.
    fn parse_c(&mut self) -> bool {
        self.eat_if_next(|_| true)
    }

    fn set_pure_on_call_or_new_expr(&mut self) {}
    fn jsx_member_expression_eq() {}
}
