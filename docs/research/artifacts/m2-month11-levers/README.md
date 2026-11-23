# The month-11 lever reads: the numbers (2026-11-24)

- These are the numbers behind [R23](../../23-m2-month11-levers.md): each month-11 lever read (predicted against measured, the time per 1% of instructions, the cycles per removed unit), the Path B series from the month-10 end to the month-11 end, the cycle map that is left, the candidate sums, the parser debug-stack measurement, the process numbers and the R22 addendum.
- The files were written by hand from the DL verdict rows, the box summaries and pooled read texts, the CI TS rows, the stack check note of DEPS-BUMP-2 and the plan-v17 research notes. Each finding names its sources. The box files and the notes are not in the repo; R23 names them by short form.

## Files

| File | What |
|---|---|
| `findings.json` | one object per finding: `id`, `claim`, `status` (verified, derived, estimate, open), `sources`, `numbers` (schema `ep9-research/m2-month11-levers/v1`) |
| `levers.tsv` | the lever table of R23 §2: one row per lever, form and build; the predicted gain and its source, the measured median and interval, pairs, boxes, the verdict row, the time per 1% of instructions and the cycles per removed unit with its inputs |
| `pathb-series.tsv` | the Path B series of R23 §4: one row per read, box and build; the TS gate ratio, the same-round t(jitless)/t(jit), the box-neutral time, the gross TS time, the switch and rule-100 words |
| `stack-measure.tsv` | the debug-stack measurement of R23 §7: bytes per pre-scan unit per form at oxc 0.158.0 and 0.159.0, debug and release, and the at-cap parse stack |

## Cite forms

- `DL <row id>` = a row of `docs/decisions-log.md`. `OL M6-ORCH-<n>` = a row of the orchestrator log.
- `PB10`, `PBm`, `PBe` = the Path B reads at the month-10 end, mid month 11 (`m11-p0`) and the month-11 end (`m11-p3`); `PBm:<line>` = a line of the mid compare file.
- `TD2:<line>` = a line of the TD-2 box summary (code 591064fc, 1 box). `L2:<line>` = a line of the LAYOUT-2 box summary.
- `SV` = the stack check note of DEPS-BUMP-2 and its logs. `R17S`, `R17P`, `R17M`, `IN17` = the plan-v17 notes (speed, process, modes, inputs). `R16S`, `P16`, `PK` = the plan-v16 speed note, plan v16, the M11-P3 pick note.
- ep9 paths (`crates/...`, `docs/...`) are at e1c1d45b (`m2m11/prereg`); `git diff --stat m11-p3 e1c1d45b -- crates/` is empty.
- Upstream cites name the repository and the commit: `v8/v8@932ddd7ca`, `WebKit/WebKit@f30433b25`, `mozilla-firefox/firefox@ec287ddb0`, `bellard/quickjs@e585640ff`, `oxc-project/oxc@3464a0d18`.

## Not here

The run JSON, the perf and record files, the per-run tables of the boxes, the box logs, the stack probe code and its logs, the binaries and the dumps stay outside the repo. No fuzz input is in this directory.
