# The month-14 plan research: the numbers (2026-11-26)

- These are the numbers behind [R27](../../27-m2-month14-plan-research.md): the month-14 speed sets against the need of the month-13 end read, the jit-only gap of TD-4, the month-13 process, box and token costs, the drift risk of each month-14 set, and the round's forecasts read against the later data.
- The files were written by hand from the 3 plan-v19 research notes (speed, process, modes), the plan-v19 inputs list, the month-13 end compare file, the TD-4 summary, the FUZZ-H ledger row, the usage log and the DL rows. Each finding names its sources. The notes, the compare file, the TD-4 summary, the usage log and plan v19 are not in the repo; R27 names them by short form.

## Files

| File | What |
|---|---|
| `findings.json` | one object per finding: `id`, `claim`, `status` (verified, derived, estimate, guess, open), `sources`, `numbers` (schema `ep9-research/m2-month14-plan-research/v1`) |
| `levers.tsv` | the month-14 set table of R27 §1.4 and §1.6: each candidate with its class, the measured hot loop, its A/B gain (low / center / high), jit column, G8, the round's call and its source |
| `forecast-later.tsv` | the table of R27 §1.9: each forecast of the round, its source and mark, the later data and its source, and the status |

## Cite forms

- `DL <row id>` = a row of `docs/decisions-log.md` (rows at 67fdda4d). `OL M6-ORCH-<n>` = a row of the orchestrator log. `UL:<n>` = row n of the usage log.
- `R19S`, `R19P`, `R19M` = the plan-v19 research notes on speed, process, and modes; `IN19 #n` = item n of the plan-v19 inputs list.
- `PBe12`, `PBe13` = the Path B reads at the month-12 end (`m12-p1`) and the month-13 end (`m13-p1`; boxes ep9-x86-m13-end-pathb-1, -2); `PBe13:<line>` = a line of its compare file. `TD4:<line>` = a line of the TD-4 summary (box ep9-x86-m13-td4-1).
- `JDR`, `JDT` = `docs/research/artifacts/m2-month13-alloc-simd/jit-delta-read.md` and `jit-delta-table.txt`.
- `V18-Dn` = a decision of plan v18 = DL row `M13-PLAN-Dn`.
- ep9 paths (`crates/...`, `docs/plan/...`) are at 0b710261 (tag `m13-p1`).
- Upstream cites name the commit: `v8/v8@932ddd7ca4`, `WebKit/WebKit@f30433b25d`, `mozilla-firefox/firefox@ec287ddb04`.

## Not here

The research notes, the inputs list, the compare file, the TD-4 summary and its records, the box logs, the usage log, the null-ledger files, plan v19 and the 2 prepared outward texts stay outside the repo. The HB-4 design is not in this directory. No fuzz input is in this directory.
