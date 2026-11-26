// Review A1 (probe p10): `parse_x` makes a `Holder` by its derived `Default`, which makes a
// `Guard`; the `Holder` drops at the end of `parse_x`, so `Guard::drop` calls `parse_x` again.
// No fn names `Guard`.

pub struct Guard;

impl Default for Guard {
    fn default() -> Self {
        Guard
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        parse_x();
    }
}

#[derive(Default)]
pub struct Holder {
    g: Guard,
}

fn parse_x() {
    let _h = Holder::default();
}

impl<C> ParserImpl<C> {
    fn set_pure_on_call_or_new_expr() {}

    fn jsx_member_expression_eq() {}
}
