# The month-13 plan research: the levers left against the gate need, the process and A/B form gaps, the parser patch track, the option paper, and what the month-12 end read did to each forecast

## Run context

| Item | Value |
|---|---|
| Dates | The round: 2026-11-25 02:07-02:34 +0800 (OL M6-ORCH-1512 starts it). The later data: the month-12 end Path B read at 13:14 (OL M6-ORCH-1542) and plan v18 accepted at 14:16 (OL M6-ORCH-1544). This record: 2026-11-25 |
| Round | The plan-v18 research: 3 read-only notes (speed, process, modes and parser) and the plan-v18 inputs list (136 items + an 85-row routes table), with the design inputs the plan used: the month-13 A/B form F1 and its check, the HA-STAGE-1 design and its check, and the gate-options paper. This report collects them and then reads each forecast against the later data. It runs no new box and no new measurement |
| What was not yet known at the round | The L1 merge (tag `m12-p1`), the month-12 end Path B read (on `m12-p1`), the health fuzz run, and plan v18 itself. Every end value in the notes is an estimate (R18S §9, R18P §8) |
| ep9 trees | The notes read 2a792a87 (the M12-P0 head, tag `m12-p0`) and the L1 side branch. `m11-p3` 016a7061 (the month-11 end read); 2c5002c8 (the TD-3 tree); `m12-p1` fcac0863 (the end read tree). Repo cites (`crates/...`, `third_party/...`, `docs/...`) are at this head 07f91c6d (`m2/m12`); `git diff --stat fcac0863 07f91c6d -- crates third_party tools Cargo.lock` is empty |
| Boxes | No box in the round. The later data: boxes ep9-x86-m12-end-pathb-1 and -2 (Cloudflare standard-4, AMD EPYC 9684X, 31 rounds, both builds; 2 x 57 min = 1.90 box-h; OL M6-ORCH-1542) |
| Upstream | `v8/v8@932ddd7ca4` (2026-11-19), `WebKit/WebKit@f30433b25d` (2026-11-19), `mozilla-firefox/firefox@ec287ddb04` (2026-11-19), `bellard/quickjs@e585640ffa` (2026-11-10), `oxc-project/oxc@3464a0d181` (2026-11-16). Each engine cite in this report was read again with `git show <commit>:<path>` for this report (§1.5, §3.5, §3.7) |
| Method | Read only. Every number has a cite. **M** = measured (a box, a CI run, a log, a source read). **D** = derived (arithmetic from cited numbers; the inputs are shown). **I** = inferred. **E** = estimate (basis named). **G** = guess. **O** = open. A forecast is never stated as a result |
| Limits | The notes are forecasts made before the end read. One end read (2 boxes, 31 rounds each), one box type, no gate host. No month-13 box has run, so every month-13 lever forecast is open |

