# oxc-sites gap fixtures

Cases for `tools/xtask/tests/oxc_sites.rs` (route OXC-SITES-LINT-GAPS). They are never
compiled: `cargo xtask oxc-sites --src tools/xtask/tests/oxc_sites/<case>` only parses them.

Each case is a false PASS of the lint before the route: an unbounded recursion that the lint
read as safe (review R2 lens B, B2-1..B2-5; the `a*` cases: the M13-P0 review round 1, A1-A5,
a false PASS of the lint at b94675f8; the `b1_*`..`b3_*` cases: review round 2, A-B1..A-B3,
a false PASS of the lint at eb6b89ab; the `b4_*` cases: round 2 A-B4, a test of a branch
that no case had, red on a copy of the lint without that branch). The lint before the route gives exit 0 and
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
| `b2_5_field_collision/` | B2-5: 2 structs `Ctx` (a.rs, b.rs) with a field `inner` of 2 types; `Ctx::go` calls `self.inner.step()` on the `Walker` one, and `Walker::step` reaches `Ctx::go` again | exit 1, cycle `Ctx::go`, `Walker::step`, `again` |
| `a1_index_field/` | A1: `walk` reads `s.t[0]` (`Index` of `Tab`), `Tab::index` calls `walk`; no fn names `Tab` | exit 1, cycle `Tab::index`, `walk` |
| `a1_drop_field/` | A1: `parse_x` writes `self.holder.g = None` (drops a `Guard`), `Guard::drop` calls `parse_x` | exit 1, cycle `Guard::drop`, `parse_x` |
| `a1_derive_default/` | A1: `parse_x` makes a `Holder` by a derived `Default` (a `Guard` field) and drops it | exit 1, cycle `Guard::drop`, `parse_x` |
| `a1_macro_tokens/` | A1: `parse_x` makes guards only in `vec![Guard; n]` (tokens that do not parse) | exit 1, cycle `Guard::drop`, `parse_x` |
| `a2_method_where/` | A2: `where F: Fn(u32) -> u32` on a method of `impl<F> Hook<F>` | exit 2, `Hook.f: the field holds a fn value` |
| `a2_fn_supertrait/` | A2: `trait Cb: Fn(u32) -> u32`, `struct Hook<F: Cb>` | exit 2, `Hook.f: the field holds a fn value` |
| `a2_impl_wrapped/` | A2: `impl<F: Fn(u32) -> u32> Hook<Box<F>>` | exit 2, `Hook.f: the field holds a fn value` |
| `a3_allow_count/` | A3: `parse_regex_pattern` has the allowed `LiteralParser::new(..).parse()` and one more `me.parse()` | exit 1, cycle `parse`, `parse_regex_pattern` |
| `a4_other_param_gate/` | A4: the gate `D::CHECKS_RECURSION`, `D` a type parameter of the fn, not the `C` of `ParserImpl<C>` | exit 1, `NOT A SITE FORM at lib.rs:8`, cycle `parse_a` |
| `a5_alias_impl/` | A5: `type HH<F> = Hook<F>; impl<F: Fn(u32) -> u32> HH<F>` | exit 2, `Hook.f: the field holds a fn value` |
| `b1_slice_input/` | round 2 A-B1: `fn call<F: Fn(u32) -> u32>(hs: &[Hook<F>])` | exit 2, `Hook.f: the field holds a fn value` |
| `b1_option_input/` | round 2 A-B1: `fn call<F: Fn(u32) -> u32>(h: Option<Hook<F>>)` | exit 2, `Hook.f: the field holds a fn value` |
| `b1_box_impl/` | round 2 A-B1: `impl<F: Fn(u32) -> u32> Tr for Box<Hook<F>>` | exit 2, `Hook.f: the field holds a fn value` |
| `b1_alias_box_impl/` | round 2 A-B1: `type HH<F> = Box<Hook<F>>; impl<F: Fn(u32) -> u32> Tr for HH<F>` | exit 2, `Hook.f: the field holds a fn value` |
| `b2_trait_default/` | round 2 A-B2: a default trait fn `fn go<F: Fn(u32) -> u32>(&self, h: &Hook<F>)` | exit 2, `Hook.f: the field holds a fn value` |
| `b3_rebound_parser/` | round 2 A-B3: `let p: &mut ParserImpl<NoTokensParserConfig> = ..;` before `if C::CHECKS_RECURSION && !p.config.may_recurse()` | exit 1, `NOT A SITE FORM at lib.rs:8`, cycle `parse_a` |
| `b4_fn_input/` | round 2 A-B4 (a mark of the inputs of a free fn): `fn call<F: Fn(u32) -> u32>(h: &Hook<F>)` | exit 2, `Hook.f: the field holds a fn value` |
| `b4_use_as_fn/` | round 2 A-B4 (`Fn` by a `use .. as` name): `use std::ops::Fn as Cb; struct Hook<F: Cb(u32) -> u32>` | exit 2, `Hook.f: the field holds a fn value` |
| `b4_use_as_impl/` | round 2 A-B4 (an impl of a `use .. as` name): `use m::Hook as HH; impl<F: Fn(u32) -> u32> HH<F>` | exit 2, `Hook.f: the field holds a fn value` |
| `b4_own_field/` | round 2 A-B4 (a field of a known type): `S { t: Tab }`, `Other { t: Plain }`; `S::walk` reads `self.t[0]`; both `index` fns reach `walk` | exit 1, cycle `S::walk`, `Tab::index`, `back`, no `Plain::index` |
