# oxc-sites gap fixtures

Cases for `tools/xtask/tests/oxc_sites.rs` (route OXC-SITES-LINT-GAPS). They are never
compiled: `cargo xtask oxc-sites --src tools/xtask/tests/oxc_sites/<case>` only parses them.

Each case is a false PASS of the lint before the route: an unbounded recursion that the lint
read as safe (review R2 lens B, B2-1..B2-5). The lint before the route gives exit 0 and
`PASS` on each case; the test of each case asserts the finding of the fixed lint. Each case
has the 2 loop fns of `LOOPS`, so that a missing loop is no finding.

| Case | Form | The fixed lint gives |
|---|---|---|
| `b2_1_implicit_drop/` | B2-1: `parse_x` makes a `Guard`; its `Drop` calls `parse_x` | exit 1, cycle `Guard::drop`, `parse_x` |
