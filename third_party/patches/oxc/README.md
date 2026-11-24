# oxc patches (oxc_parser)

ep9 builds oxc_parser 0.158.0 from a git checkout of oxc with ep9's patches on top, in the same
form as the regress and icu4x forks (third_party/patches/regress/README.md,
third_party/patches/icu4x/README.md; vendor by hash; docs/plan/01-front-end.md section 7,
docs/plan/08-repo.md sections 1 and 5). Only oxc_parser comes from the checkout; every other oxc
crate (oxc_ast, oxc_allocator, oxc_span, oxc_semantic, ...) comes from crates.io.

```text
third_party/oxc.commit              upstream commit (the vcs commit of the crates.io             tracked, orchestrator moves it
                                    oxc_parser 0.158.0 package: "release(crates): oxc v0.158.0")
third_party/patches/oxc/*.patch     ep9 patches, applied in name order                          tracked (this folder)
scripts/sync-oxc.sh                 sync_dep oxc <url> (scripts/sync-lib.sh)
third_party/oxc/                    checkout + patches                                          git-ignored
[patch.crates-io] oxc_parser = { path = ".../third_party/oxc/crates/oxc_parser" }
                                    in Cargo.toml, fuzz/Cargo.toml, spikes/spike-b/Cargo.toml and
                                    spikes/spike-c/Cargo.toml (every workspace that builds
                                    ep9_frontend); dep-guards check 16 checks all 4
```

## State

- Pin: `74e5e665ab8227c3d43abf3ce0e4383830e80c12` of https://github.com/oxc-project/oxc, the
  commit that `.cargo_vcs_info.json` of the crates.io oxc_parser 0.158.0 package names
  (`path_in_vcs` crates/oxc_parser). Its `crates/oxc_parser/src` equals the package's `src`
  (`diff -r`: no difference).
- Patches: 2 files. The local patch count of H01 (at most 3) counts source changes: 0001 is
  packaging (the icu4x 0001 precedent) and changes no line of `src/`.

  | File | What | Upstream |
  |---|---|---|
  | `0001-parser-published-manifest.patch` | `crates/oxc_parser/Cargo.toml` becomes the manifest of the crates.io package (crates.io versions, no `workspace = true`), so the path package uses the crates.io oxc crates of the rest of the graph, not path copies from the checkout. One change from the package file: the authors entry keeps the names and drops the e-mail address | Not for upstream: only a downstream `[patch]` needs it |
  | `0002-parser-recursion-check.patch` | H01 layer 1: `ParserConfig::CHECKS_RECURSION` (default `false`: every site compiles away) and `ParserConfig::may_recurse(&self)` (default `true`); 22 sites at 0.158.0: 20 checks (17 token-driven recursion entries and the 3 cover walks `AssignmentTarget::cover`, `SimpleAssignmentTarget::cover`, `cover_binding_as_expression`) and 2 walks made loops (`set_pure_on_call_or_new_expr`, `jsx_member_expression_eq`); a stop flag outside the checkpoint (a rewind drops a fatal error), read at the exits of `parse` and `parse_expression`; the fatal diagnostic `stack_limit`; `ParserReturn::stack_limit_hit`. Test: `tests/recursion_check.rs`. `cargo xtask oxc-sites` (T-SITES) checks the site list on the synced checkout | Issue oxc-project/oxc#21953; the PR text is the repo draft `docs/upstream/oxc-parser-recursion-check.md` (status draft, not sent) |

- The source of the patches is a branch of a scratch clone of this checkout (no commit in
  `third_party/oxc`); each file is its `git format-patch --zero-commit --no-signature` with the
  author `ep9` and no date.
- Bump: oxc_parser moves with the other oxc crates in one bump step (Cargo.toml, the oxc pins).
  Then the pin moves to the vcs commit of the new package, 0001 is made again from the new
  package manifest, and each other patch is ported to the new sources.

## Rules

- **Sync before cargo.** Every cargo command in the 4 workspaces needs `third_party/oxc`, because
  a missing patch path fails the resolve step. Run `bash scripts/sync-oxc.sh` in a new tree first.
  It does nothing when the pin and the patch set are unchanged (`.ep9-stamp`). CI runs it in
  `.github/actions/setup-rust` (cached by the pin and this folder).
- **Patch files.** `NNNN-<what>.patch`, applied with `git apply` in name order. A new or changed
  patch re-runs the sync (the stamp holds the hash of all patch files). A path dependency has no
  checksum in `Cargo.lock`, so a patch changes no Cargo table.
- **Live check.** `.github/scripts/dep-guards.sh` check 16: in all 4 workspaces `cargo metadata`
  shows oxc_parser with no registry source, from this checkout.
- **Licence.** oxc is MIT. A patch is code in this repository: the PR that adds the first source
  patch adds an oxc entry to `THIRD-PARTY-LICENSE`.
