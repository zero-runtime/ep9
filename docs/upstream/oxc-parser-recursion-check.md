# parser: an opt-in recursion check in `ParserConfig` (stop deep input with a diagnostic, not a stack overflow)

| Field | Value |
|---|---|
| Project, ref | oxc, a PR for issue oxc-project/oxc#21953 (open). The change is the ep9 patch `third_party/patches/oxc/0002-parser-recursion-check.patch` on oxc_parser 0.158.0 |
| Status | draft (2026-11-25). Not "ready": no agent that did not write it ran the repro, and the repro on upstream main is not run (12-governance §5 check 1). Not sent: only the owner sends it |
| ep9 need | H01 layer 1 (built in month 12, M12-P1; DL M12-L1-START): deep input gives a RangeError, never an abort; gate P1 (01-front-end §7). Due: none, the local patch covers it |
| Workaround | The local patch itself (`third_party/patches/oxc`, through `[patch.crates-io]`). Local oxc patches after this: 1/3 (the patch set has 2 patch files; `0001-parser-published-manifest.patch` is packaging, not for upstream, and does not count). When upstream takes the change, the count goes back to 0 |
| Verified by | none yet |

## Upstream state

Read on 2026-11-24 for the design of the patch; not read again for this draft (the role that wrote it had no network). Read it again before "ready".

- Issue #21953 ("parser: stack overflow on deeply nested statement bodies") is open since 2026-04-29. A member quoted the project lead: no `stacker`, because it does not work in wasm.
- 2026-11-01: a comment proposes an opt-in `ParseOptions::max_nesting_depth`. 2026-11-05: a comment says that `checkpoint` / `rewind` can drop a fatal error, and that 6 entry points miss paths such as `keyof keyof T`.
- PR #28929 (open since 2026-11-06) adds a fixed limit of 256 levels. PR #28930 (the regex parser, the same approach) is closed. The older PR #24577 (a fixed limit of 256) was closed as not planned on 2026-10-17.
- oxc 0.160.0 has no depth limit and no stack check.

## Repro (pinned version)

Not run as a standalone program for this draft. The earlier draft [oxc-depth-limit.md](oxc-depth-limit.md) has the repro on 0.155.0 (arrays and parens abort at about 5,500 levels on an 8 MiB thread). On 0.158.0 the patch test `crates/oxc_parser/tests/recursion_check.rs` of the patch shows the abort of one form: with the member-chain walk as a recursion again, `/*@__PURE__*/a` followed by `.a` x 200,000 overflows a 1 MiB thread (debug); with the patch it is a loop and the test passes. Before "ready": run the cases of oxc-depth-limit.md on 0.158.0 without the patch and with it.

## Repro (upstream main)

Not run (no network). Before "ready": port the patch to upstream main (0.160.0 has 25 sites: `cover_binding_as_expression` is gone, and 4 new sites are in `js/grammar.rs`) and run its test there.

## Open items for the owner (before sending)

The oxc contributor guide (`AGENTS.md` and `CONTRIBUTING.md` of oxc main, read at 3464a0d18 of 2026-11-16; read them again before sending) asks these of an issue or a PR. None is done in this draft; each is a call of the owner:

1. AI disclosure: the PR says that AI tools were used and names the model name and version of each (if the exact model is not known, it says so).
2. A human contributor who takes full responsibility for the PR, and who reviewed, tested and understood the code before sending it. Which account sends it is an owner call.
3. A scoped Conventional Commits title, for example `feat(parser): opt-in recursion check in ParserConfig` (the oxc PR workflow checks it; the title in "Text to post" is not in that form).
4. Their checks for an oxc_parser change: `just allocs` (the allocation snapshots), `just test`, and `cargo coverage parser` (the parser conformance). ep9 has no oxc workspace build, so none of them ran.
5. The oxc_parser test section of `AGENTS.md` tells an AI tool: "Do not add unit tests to the lexer or parser unless explicitly instructed by the user". The user there is the human who directs the AI tool, not a maintainer. The same section says that the parser has conformance tests only (`tasks/coverage`). The patch adds `crates/oxc_parser/tests/recursion_check.rs`: the owner, who directs the tool here, decides to keep it, move it, or drop it.

