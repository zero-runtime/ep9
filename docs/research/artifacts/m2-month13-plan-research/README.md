# The month-13 plan research: the numbers (2026-11-25)

- These are the numbers behind [R25](../../25-m2-month13-plan-research.md): the month-13 lever forecasts of the plan-v18 research against the gate need, the forecasts read against the month-12 end Path B read, the process and A/B form findings, the parser patch track at oxc 0.160.0, and the option paper.
- The files were written by hand from the 3 plan-v18 research notes (speed, process, modes and parser), the plan-v18 inputs list, the A/B form design and its check, the HA-STAGE-1 design and its check, the gate-options paper, the month-12 end compare file and the DL rows. Each finding names its sources. The notes, the compare file and plan v18 are not in the repo; R25 names them by short form.

## Files

| File | What |
|---|---|
| `findings.json` | one object per finding: `id`, `claim`, `status` (verified, derived, estimate, guess, open), `sources`, `numbers` (schema `ep9-research/m2-month13-plan-research/v1`) |
| `levers.tsv` | the lever forecast table of R25 §1.2-§1.4: each month-13 candidate with its A/B gain (low / center / high), jit column, G8, MERGE odds, cost, the round's call and the plan-v18 call |
| `forecast-later.tsv` | the table of R25 §1.9: each forecast of the round, its source and mark, the later data and its source, and the status (confirmed, refuted, open) |

## Cite forms

- `DL <row id>` = a row of `docs/decisions-log.md`. `OL M6-ORCH-<n>` = a row of the orchestrator log.
- `R18S`, `R18P`, `R18M` = the plan-v18 research notes on speed, process, and modes with the parser patch track; `IN18 #n` = item n of the plan-v18 inputs list; `ABF`, `ABC` = the A/B form design and its check; `HA`, `HAC` = the HA-STAGE-1 design and its check; `GOF` = the final gate-options paper.
- `PBe11`, `PBe12` = the Path B reads at the month-11 end (`m11-p3`) and the month-12 end (`m12-p1`; boxes ep9-x86-m12-end-pathb-1, -2); `PBe12:<line>` = a line of its compare file.
- `V18-Dn` = a decision of plan v18; the DL will carry it as row `M13-PLAN-Dn`.
- ep9 paths (`crates/...`, `third_party/...`, `docs/...`) are at 07f91c6d (`m2/m12`); `git diff --stat fcac0863 07f91c6d -- crates third_party tools Cargo.lock` is empty.
- Upstream cites name the commit: `v8/v8@932ddd7ca4`, `WebKit/WebKit@f30433b25d`, `mozilla-firefox/firefox@ec287ddb04`, `bellard/quickjs@e585640ffa`, `oxc-project/oxc@3464a0d181`.

## Not here

The research notes, the inputs list, the design files and their simulation outputs, the compare file, the box logs, plan v18 and the 2 prepared outward texts stay outside the repo. The HB-4 design is not in this directory. No fuzz input is in this directory.
