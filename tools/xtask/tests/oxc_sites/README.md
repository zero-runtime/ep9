# oxc-sites gap fixtures

Cases for `tools/xtask/tests/oxc_sites.rs` (route OXC-SITES-LINT-GAPS). They are never
compiled: `cargo xtask oxc-sites --src tools/xtask/tests/oxc_sites/<case>` only parses them.

Each case is a false PASS of the lint before the route: an unbounded recursion that the lint
read as safe (review R2 lens B, B2-1..B2-5). The lint before the route gives exit 0 and
`PASS` on each case; the test of each case asserts the finding of the fixed lint. Each case
has the 2 loop fns of `LOOPS`, so that a missing loop is no finding. The fixed lint also
reports each ALLOW entry as unused on each case (the entries are for the oxc_parser sources),
so a test asserts its own finding line, not only the exit code.

| Case | Form | The fixed lint gives |
|---|---|---|
| `b2_1_implicit_drop/` | B2-1: `parse_x` makes a `Guard`; its `Drop` calls `parse_x` | exit 1, cycle `Guard::drop`, `parse_x` |
| `b2_2a_renamed_trait/` | B2-2 (a): `parse_x` calls `R::go`, `use crate::Tr as R`; `ParserImpl::go` calls `parse_x` | exit 1, cycle `go`, `parse_x` |
| `b2_2b_type_alias/` | B2-2 (b): `walk(p: &mut P<C>)` calls `p.parse_x()`, `type P<C> = ParserImpl<C>` | exit 1, cycle `parse_x`, `walk` |
| `b2_2c_generic_fn_field/` | B2-2 (c): `struct Hook<F: Fn(u32) -> u32> { f: F }` | exit 2, `Hook.f: the field holds a fn value` |
| `b2_3_foreign_gate/` | B2-3: `if NoTokensParserConfig::CHECKS_RECURSION && !self.config.may_recurse() { return .. }` at the start of a self-recursive `parse_a` | exit 1, `NOT A SITE FORM at lib.rs:7`, cycle `parse_a` |
| `b2_4_allow_by_use/` | B2-4: `JSXClosing::dummy` calls itself by `Self::dummy`; ALLOW has (JSXClosing::dummy, JSXClosing::dummy) for the use `Dummy::dummy` | exit 1, cycle `JSXClosing::dummy`, `UNUSED ALLOW entry` for that entry |
