# The month-12 off-loop read: the numbers (2026-11-25)

- These are the numbers behind [R24](../../24-m2-month12-offloop.md): the off-loop split of the TS gate cell (wall against CPU time, the main-thread GC of ep9 against node's helper threads), the month-12 lever reads, the Path B series from the month-10 end to the month-12 end with the read-to-read move on nearly the same VM code, and the cost and review of the L1 parser stack check.
- The files were written by hand from the DL rows, the TD-3 box summaries, the WIDE-SET pooled read text, the 2 end Path B compare files, the L1 speed rows and review records and the box rows of the usage log. Each finding names its sources. The box files and the notes are not in the repo; R24 names them by short form.

## Files

| File | What |
|---|---|
| `findings.json` | one object per finding: `id`, `claim`, `status` (verified, derived, estimate, open), `sources`, `numbers` (schema `ep9-research/m2-month12-offloop/v1`) |
| `offloop.tsv` | the off-loop values of R24 §1: wall and CPU-time ratios, task-clock per wall, the per-thread shares, line G, the GC counts and arms, the derived main-thread split and what a lever must remove |
| `levers.tsv` | the lever table of R24 §2: TD-3, GC-BUDGET, WIDE-SET, LEVER-19 and the 2 ride roles; prediction, measured A/B or NOT RUN, verdict row, Path B transfer |
| `pathb-series.tsv` | the Path B series of R24 §3: items 1, 2, 3, 6 in both builds on both boxes at the month-10, month-11 and month-12 ends, the moves between the reads, the CPU-time and same-round cells, the gross TS time |
| `l1-stack-check.tsv` | the L1 parser stack check of R24 §5: what was built, the cost rows, the review rounds and the CI runs |

## Cite forms

- `DL <row id>` = a row of `docs/decisions-log.md`. `OL M6-ORCH-<n>` = a row of the orchestrator log.
- `TD3:<line>`, `TD3-1:<line>` = a line of the TD-3 box summary of box ep9-x86-m12-td3-2 (the read) or ep9-x86-m12-td3-1 (code 2c5002c8).
- `PB10`, `PBe11`, `PBe12` = the Path B reads at the month-10 end (DL M10-END-D3), the month-11 end on `m11-p3` (DL M11-END-PATHB) and the month-12 end on `m12-p1` (boxes ep9-x86-m12-end-pathb-1, -2); `PBe12:<line>` = a line of its compare file.
- `WR` = the WIDE-SET pooled read text. `R24B`, `R18S`, `R18P`, `P17`, `L1D`, `L1S` = the off-loop base text, the plan-v18 speed and process notes, plan v17, the L1 design note and the L1 speed rows.
- ep9 paths (`crates/...`, `third_party/...`, `docs/...`) are at 6cbc3e2d (`m2/m12`); `git diff --stat fcac0863 6cbc3e2d -- crates third_party tools Cargo.lock` is empty.
- Upstream cites name the tag: `v8/v8` tag `13.6.233.17` (b0a55a7d). The node v24.22.0 cites are not verified from a checkout.

## Not here

The perf and record files, the run JSON, the box logs, the per-run tables, the speed binaries and their outputs, the hole-hunt inputs and the review texts stay outside the repo. No fuzz input is in this directory.
