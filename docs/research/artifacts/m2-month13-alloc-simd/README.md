# Allocation cost and SIMD levers (month 13): the sources and the counts (2026-11-25)

- These are the sources behind [R26](../../26-m2-month13-alloc-simd.md): the ALLOC lever design (7 allocation levers L1-L7, the SIMD sites, the sum against the gate need), the K1 regroup of the TD-3 cycle tables, the K0, K2 and SHAPE-LOOKUP counts with their check, and the JIT-DELTA table with its read.
- The `.md` files are copies of the working notes. Each copy keeps its line numbers, so a cite `f-design.md:245` names the same line as in the note. Only scratch paths were changed: each one is now a short name or a repo path. The notes, the mapper files A-E, the design check and the TD-3 box files stay outside the repo.
- `census.patch.txt` is the K2 count patch as **text only**. It was never committed and is never applied. The worktree was clean after the runs (`counts-check.md` (c)).

## Files

| File | What |
|---|---|
| `f-design.md` | the lever design F-DESIGN (route ALLOC): §1 the 7 levers L1-L7, §4 the lever cards with the kill and pick lines, §5 the SIMD sites and the dispatch plan, §6 the sum against the need, §7 the count runs K0-K6, §10 the corrections of its adversarial check |
| `k1-regroup.md` | K1: the TD-3 box 2 cycle symbol tables by class (e-jitless, e-jit, node-jitless) |
| `k0.md` | K0: `--gc-stats` and `ic-stats` at 5c3e1e11, both builds, Mac aarch64 |
| `k2.md` | K2: the CENSUS (cells and bytes by kind, SLOTS cells by class, the chain rule, adds, SHAPEs, for-in, named-load hits) and the kill line of each lever |
| `shape-lookup.md` | SHAPE-LOOKUP-COUNT: `shape::lookup_with` calls, the caller split, a simulated (shape, key) cache and the SHAPE-LOOKUP-CACHE price; the line-M read route |
| `counts-check.md` | the read-only check of K0, K2 and SHAPE-LOOKUP (verdict: usable; record faults F1-F6) |
| `levers.tsv` | one row per lever: the A/B estimate, G8, size, the count line, the count value and the read (written for R26 from the files above) |
| `findings.json` | one object per R26 finding: `id`, `claim`, `status`, `sources`, `numbers` (schema `ep9-research/m2-month13-alloc-simd/v1`) |
| `k0-gc.txt` | K0 raw: one `--gc-stats` line per run (12 runs) |
| `k0-ic.jitless.txt`, `k0-ic.jit.txt` | K0 raw: the `ic-stats` output of each build (named `ic.<b>.err` in `k0.md`) |
| `census-c2.jitless.txt`, `census-c5.jit.txt` | K2 raw: the census output of run C2 (jitless) and C5 (jit) (named `k2/census.jitless.err` and `k2/census.jit.err` in `k2.md`) |
| `sim-l1.py`, `sim-l1.txt` | the L1 simulation on the C2 per-field counts and its output (the net rows of `k2.md` §2) |
| `census.patch.txt` | the K2 count patch (feature `census`), text only |
| `jit-delta-table.txt` | JIT-DELTA (run 45606203565, base = head = 5c3e1e11, octane-typescript, linux-x64 cachegrind): the table "what the jit build runs more", jit minus jitless, per handler, per (site, hook) and per hook, each row priced low / center / high |
| `jit-delta-handlers.tsv`, `jit-delta-hooks-by-site.tsv`, `jit-delta-hooks.tsv` | the 3 tables of `jit-delta-table.txt` as tsv, all rows (Ir only) |
| `jit-delta-read.md` | the JIT-DELTA read: the A/A check, 3 rows checked by hand, the groups by fact kind with prices, Ir against the wall gap, the call and 3 candidate ideas for plan v19 ([R26](../../26-m2-month13-alloc-simd.md) §11) |

## Cite forms

- Inside the copies: `P18:n` = line n of plan v18 (not public at this record). `TD3`, `ICS`, `GCS`, `HA` and A-E are named in `f-design.md` §0; none is in the repo. `T:crates/...` = a repo path at 5c3e1e11.
- `DL <row id>` = a row of `docs/decisions-log.md`. `OL M6-ORCH-<n>` = a row of the orchestrator log (not in the repo).
- C1-C5 = the census runs of `k2.md` §0. C1 has no raw file (`counts-check.md` F1).

## Not here

The 5 mapper notes A-E, the design before its check (`f-design.pre-check.md`) and the check itself (`g-check.md`), the TD-3 box files, the JIT-DELTA cachegrind `.out` files (in the run artifact) and the table script `jit-delta-table.mjs`, the binaries, the `*.time` files, the C3 and C4 raw files and `apply-census.py` stay outside the repo. No fuzz input is in this directory.
