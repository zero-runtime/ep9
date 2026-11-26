// B2-5: a narrowing rule that gives a wrong type. Two structs of the sources have the name
// `Ctx` (here and in b.rs), and each has a field `inner` of another type. The field rule takes
// the type of the field `inner` of `Ctx`: one type for both structs is wrong for one of them.
// `Ctx::go` calls `self.inner.step()` on a `Walker`, `Walker::step` calls `again`, and `again`
// calls `Ctx::go` again. No fn has a site.

pub struct Ctx {
    inner: Walker,
}

impl Ctx {
    fn make() -> Ctx {
        Ctx { inner: Walker { n: 0 } }
    }

    fn go(&mut self) -> u32 {
        self.inner.step()
    }
}

pub struct Walker {
    n: u32,
}

impl Walker {
    fn step(&mut self) -> u32 {
        again(self.n)
    }
}

fn again(n: u32) -> u32 {
    Ctx::make().go() + n
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
