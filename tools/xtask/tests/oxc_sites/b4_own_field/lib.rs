// Review round 2, A-B4: two structs have a field `t` of other types. `S::walk` reads
// `self.t[0]`; the type of `self` is known (`S`), so the index is `Tab::index` only, never
// `Plain::index`. Both index fns call `back`, and `back` calls `walk`: the cycle is
// `Tab::index`, `back`, `S::walk`, with no `Plain::index`.

pub struct Tab;

pub struct Plain;

pub struct S {
    t: Tab,
}

pub struct Other {
    t: Plain,
}

impl std::ops::Index<u32> for Tab {
    type Output = u32;

    fn index(&self, _i: u32) -> &u32 {
        back();
        &0
    }
}

impl std::ops::Index<u32> for Plain {
    type Output = u32;

    fn index(&self, _i: u32) -> &u32 {
        back();
        &0
    }
}

impl S {
    fn walk(&self) -> u32 {
        self.t[0]
    }
}

fn back() -> u32 {
    let s = make();
    s.walk()
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