Short forms:
- DL = [docs/decisions-log.md](../decisions-log.md), by row id. OL = the orchestrator log, by row id M6-ORCH-n (not in the repo).
- R18S, R18P, R18M = the plan-v18 research notes on speed, process, and modes with the parser patch track (not in the repo). IN18 = the plan-v18 inputs list (#n = item n; not in the repo).
- ABF, ABC = the month-13 A/B form design (form F1) and its check. HA, HAC = the HA-STAGE-1 design and its check. GO, GOF = the gate-options paper as a draft (2026-11-24) and as final (2026-11-25, after the end read). None is in the repo.
- PBe11, PBe12 = the Path B reads at the month-11 end on `m11-p3` (DL M11-END-PATHB) and the month-12 end on `m12-p1`; PBe12:n = line n of its compare file (not in the repo; its numbers are in [R24](24-m2-month12-offloop.md) §3 and in this report's artifacts).
- Plan v18 is not public yet. Its decisions are cited as "plan v18 row V18-Dn"; the DL will carry each as row M13-PLAN-Dn (not in the DL at this head).
- R23 = [23-m2-month11-levers.md](23-m2-month11-levers.md); R24 = [24-m2-month12-offloop.md](24-m2-month12-offloop.md).
- Ratios: an A/B ratio is t(lever) / t(base), paired per round (below 1 = the lever is faster). A gate ratio is t(node --jitless) / t(ep9) (above 1 = ep9 is faster). "x1.0nn" = a time gain. "Points" = 100 x a ratio difference. "Need" = 1 / the interval low end.

---

## 0. Verdict

```
 forecast (written 02:07-02:34, before the end read)          later data (end read on m12-p1, 13:14)             status
 "about 12% for month 13 IF all 4 TS low ends >= 0.985"       low ends jit 0.961 / 0.960, jitless 0.978 / 0.981   condition FAILED (4 of 4)
 "expect about the month-11 values, +- the box draw"          moves jit -0.2% / +0.5%, jitless +1.3% / +0.8%      confirmed (within 1.3 points)
 K1: "the end read moves 1.5-3 points by box draw"            TS cells -0.2 to +1.3 points                        refuted for TS (smaller)
 "jitless cells: no pass in any case" (from PBe11 low ends)   re-based: center 0.998 / 1.001, high 1.009 / 1.012  refuted (the need fell)
 "jit cells: no pass in any case"                             re-based: high 0.982 / 0.981                        confirmed (1.8-1.9 points short)
 the binding cell is the jit build                            need jit x1.041 / x1.042, jitless x1.022 / x1.019   confirmed (now both jit boxes)
 month-13 lever gains (GC-BUDGET, WIDE FIX)                   no month-13 box yet                                 open
```

1. **The 2 carried levers do not reach the jit need in any case (D).** GC-BUDGET + WIDE FIX give at the Path B x1.012 / x1.020 / x1.032 (jitless) and x1.004 / x1.013 / x1.022 (jit) (R18S §5.1, E). On the PBe12 low ends the jit cells end at 0.965-0.982: 1.8-3.5 points short (§1.3).
2. **The month-12 end read moved the binding cell from jitless to jit (M, D).** Need jit x1.041 / x1.042, jitless x1.022 / x1.019 (PBe12:29, :85). The jitless need fell by the read-to-read move, not by code (R24 §3.3).
3. **The noise measure was registered before the data and read as registered (M).** R18S-D7 put one information line in the end-read row: the move per box on nearly the same VM code. It read jit 0.9976 / 1.0053, jitless 1.0134 / 1.0081 (PBe12:406-409). Plan v18 then set the rule-101 resolve line at ln x >= 0.030 (plan v18 row V18-D3).
4. **The conditional exit chance failed on its condition (M).** R18S wrote "about 12% for month 13 if the month-12 end read puts all 4 low ends at 0.985 or more" (G). All 4 low ends are under 0.985. Plan v18 restates month 13 as about 1% or less (G; plan v18 row V18-D1).
5. **Month 12 lost on the instrument, and form F1 is the fix with a stated price (D).** The 1.010 guard line has 50% power at a +1.0% cost whatever the n; a null family of 10-14 cells fails it in 46-80% of reads. F1 lowers the false GUARD-FAIL to 2.5-16% (24-26% at the measured cell offset) and moves the 50% power point to +1.6% / +2.0% (ABF §1.4). R18P names 7 gaps G-1..G-7 that F1 leaves open (§2.3).
6. **The WIDE-SET guard cost has one cause in the code, and V8 and JSC avoid it by table form (M, source).** W-2 puts a table step in front of `Wide_cold` for 253 of 256 bytes, so an unlisted prefixed op pays one more load and one more indirect branch. The V8 and JSC wide tables hold the final handler for every opcode (§1.5).
7. **DEPS-BUMP-3 at oxc 0.160.0 is a measured re-port, not an offset move (M).** The patch has 3 failed and 3 fuzzy hunks of 36; check sites go 22 -> 25; one fuzzy hunk lands in the wrong function; the PropertyKey port has 4 sites that compile and change results (§3).
8. **The option paper keeps K as the default and records no owner decision (M).** H is the only owner action that can move the date with no change of the gate text; R helps only in its wide reading; S and A give no gain by month 14 (GOF §2-§3; §4).

---

## 1. Speed findings

### 1.1 The need, before and after the end read

| Read | jit low ends (box 1 / box 2) | jitless low ends | Need jit | Need jitless | Binding | Source |
|---|---|---|---|---|---|---|
| PBe11 (`m11-p3`), used by the round | 0.954 / 0.971 | 0.966 / 0.968 | x1.048 / x1.030 | x1.035 / x1.033 | jit box 1 | IN18 §0; R23 §4.1 |
| PBe12 (`m12-p1`), the later data | 0.961 / 0.960 | 0.978 / 0.981 | x1.041 / x1.042 | x1.022 / x1.019 | jit, both boxes | PBe12:29, :85 (D: 1 / low end) |

The medians on PBe12: jit 0.968 / 0.980, jitless 0.985 / 0.988 (PBe12:29, :85). The switch does not fire (PBe12:394; OL M6-ORCH-1542).

### 1.2 The month-13 levers, as predicted by the round (every gain is an estimate)

| Lever | Mechanism | jitless A/B low / center / high | jit A/B | G8 | MERGE odds under F1 (D, model) | Cost (E) | Source |
|---|---|---|---|---|---|---|---|
| GC-BUDGET (arm B1, carried) | fewer minor GCs: the S1 budget grows 5 x the old bytes above 12 MiB (cap 144 MiB) | x1.007 / x1.012 / x1.017 | same (E: GC work does not depend on the tier) | 0 / +1 | 73-86% | 1.0-1.5 agent-days; 2.75-3.05 box-h, expected 3.0-3.6 | R18S §2.2, §4 row 1; ABF §3 |
| WIDE FIX (shape F-a) | W-1 far twins + W-2 with a second prefix for the 3 wide-list ops; `Wide` and `Wide_cold` byte-equal to the base | x1.013 / x1.017 / x1.023 | x1.000 / x1.006 / x1.010 | +1 / +2 | 57-73% if the time cost goes; 46-60% if a part stays | 1.0-1.5 agent-days; 2.5-2.8 box-h expected | R18S §1.5, §4 row 2; ABF §2.4-§2.6; ABC X5 |
| both, A/B | | x1.020 / x1.029 / x1.040 | x1.007 / x1.018 / x1.027 | | about 45-60% (D, as if independent) | | R18S §5.1 |

Bases of the GC-BUDGET estimate (R18S §2.2): Mac TS wall B1 / arm 0 0.9861 (range 0.983-0.987; OL M6-ORCH-1471); x86 bench Ir TS -1.295% (jit) / -1.306% (jitless), runs 45349020669, 45349037862 (DL M12-P1-GC-NOT-RUN); a bound of x1.025 at most from 6 of the 11 x86 TS GCs at the average 0.40% of cycles per GC (R18S §2.2; the per-GC value from R24 §1.2); the Mac count fell 12 -> 6 (OL M6-ORCH-1471). R24 had estimated x1.001 / x1.003 / x1.010 before the arm search; the 2 estimates conflict (R24 §4.1, Q3).

### 1.3 The sum against the gate need

| Case | Path B transfer t | jitless at Path B | jit at Path B | jitless low ends: on PBe11 (R18S) -> on PBe12 (D) | jit low ends: on PBe11 (R18S) -> on PBe12 (D) |
|---|---|---|---|---|---|
| low (both merge) | 0.6 | x1.012 | x1.004 | 0.978 / 0.980 -> 0.990 / 0.993 | 0.958 / 0.975 -> 0.965 / 0.964 |
| center | 0.7 | x1.020 | x1.013 | 0.985 / 0.988 -> **0.998 / 1.001** | 0.966 / 0.983 -> 0.973 / 0.972 |
| high | 0.8 | x1.032 | x1.022 | 0.997 / 0.999 -> **1.009 / 1.012** | 0.975 / 0.992 -> **0.982 / 0.981** |
| center at t 0.2 (the low end of the registered band) | 0.2 | x1.006 | x1.004 | - -> 0.984 / 0.987 | - -> 0.964 / 0.963 |

- Rows 1-3 are R18S §5.1 (E). The PBe12 columns are low end x Path B gain (D). The t 0.2 row: 1.029^0.2 = x1.006, 1.018^0.2 = x1.004 (D; the band 0.2-0.8 of DL M12-PRE-SWITCH).
- **No case passes the jit cells.** The high case leaves them 1.8-1.9 points short. A mid Path B could fire the jit cells only with an A/B product of about x1.052 (D: 1.042^(1/0.8)).
- The other candidates (NL-1 stage 1, MH, GC-2, fusions, JUMP-INVERT, F2, BOLT, HA-STAGE-1, SHAPE-LOOKUP-CACHE, oxc 0.160 parse speed) have centers of x1.005 or less each, or are out of M2 (R18S §4). None is a jit-only gain (I).

### 1.4 The other candidates (R18S §4; estimates)

| Candidate | jitless A/B low / center / high | Evidence | Round's call | Plan v18 |
|---|---|---|---|---|
| NL-1 stage 1 (FI-WORD, 4 -> 3 dependent loads) | x1.000 / x1.005 / x1.012 | line N +3.506 G backend-stall slots; named-load gap +0.244 G (a bound) | not in month 13 (under the floor x1.015) | LEVER-20 pick, expected NOT RUN (V18-D20) |
| MH (stub-cache index with no multiply) | x1.000 / x1.002 / x1.004 | line M read on box 1 only (2.271%) | an arm only, after a read-tool fix | not in month 13 (V18-D24) |
| GC-2 (sweep share; helper policy) | x1.001 / x1.003 / x1.006; x1.000 / x1.002 / x1.006 | line G 4.43% < 6% | month 14 or later | "GC-2 first" rejected (V18-D2) |
| JUMP-INVERT | x1.001 / x1.002 / x1.006 | 417 sites, 18.0 M runs | only after R7 closes W-1 | - |
| F2 handler order | x1.000 (jit x1.000-1.005, G) | trigger: same-round t(jitless)/t(jit) < 0.985 on both boxes | trigger only | not in month 13 (V18-D24) |
| HA-STAGE-1 (S1-A) | x1.000 / x1.000-1.001 after the check | S1-A removes units where ep9 leads node already | not in M2 | not in M2 (V18-D24) |
| BOLT / PGO layout | x1.000 / x1.003 / x1.005 | L1i misses ep9 0.241, node 0.116 per 1k instructions | not in month 13 | closed (V18-D24) |
| 5 fusions | x1.005 for all 5 (D: 117.6 M x 1.04 cycles = 0.12 G) | R23 §5.3 counts at 591064fc | not in month 13 | no fusion lever (V18-D24) |
| SHAPE-LOOKUP-CACHE (new) | x1.000 / x1.002 / x1.005 (G) | `shape::lookup_with` 0.254 G = 1.0% of cycles (`crates/ep9_vm/src/shape.rs:852`) | a Mac call count only | the count SHAPE-LOOKUP-COUNT (V18-D20) |
| oxc 0.160.0 parse speed | at most x1.0006 (an upper bound) | parser instructions -11.2% on Octane (R23 §7.6); `ep9-frontend` 0.50% of samples | rides DEPS-BUMP-3 | DEPS-BUMP-3 (V18-D8) |

### 1.5 WIDE-SET: predicted against measured, and the cause of the guard cost

| Cell (TS, arm / arm 0, aligned) | Predicted | Measured median [interval], n 60 | Source |
|---|---|---|---|
| set, jitless wall (decision cell) | x1.007 / x1.013 / x1.020 | 0.9775 [0.9712-0.9895] = x1.023 | DL M12-P2-WIDE-VERDICT |
| set, jit wall | the same as jitless | 0.9957 [0.9846-1.0019] = x1.004 | same |
| W-2 alone, jit wall | x1.004-1.007 | 1.0035 (a loss) | R18S §1.1 |
| W-2 alone, jit TS branch misses | - | 1.0257 [1.0058-1.0374] | R18S §1.1 |
| guard t0-loop, jit | not priced | 1.0155 [1.0016-1.0245]: GUARD-FAIL | DL M12-P2-WIDE-VERDICT |
| bench Ir of t0-loop (x64) | - | +0.382% jit, +0.380% jitless (run 45328078816): about 1 instruction per executed prefix | ABF §1.1 facts 5, 6 |

Derived (R18S §1.1, D): time per 1% of instructions 1.19 (jitless) and 0.25 (jit); 9.0 cycles per removed far-jump prefix (E: mixes the TD-3 box and the A/B boxes); R23 had priced 13.3.

The cause (M, code): commit d1719606 (the WIDE-SET probe line, head 1ae919cc; not on the month line) writes `Wide_w` into the `Wide` entry of the dispatch table. On `Flow::Cold` it loads `WIDE[next byte]` and makes an indirect tail jump; the `WIDE` table holds a `_w2` handler only for `GetNamedProperty`, `SetNamedProperty` and `JumpLoop`, and `Wide_cold` for every other byte. The base `Wide` handler makes a direct tail jump to `Wide_cold`. So an unlisted prefixed op (t0-loop runs a prefixed `BitwiseAndSmi`) pays one more load and one more indirect branch (R18S §1.3).

| Engine | How a wide prefix dispatches | Cite (verified at the named commit) |
|---|---|---|
| V8 Ignition | `IGNITION_HANDLER(Wide) { DispatchWide(OperandScale::kDouble); }`; `DispatchWide` loads the next byte and jumps through the scaled table, which holds a handler for every bytecode | `v8/v8@932ddd7ca4` `src/interpreter/interpreter-generator.cc:3514-3516`, `src/interpreter/interpreter-assembler.cc:1433-1458` |
| JSC LLInt | `nextInstructionWide16` loads the opcode byte and jumps through `opcodeMapWide16` | `WebKit/WebKit@f30433b25d` `Source/JavaScriptCore/llint/LowLevelInterpreter.asm:401-404` (R18S wrote :405-409) |
| ep9 W-2 | the same 2 steps, but for 253 of 256 bytes the target is `Wide_cold`, which tests the opcode again | d1719606 (R18S §1.3) |

Lesson (I): a table step pays only when the entry is the final handler. Shape F-a keeps the old `Wide` path byte-equal and adds a second prefix only for the 3 listed ops; its gate adds G1b (the disassembly of `Wide` and `Wide_cold` byte-equal to the base, and 0 runs of a new handler on t0-loop) to G1-G5 (R18S §1.5; plan v18 row V18-D19).

### 1.6 Can the 1.010 guard line tell +1% from noise? (R18S §1.4; D)

| Read | SE of the median | P(median > 1.010) at a true 1.000 | at +1.0% | at +2.0% |
|---|---|---|---|---|
| 1 box, n 31 | 0.0086 | 12% | 50% | 88% |
| 2 boxes, n 60 / 62 | 0.0061 | 5% (10% with the cell offset 0.0049) | 50% | 95% |
| a family of 10 / 14 null cells | - | 46-68% / 59-80% false GUARD-FAIL | - | - |

- The line sits at the effect it must find, so a true +1% cost passes half of the reads at any n.
- 80% power at +1% with 5% false alarm on one cell needs about 142 pairs (4.6 boxes), or about 71 pairs with 2 runs per arm and round (D).
- F1 does not fix the power. It lowers the false alarm and moves the 50% point to +1.6% (LEVER cell) / +2.0% (IDENTITY cell) (ABF §1.4). A real +1.5% cost on one LEVER cell passes about half of the reads (18-68% by row) (ABC X4).

### 1.7 The noise measure registered before the data (R18S-D7)

R18S-D7: "Before the month-12 end Path B data, write one report line in its row: the move per box against PBe is a read-to-read estimate on (nearly) the same VM code." The compare file printed it as S4, information only (PBe12:406-409).

| Cell | Box 1: PBe11 -> PBe12 (ratio) | Box 2 | Same sign |
|---|---|---|---|
| TS jit | 0.970 -> 0.968 (0.9976) | 0.974 -> 0.980 (1.0053) | no |
| TS jitless | 0.972 -> 0.985 (1.0134) | 0.980 -> 0.988 (1.0081) | yes |

- The diff between the 2 reads is the lock moves, the parser check and test code; the runtime VM and heap source did not change (R24 §3.2; PBe12:381).
- Box 1 jitless read 0.985 on `m12-p1`, above its own `m11-p3` interval 0.966-0.982: the within-read interval does not hold the read-to-read move (D; plan v18 row V18-D3).
- What plan v18 did with it (V18-D3): no change to the switch row and no confirm box; rule 101 prints a transfer only when ln(x) >= 0.030, about 2.3 x the largest measured move (ln 1.0134 = 0.013, D). At the center (jitless x1.029, ln 0.029) the transfer reads NOT RESOLVED (D).

### 1.8 The exit chances as written (every value is a guess)

| Source, time | Month 13 | Month 14 | By month 15 | Basis named by the source |
|---|---|---|---|---|
| DL M12-PRE-MONTH13 (2026-11-24) | about 15% | about 35% | - | the Wide set carries; the off-loop part gives 1.5% or more |
| R18S §5.3 (02:31) | about 3% | about 12% | about 25% | 0 levers merged; the identified list sums to about x1.012 A/B |
| R18S §5.3, conditional | about 12% if all 4 low ends of the end read are >= 0.985 | - | - | a box draw like TD-3 |
| GOF, option K (after the end read) | about 3% | about 12% | about 25% | the R18S values |
| plan v18 row V18-D1 (14:16) | about 1% or less | the pre-registered fallback, no estimate | no estimate until JIT-DELTA prices a jit lever | both jit cells 1.8-1.9 points short at the high case |

### 1.9 Forecast -> later data

| # | Forecast (source) | Later data | Status |
|---|---|---|---|
| FC1 | month 13 about 12% if all 4 low ends >= 0.985 (R18S §5.3, R18S-D8; G) | low ends jit 0.961 / 0.960, jitless 0.978 / 0.981 (PBe12:29, :85) | condition failed on 4 of 4; the 12% does not apply |
| FC2 | month 13 about 3%, month 14 about 12%, by month 15 about 25% (R18S §5.3; G) | plan v18 row V18-D1: month 13 about 1% or less; no month-14/15 estimate | open (a guess); superseded |
| FC3 | the end read gives about the month-11 values with the box spread (IN18 #2; R18S §0) | jit -0.24% / +0.53%, jitless +1.34% / +0.81% (PBe12:396-397) | confirmed (within 1.3 points) |
| FC4 | K1: the end read moves 1.5-3 points by box draw (R18S §8) | TS cells -0.2 to +1.3 points; items 2 and 6 +2.6 to +5.5 points (R24 §3.3) | refuted for the TS cells (smaller); above the range on items 2 and 6 |
| FC5 | jitless cells do not pass in any case (R18S §5.1, from PBe11) | re-based on PBe12: center 0.998 / 1.001, high 1.009 / 1.012 (§1.3, D) | refuted: the jitless need fell to x1.019-1.022 |
| FC6 | jit cells do not pass in any case (R18S §5.1) | re-based: high 0.982 / 0.981 (§1.3, D) | confirmed: 1.8-1.9 points short |
| FC7 | the jit build is the binding build; the box-1 jit low end binds (R18S §0, §5.1) | need jit x1.041 / x1.042 against jitless x1.022 / x1.019 | confirmed; both jit boxes now bind |
| FC8 | no mid Path B can fire (R18P §1.3 row N4; IN18 #5) | the jit cells need about x1.052 A/B at t 0.8 (D) | confirmed (no case reaches it) |
| FC9 | the end Path B box dies at build without the oxc sync (R18M §0, X9) | `fork_attest regress=pinned icu4x=pinned oxc=pinned` on both boxes (PBe12:10, :15, :393; OL M6-ORCH-1542) | did not happen: the fix R18M-D4 was applied before the box |
| FC10 | the end read costs 1.75-2.10 box-h (R18P §1.1 row 6) | 2 x 57 min = 1.90 box-h (OL M6-ORCH-1542) | confirmed |
| FC11 | the month uses 6.36-6.71 box-h with the health run (R18P §0) | 3.53 + 1.90 = 5.43 before the health run (R24 §6); health run est. 1.08, not read here | open (5.43 + 1.08 = 6.51 if the estimate holds, E) |
| FC12 | F2 trigger: same-round t(jitless)/t(jit) last 0.997 / 0.995 (R18S §4 row 7) | 0.982 [0.968-0.995] / 0.997 [0.981-1.014] (PBe12:363) | not fired (box 1 only below 0.985) |
| FC13 | GC-BUDGET x1.007 / x1.012 / x1.017; WIDE FIX x1.013 / x1.017 / x1.023 (R18S §4) | no month-13 box | open |
| FC14 | MERGE odds GC 73-86%, WIDE FIX 57-73% (ABF §2.4, §3) | no month-13 box | open |
| FC15 | L1 moves TS wall by under 0.01% (IN18 #2; R18S §4) | not separable in the end read (no paired read) | open |
| FC16 | the R18S-D7 A/A line is printed as information only (a rule, not a forecast) | printed as S4; no verdict used it (PBe12:406-409) | done as registered |

Rows are in [forecast-later.tsv](artifacts/m2-month13-plan-research/forecast-later.tsv).

---

## 2. Process findings

### 2.1 Box use by kind, month 12

| Kind | Runs | box-h | Share | Source |
|---|---|---|---|---|
| lever A/B (WIDE-SET, 85 + 72 min) | 2 | 2.62 | 48% | R18P §1.1; DL M12-P2-WIDE-VERDICT |
| cause read (TD-3, 26 + 28 min) | 2 | 0.91 | 17% | R18P §1.1; DL M12-PRE-TD3-READ |
| Path B (the end read, 57 + 57 min) | 2 | 1.90 | 35% | OL M6-ORCH-1542 |
| fuzz (the health run) | 1 | est. 1.08, not read here | - | DL M12-FUZZ-H |
| **sum before the health run** | 6 | **5.43 of 15.9** | | R24 §6 |

- At the round the sum was 3.53 box-h: lever A/B 74%, cause read 26% (R18P §1.1).
- WIDE-SET used 2.62 box-h against a plan of 1.6-2.0: 3 binaries x 5 arms on box 1; 143 s per round with 5 arms, 111 s with 4; setup 14-16 min (R18P §1.1).
- WIDE box 1 ran 29 read rounds, not 31: the box timer did not fit 1 + 31 rounds with 5 arms; the box set warned at +16 min and nothing could act on it (R18P §1.1).
- R18P's month-13 sums (E): planned 8.18-10.30 box-h; with the expected third boxes 8.53-11.22; a third lever fits only if the first two used no retry. The cap 15.9 holds; no raise (R18P §1.3). Plan v18 kept the cap and priced a lost box at 3.41 box-h (plan v18 row V18-D10).

### 2.2 Unit costs for month 13 (R18P §1.2)

| Unit | box-h | Mark |
|---|---|---|
| 2-box same-binary read, 4 arms each | 2.40 | M (WIDE box 2) |
| the same with arm 0' on box 1, sized for 1 + 31 rounds | about 2.75 | E |
| one more box | 1.07-1.35 | M (1.20) / E |
| 2 runs per arm and round on one LEVER row | 0.07-0.11 per row per 2-box read | D from one round split |
| 2-box Path B | 1.75-2.10 | M (month 11); 1.90 at the month-12 end |
| TD-form cycle read | 0.44-0.47 | M |
| health run, 1 campaign | 1.08-1.09 | M (month 11) |

### 2.3 Form F1 and the 7 gaps it leaves open (R18P §1.4)

| Gap | What it is | Why it matters | R18P's text for the row |
|---|---|---|---|
| G-1 | the pair definition with 2 runs per arm (O16) | with 2 runs the pair can be a ratio of sums, of means or a median of 2 ratios; each gives another level | the round value of an arm is the mean of its 2 runs; n counts rounds |
| G-2 | O16 on a REPEATED READ row | 2 runs make the failed t0-loop cell less noisy: a softer reading of the row that failed | O16 never applies to a REPEATED READ row |
| G-3 | the box timer for 1 + 31 rounds | WIDE box 1 ran 29 read rounds; a GC box 1 with an A/A arm, 7 guard rows and O16 nears the destroy line | timer = setup + 32 x the dry-run round time x 1.05 + 150 s, set before box 1 |
| G-4 | the x86-64 GC count run (O7) | without it the 12 IDENTITY candidates have Mac counts only | build a count mode (0.3-0.5 agent-day) or write "no x86 count = LEVER" |
| G-5 | no octane-splay jitless bench row | PROOF (b) has no reading, so that cell is LEVER | add the row (0 box-h) |
| G-6 | the knee reading (O17) | `--gc-stats` prints collections, freed cells and allocated bytes only | read old bytes from the trace binary; no reading = LEVER |
| G-7 | the null-ledger place (O11) | the read code is fixed by its sha256 at box start | the read prints the ledger lines; a records writer appends them to one file in `docs/measurements/` |

- F1 in one line (ABF §1.2-§1.3): a guard cell is a LEVER cell unless the fill row proves IDENTITY before box 1 (bytecode sha256 equal, bench Ir within 1e-5, and a count run of the lever surface); STRONG (median above 1.010 and the Bonferroni pair above 1.000) = GUARD-FAIL; WEAK on a LEVER cell = ONE MORE BOX.
- The cell offset (tau) of the 42 null cells is about 0.0057 (95%: 0.0028-0.0092); at 0.0057 F1 gives 24-26% false GUARD-FAIL, at 0.0092 about 56% (ABC X3). That is above the 20% line of the new rule "instrument first"; plan v18 kept F1 and recorded the finding (plan v18 row V18-D5).
- Under F1, read 1 of WIDE-SET would most likely have read ONE MORE BOX, never MERGE: the new form changes the month-12 word in the lever's favour (ABF §1.3; ABC X1). The REPEATED READ line exists for that reason.

### 2.4 The 5 largest idle waits of month 12 (R18P §2.2; D from the OL times)

| # | Wait | Length | Why | What could have run | Source |
|---|---|---|---|---|---|
| 1 | box lane: WIDE read 16:52 -> no box at 02:07 | >= 9.2 h | by decision: GC NOT RUN; the end read waits for the last tag, which waits for the L1 merge | the end read on `m12-p0` and L1 as the month-13 base (a K9-form row) | OL M6-ORCH-1481, -1484, -1510 |
| 2 | box lane: TD-3 end 10:14 -> WIDE boxes 15:24 | 5.2 h | the WIDE rows and the fill ran after the probe; a commit-text gate miss and a lint defect; the GC arms failed the Mac preview | draft the rows and the box set from the design, in parallel with the probe | OL M6-ORCH-1461 .. -1474 |
| 3 | plan accept 06:48 -> first box 09:15 | 2.45 h | the TD-3 row landed after the month-11 records push; a lander stopped by the weekly usage limit | land the pre-registration rows first | OL M6-ORCH-1445 .. -1452 |
| 4 | merge head red CI 23:52 -> green 02:00 | 2.1 h | a script test read the old step text; the fixer ran actionlint only | the script tests in the first brief | OL M6-ORCH-1501, -1504, -1510 |
| 5 | STACK-FORMS part 2 waits for a Windows cell 13:16 -> 15:09 | 1.9 h | an old paint helper wrote below the Windows guard page | run a new CI step once on the 4 targets at the base | OL M6-ORCH-1468, -1469, -1473 |

### 2.5 The review-coverage gap

| Finding | Evidence | Source |
|---|---|---|
| A second-model review could not read parser-stack code | 3 stops by its service filter in one day (STACK-FORMS twice, L1 once); one blocked run cost about 1 h and left 831 MB and 5 processes; the ADR text slice passed with findings | OL M6-ORCH-1479, -1493; DL M12-P0-LEDGER, M12-P1-LEDGER |
| Code ranges without a review round | M12-P0: 5 code ranges had no review round (logged after the merge); L1: the gap after review round 1 was found before the merge, and round 2 then found the HIGH defect A2-1 | OL M6-ORCH-1515, -1527; R24 §5.3, §6 |
| The round's proposal | the reviewer of record set per subject before the review: parser-stack, recursion-guard and stack-limit code -> 2 Claude reviewers + 1 verifier, no second-model try; the MERGE row names the reviewer of record and why (R18P-7) | R18P §2.1 |
| What plan v18 took | a review-coverage rule for every changed non-docs file (rule 106) and the reviewer of record by subject; the second model on every non-parser code slice, with the filter-stop fallback | plan v18 row V18-D12 |

### 2.6 Records load and tokens

| Item | Value | Source |
|---|---|---|
| DL rows of month 12 (at 2a792a87) | 50 rows, 173,068 characters = 3,461 per row (month 11: 2,758) | R18P §5.1 |
| the 2 largest rows | GC-BUDGET PREREG 21,775 and WIDE-SET PREREG 15,012 characters; each restated every generic line of DL M12-PRE-AB93 word for word | R18P §5.1 |
| lever rows with no merge | WIDE-SET 6 rows, 47,535 characters; GC-BUDGET 2 rows, 26,341 | R18P §5.1 |
| M12-P0 integration | 5 workflows, 10 agents, 1.41 M tokens, 3.6 h + 2.1 h of CI rework; 2 of the 4 stages were rework | R18P §5.2 |
| tokens at the round | 22.94 M in 42 workflows and 114 agents: engine roles 30%, design and research 25%, rule-96 rows and box sets 20%, integration and records 16%, Claude review 5%, close and MODES 4% (I: class by reading the kind text) | R18P §2.3; IN18 #110 |
| tokens of the levers that did not merge | about 6.5-6.9 M + 2.62 box-h | R18P §2.3 (D) |
| tokens of the month (later) | 32.03 M | plan v18 row V18-D23 |
| the round's smaller form | cite the generic row by id + the sha256 of its line; 3 rows for a no-merge lever, 5 for a merge; one integration workflow; checks only on rule-96 rows, Frozen ADR rows and one phase-close fact check | R18P §5.3; plan v18 row V18-D13 |

### 2.7 Rules proposed by the round and what plan v18 took

| Proposal | Text in short | Plan v18 |
|---|---|---|
| instrument first | print a line's modelled false-alarm rate from the null ledger before its first lever box; above 20% is a plan finding | taken as rule 104 (V18-D14) |
| odds before box | each FILL row prints the modelled odds of each word; a guard Ir change above 1e-5 with MERGE odds under 50% runs a 0-box FIX gate first | taken as rule 105 (V18-D14) |
| F1 with G-1..G-7 closed | §2.3 | taken (V18-D5) |
| brief rules | script tests, speed rows at the first ready push naming the pass measured, class grants from a scratch merge, one target dir per worktree, detached pushes, the month PR at the line start, a new CI step run once on 4 targets | taken (V18-D15) |
| approved rows byte-frozen | an approved Approval row is never edited, also not for a line number | taken (V18-D13) |
| no Mac gate unit in a branch-N month | units run only on the exit candidate, in the batch hour | taken (V18-D21) |

---

## 3. Modes and parser findings

### 3.1 The patch drift per oxc release (R18M §1.2; M: `patch --dry-run` on the registry sources)

| oxc_parser | Distance from 0.158.0 | Hunks clean | Offset only | Fuzz | Failed | Failed files |
|---|---|---|---|---|---|---|
| 0.154.0 | -4 | 17 | 12 | 3 | 4 | arrow, binding, expression, jsx |
| 0.156.0 | -2 | 19 | 11 | 3 | 3 | arrow, expression, jsx |
| 0.157.0 | -1 | 25 | 9 | 1 | 1 | `js/arrow.rs` |
| 0.159.0 | +1 | 10 | 26 | 0 | 0 | - |
| 0.160.0 | +2 | 5 | 25 | 3 | 3 | `js/arrow.rs`, `jsx/mod.rs`, `lib.rs` (`ParserImpl::new`) |

The patch is `third_party/patches/oxc/0002-parser-recursion-check.patch` (36 hunks, 13 files; R18M §1.1); `third_party/patches/oxc/README.md:33` gives its site count at 0.158.0.

### 3.2 DEPS-BUMP-3 at oxc 0.160.0: 22 -> 25 check sites

| Group | 0.158.0 -> 0.160.0 | What the re-port does |
|---|---|---|
| 17 class-A sites | 16 apply by offset; `parse_jsx_element` fails | place one by hand |
| B1, B2 (cover walks) | apply | none |
| B3 `cover_binding_as_expression` | the function is gone (the async arrow path was rewritten) | drop the hunk |
| B4, B5 (loops) | apply | none |
| B6-B9 (new) | 4 new recursion groups in the cover-to-binding code; 2 of them in `&self` functions | 4 new sites + a `&self` stop helper |
| sum | 22 sites (20 checks + 2 loops) -> **25 sites (23 checks + 2 loops)** | the site tool on the new call graph first |

Source: R18M §1.2, confirmed by the design's call-graph script on the saved graphs of 0.158.0, 0.159.0 and 0.160.0 (R18M §8). The pin for 0.160.0 is 87e38f32, the vcs commit of the crates.io package (R18M §8).

Two traps (M, R18M §0):
- Plain `patch` places the `parse_expression` exit hunk inside a token-edit loop of 0.160.0 `lib.rs` and reports "succeeded with fuzz 2". The right place is `parse_expression_impl`. `scripts/sync-lib.sh` uses `git apply` (no fuzz), so CI is safe; a hand port must use a 3-way rebase.
- The PropertyKey port (next table).

### 3.3 The PropertyKey port: 4 silent sites

| Kind | Sites at this head | Why |
|---|---|---|
| silent (compiles, changes results) | `property_name` `crates/ep9_frontend/src/bytecodegen/literal.rs:111-125` (ends in `_ => None`); `property_key_into` `crates/ep9_frontend/src/bytecodegen/expr.rs:861-878` (`key.as_expression()` else `self.unsupported("private names", ..)`); `crates/ep9_frontend/src/bytecodegen/class_fields.rs:226-236`; `crates/ep9_frontend/src/bytecodegen/pattern.rs:197-200` | at 0.160.0 a non-computed string, number or bigint key is a new `PropertyKey::Static*Literal` variant, not an `Expression`; so `({"a":1})` would give an "unsupported" compile error with no compiler warning (I, from the enum form; R18M §1.2) |
| loud (compile error) | R18M counts 22 uses of `.computed` that 0.160.0 removes from 6 + 2 AST structs | a plain `grep '\.computed'` over `crates/ep9_frontend/src` gives 27 lines at both 2a792a87 and this head; the struct-by-struct count was not done again here |
| the gate that sees the silent kind | the corpus digest equal and the test262 parse lists equal (DL M12-PLAN-D18) | a wildcard arm hides the change from the compiler, not from the digest |

Plan v18 added a key-form fixture against node and a stop line S9 for this port (plan v18 row V18-D8).

### 3.4 The DEPS-BUMP-3 steps and stop lines (R18M §1.3; summary)

| Step | Who | Content |
|---|---|---|
| 0 | orchestrator | fetch the pin; the package `src/` and the pin's parser `src/` must diff empty |
| 1-2 | role | move the patch branch by a 3-way rebase (never `patch` with fuzz); the 4 new sites; make the packaging patch 0001 again |
| 3-4 | role | the site tool on the new checkout first (25 sites, each needed); the patch tests + one case per new site |
| 5 | role | the PropertyKey port: the 4 silent sites first (exhaustive matches), then the loud sites |
| 6-8 | CI | T-HOLES and the small-stack tests on 4 targets x 2 profiles; the stack unit lines (debug worst <= 13,208 B, release unit <= 7,074 B; DL M12-L1-START); corpus digest and test262 parse lists equal; BENCH-SET floors |
| 9 | orchestrator (Mac) | parse instructions of 0.160.0 + L1 against 0.158.0 + L1; the slow-class rows |
| 10 | review | the second model on the slices with no parser code; 2 Claude reviewers + a verifier on the re-port |
| 11 | records | the pin fills, the patch README site count, a bump-hours line |

| Stop line | Hold oxc at 0.158.0 when |
|---|---|
| S1 | the step-0 diff is not empty |
| S2 | the re-port needs a change in a second oxc crate (the patch-count rule, gate P1: `docs/plan/01-front-end.md:168`) |
| S3 | the site tool finds a cycle that no parser site can cut |
| S4 | a signal in T-HOLES or the small-stack tests after one fix round |
| S5 | a stack unit line is passed on any target |
| S6 | the corpus digest moves after one fix round of the PropertyKey port |
| S7 | parse instructions above 0.158.0 + L1 x 1.010 (0.160.0 is a speed release: a rise means a broken site) |
| S8 | the re-port takes more than 1.5 agent-days |

Re-port cost (E, R18M §1.2): 0.25 agent-day for a step like 0.158 -> 0.159; 0.75-1.0 for a step like 0.158 -> 0.160; review about 0.15-0.3 M tokens a bump. Per calendar month (2 bumps) 0.5-2.0 agent-days; per year 6.5-26 agent-days against gate P2 "measured work for oxc version bumps > 1 PM per year (est.)" (`docs/plan/01-front-end.md:169`). The high end can trip P2, so the round asked for a bump-hours ledger (R18M-D5); plan v18 took it (V18-D8).

### 3.5 How others bound parser recursion, and the upstream path

| Parser | Form | Cite |
|---|---|---|
| V8 | `CheckStackOverflow()`: stack position < `stack_limit_` -> `set_stack_overflow()` | `v8/v8@932ddd7ca4` `src/parsing/parser-base.h:942-945`; call sites :2109, :2126 (verified) |
| JSC | `canRecurse()` = `m_vm.isSafeToRecurse()` | `WebKit/WebKit@f30433b25d` `Source/JavaScriptCore/parser/Parser.h:1909` (verified) |
| SpiderMonkey | `AutoCheckRecursionLimit`, 11 uses | `mozilla-firefox/firefox@ec287ddb04` `js/src/frontend/Parser.cpp` (grep count 11, verified) |
| QuickJS | `js_check_stack_overflow`: stack pointer - alloca size < `rt->stack_limit`; no check when `CONFIG_STACK_CHECK` is off (Emscripten) | `bellard/quickjs@e585640ffa` `quickjs.c:2076-2099` (verified) |
| data parsers (serde_json, regex-syntax, toml_edit) | a depth counter with a fixed default | crates.io sources (R18M §1.4; not verified here) |
| ep9 (built) | an opt-in `ParserConfig` hook; the embedder supplies the compare; oxc gets no stack code and no thread-local | `third_party/patches/oxc/0002-parser-recursion-check.patch` |

The oxc contributor policy asks for an AI disclosure with the model name and version and a human contributor who takes full responsibility (`oxc-project/oxc@3464a0d181` `AGENTS.md:7-16`, verified). R18M found the prepared upstream text stale: it proposes a run-time option field, not the built hook (R18M §1.4). **2 outward texts were prepared for the owner (an upstream request and a report); not sent.** The upstream request is a user call, offered in the month-12 status and again after DEPS-BUMP-3 (plan v18 row V18-D9).

### 3.6 The G8 cap and the SAFETY allowance

| | Month 12 (DL M12-PRE-G8) | R18M proposal (R18M §2.2) | Plan v18 (V18-D6) |
|---|---|---|---|
| cap | +4: STACK-FORMS 1, WIDE-SET 2, GC-BUDGET 1; "No reserve" | +6: WIDE FIX 2, GC-BUDGET 1, the pick 1, SAFETY 2 (+2 if L1 is carried; +1 if the month holds 2026-12-08) | +7: lever rows +4, DEPS +1, SAFETY +2 |
| SAFETY | none | +2, only for a confirmed class of process abort, memory-safety defect or security finding, by its own DL row before the role | +2 as a purpose row, with 3 limits; a stated change against "No reserve" |
| SWAP | a new pick may spend a freed row by a SWAP row (L1 used it: DL M12-L1-START) | no lever SWAP | SWAP only among the lever rows; the R18M "no lever SWAP" line rejected (the pick would need a raise) |
| the reason | - | the month-12 swap tied a safety fix to a lever failure: had WIDE-SET merged, L1 would have waited a month | taken for SAFETY; rejected for SWAP |

### 3.7 The modes room and the MODES-CHECK items

| Candidate | Narrows T1 / AOT / hybrid? | Reason (R18M §3.2) |
|---|---|---|
| WIDE FIX | no | far jumps keep the offset in the stream; W-2 and the fix are T0 handler-table code; must-show: ABI 7 -> 8 |
| GC-BUDGET | no | heap policy constants; one GC and one barrier for every mode |
| NL-1 stage 1, GC-2, JUMP-INVERT | no | no frame or ABI change |
| HA-STAGE-1 (not in M2) | S1-A widens; S1-C narrows a little | the frame does not change; the register roles are cheap to change only before T1 text exists (HAC fix 10) |
| L1, DEPS-BUMP-3, FUZZ-SMALL-STACK | no | one front end for every mode; a stack-limit error at AOT build time is a build error, never a unit |

Baseline-tier evidence (verified): V8 Sparkplug `VisitJumpConstant() { VisitJump(); }` and `VisitWide` / `VisitExtraWide` are `UNREACHABLE` because the shared iterator consumes the prefix (`v8/v8@932ddd7ca4` `src/baseline/baseline-compiler.cc:2726`, `:3077-3085`); `class BaselineFrame : public UnoptimizedJSFrame` (`src/execution/frames.h:1294`); JSC Baseline reads each instruction from the stream (`WebKit/WebKit@f30433b25d` `Source/JavaScriptCore/jit/JIT.cpp:500`). So an operand width or a jump form does not narrow a baseline tier; a frame change does (I).

MODES-CHECK must-show rows proposed for month-13 tags (R18M §3.3): the 4 versions (ABI, FACT_SCHEMA, IC_LAYOUT, CONTAINER); the corpus digest; the oxc pin, patch count and site count; G8 spent / cap and the SAFETY state; at the WIDE FIX tag the ABI 8 note; at a DEPS-BUMP merge the 0.160.0 pin and 25 sites. Open items from w57 (IN18 #88): MC-w57-2 (on wasm32 every parse is StackUnavailable today; M3), MC-w57-3 (a per-port JS-visible limit), MC-w57-5 (the AOT build-error rule). The w58 check ran at `m12-p1` (DL M12-P1-W58); plan v18 puts w59 at tag `m13-p0` or by 2026-12-02 (V18-D16).

### 3.8 Fuzz and security

| Item | Round's call | Plan v18 |
|---|---|---|
| FUZZ-SMALL-STACK | build in month 13: a cargo-fuzz target on a 2 MiB worker, entry under `cfg(fuzzing)` (G8 0), on the GitHub fuzz job, 0 boxes (R18M §1.7) | a ride role of M13-P0 (V18-D22) |
| health run M12-FUZZ-H | campaign 1 only (jit + GC stress) on the last tag; it must cover the patched parser (R18M §5) | DL M12-FUZZ-H; campaign 2 only after an ABI, opcode or REPRL merge (V18-D11) |
| slow parse classes (memory grows with depth squared) | a site-entry work budget, design now, build at M3 (R18M-D8) | rejected for month 13; stays at M3 (V18-D22) |
| layer 0 | a cheap first filter only; a proof test with layer 0 off (R18M-D7) | - |

HB-4: a known weakness, already public in [docs/research/20](20-bestfit-data-structures.md) (:232) and DL DEPS-BUMP-2; its design stays out of this record.

---

## 4. The option paper (GOF; no user decision is recorded)

| Option | What it costs | Who must act | Chance M2 exits (G) | Main risk |
|---|---|---|---|---|
| **K: keep the cell, go on (default)** | month 13 as planned: 8.3-10.3 of 15.9 box-h, about 24-32 M tokens (E) | nobody (the orchestrator runs the plan) | month 13 about 3%; by month 14 about 12%; by month 15 about 25% (GOF); plan v18: month 13 about 1% or less | the named gains do not reach the jit low ends in any case |
| H: a bare-metal x86-64 gate host | price and delivery not known; set-up 1-2 agent-days (G); 0 Cloudflare box-h; read it 2 times or more | the owner buys or rents one bare-metal Linux x64 machine (performance governor, turbo off, SMT sibling off, isolated cores) | with a host read by the month-13 end: about 40-60% if the host is the model that read above 1.0 on shared VMs (EPYC 7763); else the K values | the host may read lower; the 7763 sign changed between 2 commits |
| R: adopt the registered option-R text | 1 DL row; 0 build; 0 box-h | the owner says "adopt R"; the orchestrator writes the row (DL M11-PRE-OPTION-R) and must settle 2 open points (does the bound read the jit half of item 3; median or interval) | narrow reading: the K values; wide reading: month 13 about 15%, by month 14 about 35% | on box 1 the interval of t(jit)/t(jitless) reaches 1.033, above the 1.03 bound |
| S: GC S2b ahead of M5 | 6-12 PM; about 80-160 agent-days (D, factor 2) | the owner approves a plan change | no gain by month 14 | concurrent-GC defects before the counted fuzz |
| A: handler ABI stage 1 in M2 | 12-19 agent-days, 4-6 box-h, G8 +3 | the owner reverses V16-D8 and gives a ruling for a feature-gated ABI | the K values: the design prices the stage at x1.000-1.001 and says do not build in M2 (HA §0; HAC fix 2) | a new layout draw for every handler |

- What changed from the draft to the final paper (GOF §3): the binding cell moved to the jit build; R moves up only in its wide reading; A moves to the last place; H is unchanged in kind; the K chances are lower.
- DL M12-PRE-MONTH13 line (3) holds: the status restates the option paper with K the default. Plan v18 adopts none of H, R, S, A (V18-D4). The paper asks no question.

---

## 5. Where plan v18 used the round (decision ids)

| Plan v18 row (DL M13-PLAN-Dn) | Takes from the round | Changed from the round |
|---|---|---|
| V18-D1 | the switch row in the M12-PRE-SWITCH form; month 14 the fallback | the R18S 3% / 12% / 25% and the conditional 12% are replaced by "about 1% or less" and no month-14/15 estimate |
| V18-D2 | GC-BUDGET first, then WIDE FIX (F-a), then the pick (R18S-D2..D6) | lines (1) and (2) of DL M12-PRE-MONTH13 overridden with the new facts |
| V18-D3 | the R18S-D7 A/A line | the rule-101 resolve line ln x >= 0.030; no confirm box |
| V18-D5 | F1 with G-1..G-7 closed (R18P-3) | the rule-104 finding (24-26%) kept open with reasons |
| V18-D6 | the SAFETY +2 row (R18M-D10) | cap +7 and SWAP among lever rows (R18M: +6, no SWAP) |
| V18-D8 | the 11 steps and the stop lines (R18M-D2..D5) | a key-form fixture and stop line S9; DEPS-BUMP-3 its own set, carried to month 14 |
| V18-D12 | the reviewer of record by subject (R18P-7) | the review-coverage rule 106 |
| V18-D17 | A1H-JOIN after L1 (R18M-D11) | "never in a month with WIDE FIX" taken only as "no merge between the WIDE-FIX box start and its tag" |
| V18-D22 | FUZZ-SMALL-STACK in M13-P0 (R18M-D9) | the slow-class work budget stays at M3 (R18M-D8 rejected for month 13) |
| V18-D28 | R25 into the repo with local paths and the HB-4 detail stripped (R18M-D13, R18S-D11) | - |

---

## 6. Verified, not verified, limits

### 6.1 Verified

- The PBe12 values of §1.1, §1.7 and §1.9 against the compare file (PBe12:10, :15, :29, :85, :363, :381-409) and OL M6-ORCH-1542; the 4 low ends are under 0.985, so the FC1 condition failed.
- The derived values of §1.3: the re-based low ends (low end x Path B gain), the t 0.2 row, the x1.052 mid-read need; the needs as 1 / low end.
- The R18S, R18P and R18M values copied here against the notes; the A/B form numbers against ABF and ABC; the HA verdict against HA §0 and HAC fixes 1-2; the option table against GOF §2-§3.
- Every engine cite of §1.5, §3.5 and §3.7 with `git show <commit>:<path>` in the named upstream commit. One correction: the JSC `nextInstructionWide16` macro is at `LowLevelInterpreter.asm:401-404`, not :405-409 as R18S wrote.
- The ep9 cites at this head: `literal.rs:111-125` (`_ => None`), `expr.rs:861-878` (`unsupported("private names")`), `class_fields.rs:226-236`, `pattern.rs:197-200`, `shape.rs:852` (`lookup_with`), `third_party/patches/oxc/README.md:33`, `third_party/oxc.commit` (74e5e665), `docs/plan/01-front-end.md:168-169`.
- The OL row ids: each orchestrator-log line cited by the notes as a line number was mapped to its M6-ORCH id and read.
- The commits d1719606, 1ae919cc, 5ca1610c, dabc0e85 exist in the repo object store; d1719606 is on the pushed probe branches, not on the month line.

### 6.2 Not verified

- Every month-13 lever gain and MERGE odds value (no month-13 box has run).
- The exit chances (all guesses), in every source.
- The patch drift table of §3.1 and the 22 -> 25 site count: R18M's dry runs and its call-graph script, not run again here.
- The loud `.computed` count of 22 (a plain grep gives 27 lines; the struct-by-struct count was not done).
- The "{"a":1} gives an unsupported error at 0.160.0" claim: inferred from the enum form; no build at 0.160.0.
- The data-parser cites of §3.5 (serde_json, regex-syntax, toml_edit, stacker): crates.io sources, not engine clones.
- The token class split of §2.6 (one reader's class of each workflow) and the integration one-form saving (an estimate).
- The health-run box-h of month 12 (registered, not read here).
- Whether L1 moved the TS wall (no paired read).

### 6.3 Limits

- The notes were written before the L1 merge and the end read; their end values are forecasts, and this report reads them only against one end read (2 boxes, 31 rounds each, not paired with the month-11 read).
- One box type (Cloudflare standard-4, EPYC 9684X, SMT on, 2 CPUs pinned); no gate host.
- The read-to-read move is measured once (§1.7); the A/B form models use the read-1 widths and 42 null cells.
- Plan v18 is cited by decision id only; its text is not in the repo at this head.

## Raw numbers

[artifacts/m2-month13-plan-research/](artifacts/m2-month13-plan-research/README.md) holds `findings.json` (one object per finding, with its status, sources and numbers), `levers.tsv` (the lever forecast table of §1.2-§1.4) and `forecast-later.tsv` (the table of §1.9).