## Text to post

> **parser: an opt-in recursion check in `ParserConfig`**
>
> Fixes #21953 for callers that turn the check on. Nothing changes for the built-in configs.
>
> A deep input (`[[[[...]]]]`, `!!!!...x`, `{{{{...}}}}`, `<a><a>...`) makes the parser recurse once per level, and a deep enough input ends the process with a native stack overflow. A library user cannot know before the parse how much stack an input needs, and a fixed depth limit (#24577, #28929) is too low for some users and too high for a small thread. This PR lets the caller decide, with no cost for the callers that do not ask.
>
> **What it adds**
>
> - `ParserConfig::CHECKS_RECURSION: bool` (default `false`) and `ParserConfig::may_recurse(&self) -> bool` (default `true`). The built-in configs keep the defaults, so every check compiles away for them. A config that sets `CHECKS_RECURSION = true` gives the body: for example "the stack pointer is above a limit", or a counter.
> - 20 checks, one at each recursion entry that the call graph needs: 17 entries that tokens drive (assignment expression, unary, update, await, binary rest, private-in, primary after the identifier fast path, statement list item, binding pattern, TS type, non-array type, type operator, `is_start_of_type`, type assertion, namespace declaration, JSX element, JSX fragment) and 3 walks over a subtree that is already built (`AssignmentTarget::cover`, `SimpleAssignmentTarget::cover`, `cover_binding_as_expression`). A check is `if C::CHECKS_RECURSION && !self.config.may_recurse() { return self.recursion_stopped(); }`; `recursion_stopped` sets a fatal error, as `fatal_error` does.
> - 2 walks become loops: `set_pure_on_call_or_new_expr` (down a member chain) and `jsx_member_expression_eq` (down a JSX name chain). The parser builds a member chain with a loop, so a walk over it must not use stack per member.
> - The stop is sticky. `rewind` restores the fatal error of its checkpoint, so it can drop a stop (the point of the 2026-11-05 comment on #21953). A `Cell<bool>` outside the checkpoint keeps the stop, and the exits of `parse` and `parse_expression` read it. Then `parse` gives one "Maximum call stack size exceeded" error, a program with no body, `fatal_error = true`, and a new field `ParserReturn::stack_limit_hit` (the struct is `#[non_exhaustive]`); `parse_expression` gives `Err` with that one error.
>
> **Which entries.** A cycle in the call graph of `oxc_parser` that no check cuts is a recursion with no stop. With the 20 checks and the 2 loops, the call graph of the 0.158.0 sources has no such cycle (a call-graph script over the sources: function calls, method calls, path calls and function values). If one of the 20 checks is removed, a cycle stays. `keyof keyof T` and the other TS type paths are in the list.
>
> **Why the config, not a depth option.** V8, JavaScriptCore, SpiderMonkey and QuickJS all stop their parsers by a compare of the stack position with a limit, not by a fixed depth. A `ParserConfig` method lets an embedder do the same, or count levels, with no new field in `ParseOptions` and no cost for other users.
>
> **Cost.** About 4 instructions per check when it is on (an estimate; aarch64: read the stack pointer, load the limit, compare, branch). In an engine that turns the check on, its whole parse step of the Octane sources took +0.45% instructions against the same engine without layer 1 (the unpatched crate and none of the engine's own stack checks; release, aarch64), and the outputs were byte-equal.
>
> **Not covered.** The check does not bound the regular expression parser when `ParseOptions::parse_regular_expression` is on: the regex parser runs inside the parse and recurses in `parse_disjunction` and `parse_nested_class`. A follow-up PR can add the same 2 checks there. Until then, a caller that needs a bound keeps that option off (the default) and checks its regex literals on its own.
>
> **Tests.** `tests/recursion_check.rs`: a deep input of each form stops with one error; a stop at any one entry survives every rewind; a config that checks and goes on gives the result of one that does not check; a stop wins over a lexer error before it; the 2 loops on a 1 MiB thread.
