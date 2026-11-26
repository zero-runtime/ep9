// Review A1 (probe p1): an implicit call through an index on a field. `walk` never names
// `Tab`; it reads `s.t[0]`, so `Tab::index` runs, and `Tab::index` calls `walk` again.

pub struct Tab;

pub struct S {
    t: Tab,
}

impl std::ops::Index<u32> for Tab {
    type Output = u32;

    fn index(&self, _i: u32) -> &u32 {
        let s = S { t: Tab };
        walk(&s);
        &0
    }
}

fn walk(s: &S) -> u32 {
    s.t[0]
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
