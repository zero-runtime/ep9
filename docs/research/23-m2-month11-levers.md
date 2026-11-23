# The month-11 lever reads: predicted against measured x86 time, the Path B series, the gap that is left, and the parser debug-stack finding

## Run context

| Item | Value |
|---|---|
| Date | 2026-11-24 |
| Round | The month-11 records of the plan-v17 research: the box reads of month 11 (2 cause reads, 1 cycle read, 4 lever A/Bs, 2 Path B reads) and 3 plan-v17 research notes (speed, process, modes) with their inputs list. This report collects them. It runs no new box and no new measurement |
| ep9 trees | `m10-p5` 591064fc (the month-10 end; the TD-2, LAYOUT-PIN and K2 base code); `m11-p0` 0814321a (LDAR-ELIDE); `m11-p1` 7b96849a (the alignment rule); `m11-p3` 016a7061 (STAR-DST; the end Path B). Code cites (`crates/...`, `docs/adr/...`) are at e1c1d45b on `m2m11/prereg`. `git diff --stat m11-p3 e1c1d45b -- crates/` is empty, so they are also true at `m11-p3`. oxc is `=0.158.0` (`Cargo.toml:32`) |
| Boxes | Cloudflare standard-4, AMD EPYC 9684X (Zen 4; family 25, model 17, stepping 2, microcode 0x1000065), 4 vCPU, the engines pinned to CPUs 2-3. 16 boxes in month 11, and one health fuzz box that this report does not read. Box ids: ep9-x86-m11-layout-1, -td2-1, -ldar-1, -ldar-2, -layout-2, -k2-1, -k2-2, -rule-1, -rule-2, -mid-pathb-1, -mid-pathb-2, -star-1, -star-2, -star-3, -end-pathb-1, -end-pathb-2 |
| Reads used | DL rows M11-PRE-LAYOUT, M11-PRE-TD2-READ, M11-P0-LDAR-VERDICT, M11-PRE-LAYOUT-2-READ, M11-P2-K2-VERDICT, M11-P1-RULE-VERDICT, M11-P2-PATHB-MID, M11-P3-L16-VERDICT, M11-END-PATHB, DEPS-BUMP-2-OXC-HOLD; the box summaries and pooled read texts behind them; the CI TS rows 45021005639 and 45130289111 |
| Mac reads | The stack measurement of DEPS-BUMP-2 (aarch64-apple-darwin, debug and release, oxc 0.158.0 and 0.159.0; OL M6-ORCH-1429). 0 Mac speed reads |
| Upstream | `v8/v8@932ddd7ca` (2026-11-19), `WebKit/WebKit@f30433b25` (2026-11-19), `mozilla-firefox/firefox@ec287ddb0` (2026-11-19), `bellard/quickjs@e585640ff` (2026-11-10), `oxc-project/oxc@3464a0d18` (2026-11-16; oxc_parser 0.159.0). Each engine cite in §7.5 was read again in that commit for this report |
| Method | Read only. Every number has a cite. **M** = measured (a box, a CI run, a log). **D** = derived (arithmetic from cited numbers; the inputs are shown). **E** = estimate (basis named). **O** = open |
| Limits | One cycle map only (TD-2, 1 box, `m10-p5` code). The lever A/Bs and the Path B reads use different box instances and a different harness. Each Path B read is one read of 31 rounds per box. No x86-64 or Windows debug stack frame was measured |

Short forms:
- DL = [docs/decisions-log.md](../decisions-log.md), by row id. OL = the orchestrator log, by row id M6-ORCH-n (not in the repo).
- PB10, PBm, PBe = the Path B reads at the month-10 end (DL M10-END-D3), at mid month 11 on `m11-p0` (DL M11-P2-PATHB-MID) and at the month-11 end on `m11-p3` (DL M11-END-PATHB). PBm:79 = line 79 of the mid compare file. The compare files are not in the repo; the numbers this report uses are in [artifacts/m2-month11-levers/](artifacts/m2-month11-levers/README.md).
- TD2 = the TD-2 box summary (code 591064fc, 1 box, 11 read rounds; DL M11-PRE-TD2-READ). L2 = the LAYOUT-2 box summary.
- SV = the stack check note of DEPS-BUMP-2 and its logs (OL M6-ORCH-1429; DL DEPS-BUMP-2-OXC-HOLD).
- R17S, R17P, R17M = the plan-v17 research notes on speed, process and modes (not in the repo). IN17 = the plan-v17 inputs list. R16S = the plan-v16 speed note. P16 = plan v16 (DL M11-PLAN-D*). PK = the M11-P3 pick note (DL M11-P3-PICK holds its numbers).
- R21 = [21-m2-x86-ts-time.md](21-m2-x86-ts-time.md), R22 = [22-m2-stuck-causes.md](22-m2-stuck-causes.md).
- Ratios: an A/B ratio is t(lever) / t(base), paired per round (below 1 = the lever is faster). A gate ratio is t(node --jitless) / t(ep9) (above 1 = ep9 is faster). Box-neutral = t(ep9) / t(QuickJS) in the same rounds (lower = faster). "x1.0nn" = a time gain. Intervals are the order-statistic 95% intervals of the reads.
- "Instructions" = the perf instruction count on the box. "x64 TS Ir" = the cachegrind count of the CI TS row.

---

## 0. Verdict

```
 month 11, jitless TS (A/B terms)       predicted (plan-v16 record)     measured (pooled A/B)
 LDAR-ELIDE (a removed dispatch)        x1.009-1.021                     x1.043     2-5 times the record
 alignment rule (placement)             x1.000-1.013                     x0.999     (jit x1.043)
 REG-SLOT K2 (fewer ALU instructions)   x1.022-1.050                     x0.990     KILL, -8.0% instructions
 STAR-DST (a fused dispatch)            center x1.015-1.023               x1.0126    under the center
 product                                x1.032 / x1.057 / x1.085          x1.055

 Path B (gate ratio, box 1 / box 2)     m10 end          mid m11-p0       end m11-p3       move mid -> end
 jit TS                                 0.882 / 0.885    0.936 / 0.924    0.970 / 0.974    x1.036 / x1.054
 jitless TS                             0.942 / 0.942    0.974 / 0.973    0.972 / 0.980    x0.998 / x1.007   <- did not rise
 need at the low end (end read)         jit x1.048 / x1.030    jitless x1.035 / x1.033
```

1. **Instruction counts do not predict x86 time (M).** In one month the time change per 1% of instructions ran from -0.13 (REG-SLOT, default form) to 1.39 (LDAR-ELIDE). The month product matched the plan-v16 middle (x1.055 against x1.057) only by chance: each lever missed (§2).
2. **The removed unit sets the gain (D).** A removed dead dispatch saved about 5.55 cycles, more than its own 4.11 cycles of self time. A fused dispatch whose work stays saved about 1.04 cycles. Fewer ALU instructions inside a handler saved about 0 (-0.20 to +0.05 cycles per register decode) (§3).
3. **One lever was read alone at the Path B (M, D).** LDAR-ELIDE gave x1.043 in its A/B and x1.034 / x1.033 at the Path B: a transfer of about 0.78 in log terms. This is one lever on one box type (§3.3).
4. **C2 is closed (M).** LAYOUT-2 named the jit fall at LEVER-13 a placement draw. The alignment rule removed it on 3 trees (spread 0.0091). At the end read the jit build and the jitless build run the same time (same-round t(jitless)/t(jit) 0.997 / 0.995) (§4).
5. **The jitless cell did not rise from `m11-p0` to `m11-p3` (M).** The alignment rule and STAR-DST together predict x1.012 in A/B terms. The end read moved x0.998 / x1.007. Five candidate causes are listed as open questions; none is proven (§4.3).
6. **The interpreter loop is near node in cycles; the rest of the gap is not located (E).** The TD-2 map at `m10-p5` and the two cycle cells of month 11 project 24.86 G cycles against node's 24.98 G. Node runs 2.35% more CPU per wall second (helper threads, E) (§5).
7. **The candidates left do not reach the need in the center case (E).** The named month-12 set gives x1.022 in A/B terms (range x1.007-1.050). The need at the low ends is x1.030-1.048. With the month-11 transfer it is less (§6).
8. **A worst-case stack budget missed forms that were not in its measured set (M).** At oxc 0.159.0, 8 forms outside FORMS use up to 9,216 B per pre-scan unit in debug against a budget of 6,199. At the pre-scan cap the measured parse stack is 230.4 MB of 232.5 MB (margin 1.009x; 0.158.0: 1.46x). Three check rounds did not see it. A second-model review that added new inputs did. oxc stays at 0.158.0 (§7).
9. **V8, JSC, SpiderMonkey and QuickJS compare the real stack pointer at each parser recursion step (M, upstream source).** None of them uses a worst-case budget. oxc has no such check and no hook for it (§7.5).
10. **Process (M):** the 2-box pooled A/B form gave a word for each of the 4 levers (3 MERGE, 1 KILL; one needed the one more box), with 0 VOID reads. The checks that changed an outcome were the code-slice review, the box-script and pre-registration checks and one verifier of a reviewer claim. Tag CIs (3 runs) and DL fact checks did not change one gate number (§8).

---

## 1. The month in one diagram

```
 2026-11-22 (+0800)                                                                   2026-11-23            2026-11-24
 LAYOUT-PIN  LDAR A/B    TD-2       LAYOUT-2    K2 A/B      rule read   mid PB        STAR A/B             end PB
 02:43 1 box 11:31 2 box 12:22 1 b  13:18 1 box 14:17 2 box 19:44 2 box 20:44 2 box   03:42 2 box + 05:06  05:00 2 box
 MIXED       MERGE       map        DRAW        KILL        MERGE       no fire       MERGE (93 pairs)     no fire
   │           │           │          │           │           │           │             │                    │
   └─ C2 is placement ─────┼──────────┘           │           │           │             │                    │
               └─ m11-p0 ──┼──────────────────────┼───────────┼──────────►│ jitless +3.4%                      │
                           │                      │           └─ m11-p1 ──┼─────────────┼─ m11-p3 ──────────►│ jitless +0.3% (mean)
                           └─ STAR-DST pick input ┼─ (Star + Mov +0.761 G)┼─────────────┘                    │ jit     +4.5% (mean)
                                                  └─ instructions -8.0%, time +1.0%
```

Sources: DL M11-PRE-LAYOUT, M11-PRE-TD2-READ, M11-P0-LDAR-VERDICT, M11-PRE-LAYOUT-2-READ, M11-P2-K2-VERDICT, M11-P1-RULE-VERDICT, M11-P2-PATHB-MID, M11-P3-L16-VERDICT, M11-END-PATHB. Times are +0800.

---

## 2. The main result: each month-11 lever, predicted against measured

### 2.1 The table

| Lever (phase) | What it changes | Instruction change (TS) | Predicted wall gain | Measured A/B, TS wall (median [interval], pairs, boxes) | Verdict row | Time % per 1% instructions (D) | Cycles saved per removed unit (D) |
|---|---|---|---|---|---|---|---|
| LAYOUT-PIN (cause read, M11-PRE) | `-C llvm-args=-align-all-functions=6`: every function on a 64 B line, `m10-p5` code | a-jit / e-jit 1.000 [0.998-1.002] | jit x1.00-1.07, jitless x1.000-1.013 (R16S §3.1; P16 §10 row 1) | a-jit / e-jit **0.943** [0.928-0.959] = x1.060; a-jitless / e-jitless 1.007 [0.981-1.019]; aligned pair t(a-jitless)/t(a-jit) 0.986 [0.972-1.019]; 31 pairs, 1 box | M11-PRE-LAYOUT: **MIXED** | n/a (0 instructions) | n/a (placement only) |
| LAYOUT-2 (cause read, M11-PRE) | the LEVER-13 A/B (f09c2daa against 049ed54d) with an aligned control column | Ad / Bd jit 0.995, jitless 0.994 | no time line (a cause read; R22 §7 row 1) | S_d (pair shift, default form) **+0.0542** [+0.0262 to +0.0706]; S_a (aligned) -0.0058 [-0.0231 to +0.0177]; G = t(Aa-jit)/t(Ad-jit) 0.959 [0.944-0.970]; 31 pairs, 1 box | M11-PRE-LAYOUT-2-READ: G2 **REPEAT**, cause **DRAW** | LEVER-13 alone, aligned form: both intervals hold 1.000 (L2:170, :175): not resolved | n/a |
| LDAR-ELIDE arm 2 (M11-P0) | no dispatch for the dead `Ldar`: arm 1 the statement-position classes T + B-S, arm 2 = arm 1 + the no-mark class B-N; same binary, switch `EP9_LDAR_ELIDE` | x64 TS Ir jit -2.926%, jitless -2.961% (CI 45021005639); arm 1 box instructions 0.9777 | jitless x1.009-1.021 (arm 1 x1.007-1.016 + arm 2 x1.002-1.005; an Ir-factor record, P16 §10 row 3); time-only ceiling x1.024 (DL M11-PLAN-D1) | jitless **0.9587** [0.9518-0.9687] = x1.043; jit 0.9602; cycles 0.9580; arm 1 0.9691 [0.9604-0.9770]; 62 pairs, 2 boxes | M11-P0-LDAR-VERDICT: **MERGE** (arm 2) | **1.39** (jitless; arm 1 by box counts: 1.39) | **5.55** per removed `Ldar` (self time 4.11) |
| X86-ALIGN-RULE (M11-P1) | the LAYOUT-PIN flag as a build rule in `.cargo/config.toml`, judged by spread on 3 trees (T_m9, T_mid, T_end) | 1.000-1.001 (each box, both builds) | jit x1.03-1.07 only on CONFIRM, jitless x1.000-1.013 (P16 §10 row 1) | T_end jit **0.959** [0.950-0.971] = x1.043; jitless 1.001 [0.988-1.008]; spread S_a 0.0091; 62 pairs, 2 boxes | M11-P1-RULE-VERDICT: **MERGE** | n/a (0 instructions) | n/a (placement only) |
| REG-SLOT K2 (M11-P2) | the register operand becomes the frame slot offset: 3 instructions (`lea`, `test`, `cmovns`) leave each register decode; two binaries (24259cf2 against 591064fc), both layout forms | box instructions **0.920** (-8.0%), both builds, both forms | jitless x1.022-1.050 (Ir-factor record, P16 §10 row 2); no time bound (DL M11-PLAN-D1) | default jitless **1.010** [1.003-1.023] = x0.990; aligned jitless 0.996 [0.986-1.005]; richards jitless 1.037 (GUARD-FAIL); 62 pairs, 2 boxes | M11-P2-K2-VERDICT: **KILL** + GUARD-FAIL | **-0.13** (default), 0.05 (aligned) | **-0.20** (default) to +0.05 (aligned) per register decode (E) |
| STAR-DST (M11-P3) | 3 fused opcodes (`GetNamedPropertyStar`, `LdaCurrentContextSlotStar`, `LdaContextSlotStar`) replace a producer + `Star` pair; same binary, switch `EP9_STAR_DST` | box instructions jitless 0.9833, jit 0.9816 (n 93); x64 TS Ir jit -1.813%, jitless -1.838% (CI 45130289111) | center x1.015-1.023, range x1.000-1.045 (DL M11-P3-PICK) | aligned jitless **0.9876** [0.9828-0.9917] = x1.0126; jit 0.9850; cycles 0.9879; default jitless 0.9956 [0.9895-1.0010]; 93 pairs, 3 boxes (read 1 on 62 pairs: ONE MORE BOX) | M11-P3-L16-VERDICT: **MERGE** (the set, 290 handlers) | **0.74** (box counts), 0.67 (x64 Ir); default form 0.24 (x64 Ir) | **1.04** per removed `Star` dispatch (self time 2.76) |

Each row's numbers are in its verdict row. The arm classes of LDAR-ELIDE are in DL M11-PRE-LDAR. Rows are in [levers.tsv](artifacts/m2-month11-levers/levers.tsv).

### 2.2 The derivations

```
 LDAR-ELIDE arm 2 (jitless TS)
   time      1 - 0.9587 = 4.13%                                   (DL M11-P0-LDAR-VERDICT)
   ratio     4.13 / 2.961 = 1.39 per 1% of x64 TS Ir               (CI 45021005639: 54.083 G -> 52.482 G; OL M6-ORCH-1336)
             arm 1 on box counts: (1 - 0.9691) / (1 - 0.9777) = 1.39
   cycles    26.272 G x (1 - 0.9580) = 1.103 G                     (TD2:22 e-jitless cycles x the A/B cycles cell)
   per unit  1.103 G / 198,698,375 removed dispatches = 5.55       (count: DL M11-P3-PICK)
   compare   Ldar self time 1.075 G / 261,755,534 runs = 4.11       (TD2:204; the run count R22 §9.2)

 STAR-DST (aligned jitless TS)
   time      1 - 0.9876 = 1.24%                                   (DL M11-P3-L16-VERDICT)
   ratio     1.24 / 1.67 = 0.74 (box OV-5 0.9833);  1.24 / 1.838 = 0.67 (CI 45130289111)
   cycles    26.272 G x 0.9580 = 25.17 G (E: the base after LDAR-ELIDE); x (1 - 0.9879) = 0.305 G
   per unit  0.305 G / 293,667,585 removed Star dispatches = 1.04   (count: DL M11-P3-PICK)
   compare   Star self time 1.030 G / 373,415,409 Star runs = 2.76  (TD2:74; R22 §9.2): 1.04 is about 3/8 of it

 REG-SLOT K2 (jitless TS)
   time      default +1.0% (1.010); aligned -0.4% (0.996)         (DL M11-P2-K2-VERDICT)
   ratio     -1.0 / 8.0 = -0.13;  0.4 / 8.0 = 0.05
   cycles    default 1.011 -> -0.29 G; aligned 0.997 -> +0.08 G    (the K2 read: cycles cells; base 26.272 G)
   per unit  8.0% of 54.562 G = 4.36 G instructions / 3 = 1.45 G register decodes (E)
             -0.29 / 1.45 = -0.20;  +0.08 / 1.45 = +0.05 cycles per decode
```

- The per-unit cycles mix 2 box sets: the TD-2 base (1 box) and the A/B cycle cells (other boxes). The TD-2 box ran TS e-jitless in 7.21 s; the A/B boxes differ (§10.3). So these are estimates of a size, not exact counts.
- The STAR-DST base 25.17 G is an estimate: no cycle read exists between LDAR-ELIDE and STAR-DST.
- The REG-SLOT decode count uses the measured instruction cut over 3 instructions per decode. R16S gave a static estimate of about 1,356 M decodes; the sign and the size (about 0) do not change.

### 2.3 Predicted against measured

```
 jitless TS, A/B terms           low      record   high      measured   where the record came from
 LDAR-ELIDE arm 1 + arm 2        x1.009            x1.021    x1.043     Ir x 0.31-0.69 (R16S §4.1); above the time ceiling x1.024
 alignment rule (jitless)        x1.000            x1.013    x0.999     the m10-p1 X86-ALIGN read (DL M10-P1-X86ALIGN)
 REG-SLOT                        x1.022            x1.050    x0.990     Ir x 0.31-0.69; no time bound (DL M11-PLAN-D1)
 STAR-DST                        x1.000   x1.015-1.023 x1.045 x1.0126   bucket share, "dispatch part only" x1.012 (PK)
 product                         x1.032   x1.057   x1.085    x1.055     (D: 0.9587 x 1.001 x 0.9876 = 0.948)
```

- Plan v16 kept the Ir-factor numbers as "a record only" and took no target or date from them (P16 §0; DL M11-PLAN-D1). The reads confirm that rule.
- The one method that hit was the STAR-DST "dispatch part only" estimate: the Star bucket share of the fused pairs times 3/8 (the dispatch part of the 8 Star instructions) gave x1.012 (PK). The read gave x1.0126 (R17S §2.2).

---

## 3. The lesson, with its evidence and its limits

### 3.1 Instruction counts do not predict x86 time

| Lever | Instructions | Time | Time % per 1% instructions | Source |
|---|---|---|---|---|
| REG-SLOT K2, default form | -8.0% | +1.0% | -0.13 | DL M11-P2-K2-VERDICT |
| REG-SLOT K2, aligned form | -8.0% | -0.4% | 0.05 | same |
| STAR-DST, default form | -1.84% (x64 Ir) | -0.44% | 0.24 | DL M11-P3-L16-VERDICT |
| STAR-DST, aligned form | -1.67% | -1.24% | 0.74 | same |
| LDAR-ELIDE arm 2 | -2.96% (x64 Ir) | -4.13% | 1.39 | DL M11-P0-LDAR-VERDICT |
| alignment rule, jit | 0 | -4.1% | n/a | DL M11-P1-RULE-VERDICT |
| month 10 (R22 §3.2) | LEVER-12 -2.1% | 0 | 0 | DL M10-P1-B1 |

- The largest instruction cut of month 11 was the only lever that lost time.
- The largest time gain at 0 instructions came from placement.
- In the same-binary A/Bs the cycles cell equals the wall cell (LDAR 0.9580 / 0.9587; STAR-DST 0.9879 / 0.9876). So a cycle estimate inside one binary is also a wall estimate (R17S §2.2 L3).

### 3.2 What kind of removed work saves cycles

| Removed unit | Example | Cycles saved per unit (D) | Own self time per run | Why (I) | Source |
|---|---|---|---|---|---|
| a whole dispatch of dead work | `Ldar` with no reader | 5.55 | 4.11 | the indirect jump goes too: branch misses fell 8.8% (arm 1, 0.9121) | §2.2; DL M11-P0-LDAR-VERDICT |
| a dispatch whose work stays | producer + `Star` fused | 1.04 | 2.76 | the store stays; branch misses +0.8%, L1i misses +2.6% (jitless) | §2.2; DL M11-P3-L16-VERDICT |
| ALU instructions inside a handler | 3 of 8 instructions per register decode | -0.20 to +0.05 | - | the core runs them in the shadow of the loads (I); L1i misses rose 5.8% in the aligned form | §2.2; K2 read |
| placement only | 64 B alignment | jit 4.1%, jitless 0 | - | the default jit build sat in a bad draw (§4.2) | DL M11-P1-RULE-VERDICT |

Limits: 1 lever per class; 1 box type (9684X, SMT on); TS only for the decision cells. The "why" column is an inference, not a counter read.

### 3.3 How an A/B gain carried to the Path B

| Step | A/B gain | Path B move (box 1 / box 2) | Transfer, log terms (D) | Other change between the reads |
|---|---|---|---|---|
| m10 end -> mid: LDAR-ELIDE only | jitless x1.043 | x1.034 / x1.033 | 0.79 / 0.77 | none in the engine (DL M11-P2-PATHB-MID) |
| mid -> end: alignment rule + STAR-DST, jitless | x1.012 (D: 1 / (1.001 x 0.9876)) | x0.998 / x1.007 | -0.18 / 0.62 | 2 merges; 153 files in the switch paths (DL M11-END-PATHB) |
| mid -> end, jit | x1.059 (D: 1 / (0.959 x 0.9850)) | x1.036 / x1.054 | 0.63 / 0.92 | same |
| m10 end -> end, whole month, jitless | x1.055 (D: 0.9587 x 1.001 x 0.9876) | x1.032 / x1.040 | 0.58 / 0.74 | 3 merges |
| m10 end -> end, whole month, jit | x1.103 (D: 0.9602 x 0.959 x 0.9850) | x1.100 / x1.101 | 0.97 / 0.98 | 3 merges |

- **Only the first row is one lever.** It gives a transfer of about 0.78. R17S uses 0.8 for every projection, as an estimate (R17S §2.2 L4).
- The jit cell got almost all of its A/B product over the month. The jitless cell got 58-74% of it. Section 4.3 lists the candidate causes as open questions.

---

## 4. The Path B series

### 4.1 The reads

| Read | Tree | Boxes | Item 3 TS jit (box 1 / box 2) | Item 3 TS jitless | Same-round t(jitless)/t(jit) TS | Box-neutral jit / jitless (box 1; box 2) | ep9 jitless TS gross | SWITCH | RULE 100 |
|---|---|---|---|---|---|---|---|---|---|
| PB10 (month-10 end) | `m10-p5` 591064fc | ep9-x86-m10-end-pathb-1, -2 | 0.882 [0.875-0.893] / 0.885 [0.879-0.905] | 0.942 [0.923-0.955] / 0.942 [0.922-0.960] | 0.951 / 0.943 (R22 §2.3) | 0.667 / 0.630 (2-box mean, R22 §3.1) | - | - | - |
| PBm (mid) | `m11-p0` 0814321a | ep9-x86-m11-mid-pathb-1, -2 | 0.936 [0.908-0.952] / 0.924 [0.918-0.939] | 0.974 [0.956-0.988] / 0.973 [0.957-0.985] | 0.956 [0.940-0.970] / 0.948 [0.932-0.967] | 0.642 / 0.620; 0.638 / 0.608 | 7.610 / 7.624 s | DOES NOT FIRE | NO FALL |
| PBe (end) | `m11-p3` 016a7061 | ep9-x86-m11-end-pathb-1, -2 | 0.970 [0.954-0.977] / 0.974 [0.971-0.985] | 0.972 [0.966-0.982] / 0.980 [0.968-0.990] | 0.997 [0.985-1.008] / 0.995 [0.982-1.001] | 0.605 / 0.604; 0.614 / 0.611 | 6.059 / 5.847 s | DOES NOT FIRE | NO FALL |

| Item (gate ratio; box 1 / box 2) | Build | PBm | PBe | Threshold |
|---|---|---|---|---|
| 1 Octane-7 vs QuickJS | jit | 1.788 / 1.771 | 1.898 / 1.886 | 1.33 |
| 1 | jitless | 1.898 / 1.883 | 1.929 / 1.934 | 1.33 |
| 2 v8-v7 vs node --jitless | jit | 1.143 / 1.151 | 1.165 / 1.162 | 1 |
| 2 | jitless | 1.197 / 1.202 | 1.182 / 1.179 | 1 |
| 3 TS vs node --jitless | jit | 0.936 / 0.924 | 0.970 / 0.974 | 1 |
| 3 | jitless | 0.974 / 0.973 | 0.972 / 0.980 | 1 |
| 6 splay vs node --jitless | jit | 1.118 / 1.118 | 1.105 / 1.118 | 1 |
| 6 | jitless | 1.141 / 1.152 | 1.116 / 1.098 | 1 |

Sources: DL M11-P2-PATHB-MID, M11-END-PATHB, M10-END-D3; PBm:78-79, :134-135, :363; PBe:8-24, :78-79, :134-135, :363. Box-neutral = 1 / (the TS-vs-QuickJS median); for 31 rounds the median of 1/x is 1 / the median of x. Rows are in [pathb-series.tsv](artifacts/m2-month11-levers/pathb-series.tsv).

- **The rule-100 changes (M):** PBm against PB10: jit +6.08% / +4.44%, jitless +3.36% / +3.36%. PBe against PBm: jit +3.72% / +5.40%, jitless -0.17% / +0.71%. No fall above 2% on both boxes, so no LAYOUT-PIN read ran.
- **The switch (M):** 12 of 16 intervals sit at or above the threshold in both reads. The 4 TS intervals are below 1.0, so branch N: month 11 is not the M2 exit month.
- **The gap left at the low ends (D, PBe):** jit 1 / 0.954 = x1.048 (box 1), 1 / 0.971 = x1.030 (box 2); jitless 1 / 0.966 = x1.035, 1 / 0.968 = x1.033. At the medians: jit x1.031 / x1.027, jitless x1.029 / x1.020.
- The TS steering row 0.95 is met on both boxes in both builds at PBe (low ends 0.954-0.971).

### 4.2 What the series shows about the jit cell

- At PB10 and PBm the jit build ran 4.4-5.7% slower than the jitless build in the same rounds (0.951 / 0.943; 0.956 / 0.948). At PBe the two builds run at the same time (0.997 / 0.995; both intervals hold 1.000).
- So the jit cell rose to the jitless cell. Its rise mid -> end (x1.036 / x1.054) is near the rule's A/B gain (x1.043) times STAR-DST (x1.015).
- This agrees with C2 = placement (LAYOUT-2 cause DRAW) and with the rule read on 3 trees (aligned pair medians 1.002, 1.001, 1.007).
- The jit build still pays about 1.1% more instructions for the record hooks (L2:137, Aa-jit / Aa-jitless instructions 1.011). At PBe this cost does not show in the same-round ratio.

### 4.3 The jitless cell did not rise from `m11-p0` to `m11-p3`

| Read | What it measures | jitless value | Source |
|---|---|---|---|
| alignment rule A/B (T_end, aligned / default) | the rule alone, jitless | 1.001 [0.988-1.008] (x0.999) | DL M11-P1-RULE-VERDICT |
| STAR-DST A/B, aligned form (the merged form) | the fusion alone, same binary | 0.9876 [0.9828-0.9917] (x1.0126) | DL M11-P3-L16-VERDICT |
| STAR-DST A/B, default form | the fusion in the old layout | 0.9956 [0.9895-1.0010] (x1.0044) | same |
| expected Path B move (D, transfer 1.0 / 0.78) | product of the two A/Bs | x1.012 / x1.009 | §3.3 |
| Path B gate ratio, PBm -> PBe | the gate cell | 0.974 -> 0.972 (x0.998); 0.973 -> 0.980 (x1.007) | DL M11-END-PATHB |
| Path B CPU-time ratio vs node, PBm -> PBe | the same cell on CPU time | 0.984 -> 0.989; 0.988 -> 0.997 | PBm:316, PBe:316 |
| box-neutral ep9 jitless / QuickJS, PBm -> PBe (D) | the cell against QuickJS | 0.620 -> 0.604 (x1.027); 0.608 -> 0.611 (x0.995) | PBm:134, PBe:134 |
| node / QuickJS, PBm -> PBe (D, from medians, not paired) | the comparator against QuickJS | 0.604 -> 0.587; 0.591 -> 0.598 | PBm:134-135, PBe:134-135 |
| ep9 jitless TS gross time, PBm -> PBe | the box speed | 7.610 s -> 6.059 s; 7.624 s -> 5.847 s (0.80 / 0.77) | PBm:23, PBe:23 |

Candidate explanations. **All are OPEN. None is shown by a read.**

| # | Candidate | For it | Against it | What would settle it |
|---|---|---|---|---|
| O1 | The two Path B reads are not paired, so a 1% move is inside the read-to-read noise | the 2 boxes of PBe differ by 0.008 in the median; node / QuickJS moved -2.9% on box 1 and +1.2% on box 2 between the reads (D) | the jit cell moved as predicted in the same reads | the same tree read twice by Path B (an A/A of the Path B form) |
| O2 | The box state differs: the end boxes ran TS 20-23% faster in gross time than the mid boxes, at the same CPU model and microcode | R22 Q1: A/B boxes ran 8.0-8.8 s at IPC 1.85-1.95, the TD box 6.2 s at IPC 2.46; a fused-dispatch gain can depend on the frontend / backend mix (I) | the gate ratio is paired in the round, so a box speed change cancels for a gain that does not depend on the box | the R22 Q1 read (route AB-BOX-CONFOUND, cut in month 11) or a TD-form read on the end tree |
| O3 | The A/B arm 0 is not the binary that shipped: the probe binary carries the 3 new opcodes and the switch, and the merge removes the switch and moves the code | the default-form gain is x1.0044 only: the gain needs a layout (DL M11-P3-L16-VERDICT) | every handler is on a 64 B line in both (580 of 580 on all 3 boxes) | a two-binary A/B, `m11-p0` + rule against `m11-p3`, in the Path B harness |
| O4 | STAR-DST costs more on another box: it raised L1i misses 2.6% and branch misses 0.8% (jitless) | the A/B report cells (DL M11-P3-L16-VERDICT) | the same-binary cycles cell matched the wall cell (0.9879 / 0.9876) | per-symbol cycles on the end tree (a TD-3 read) |
| O5 | Something else in the 153 changed files moved jitless time | the switch-path diff is 153 files, +6,422 -3,524 (DL M11-END-PATHB) | the two merges are the only engine changes named | the same two-binary A/B as O3 |

- DEPS-BUMP-2 is not a candidate: the end read ran on tag `m11-p3`, which does not hold the bump (OL M6-ORCH-1432).
- Do not read the end jitless value as a floor or as a loss of STAR-DST: no read shows either (I).

---

## 5. Where the rest of the gap is

### 5.1 The TD-2 cycle map (`m10-p5` code, 1 box, before the 3 merges)

| Item (TS, TD-2 box) | ep9 e-jitless | node --jitless | Gap | Mark | Source |
|---|---|---|---|---|---|
| cycles, all threads | 26.272 G | 24.976 G | +1.296 G | M | TD2:22, :24 |
| instructions | 54.562 G | 57.266 G | -4.7% | M | same |
| IPC | 2.076 | 2.293 | CPI +10.4% (D) | M | same |
| wall | 7209.1 ms | 6696.2 ms | gate ratio 0.933 [0.898-0.956] | M | TD2:22-27 |
| named load (no Wide) | 7.795 G | 7.792 G | +0.003 | M | TD2:203 |
| `Ldar` handler | 1.075 G | 0.285 G | +0.790 | M | TD2:204 |
| Wide / prefixed path | 1.448 G | 0.802 G | +0.646 (+0.349 like for like, PK) | M | TD2:205 |
| `Star` + `Mov` (biased by the V8 Star lookahead) | 1.190 G | 0.430 G | +0.761 | M | TD2:206 |
| cycles per named-load run, `Star` included | 24.18 | 22.41 | +1.77 | M | DL M11-PRE-TD2-READ line [4] |
| cycles per named-load run, no `Star` | 22.42 | 22.41 (19.86 + 2.55 `LoadIC_Noninlined`) | parity | M | TD2:212, :216-217 |
| backend-stall slots in the named load | 11.805 G | 8.679 G | ep9 latency-bound | M | TD2:220, :222 |
| frontend slots in the named load | 7.859 G | 9.798 G | node fetch-bound | M | TD2:223, :225 |

### 5.2 Projection to `m11-p3`, and the part outside the interpreter loop

| Item | Value | Mark | Basis |
|---|---|---|---|
| all-thread cycles at `m11-p3`, jitless | 26.272 x 0.9580 x 0.9879 = 24.86 G (node 24.98 G) | E | the two same-binary cycle cells; the rule gives jitless 1.001; mixes boxes |
| `Ldar` bucket left | 1.075 G x 63.06 M / 261.76 M = 0.26 G (node 0.285 G) | E | the live `Ldar` runs (R22 §9.2) |
| `Star` + `Mov` left | about 0.89 G against 0.43 G | E | R17S §3: 1.190 G minus the fused part |
| CPU per wall second | ep9 3.644 G/s, node 3.730 G/s (node +2.35%), QuickJS 3.569 G/s | D | cycles / wall, TD2:22-25 |
| wall share node gets off its main thread | about 2.3% | E | 0.951 (cycle ratio) x 3.644 / 3.730 = 0.929 against the measured 0.933; SMT and clock effects not excluded |
| GC-like symbols, all threads | ep9 7.8% (about 2.05 G), node 6.1% (about 1.53 G) | E | a symbol-name match over 1 record per engine (R17S §3) |
| calls (handlers, slow path, Return, Construct) | ep9 about 3.8 G, node about 4.7 G | D | R17S §3 from TD2:75-94, :138-160 |

- So at `m11-p3` the loop buckets that month 11 attacked are near node (E). The wall gap of 2-3% (§4.1) has no measured location. The candidates are the Wide path, the `Star` + `Mov` rest, GC on the main thread and the work node runs on helper threads (O).

### 5.3 The opcode pair counts (fall-through pairs, `m10-p5` dump, before LDAR-ELIDE and STAR-DST)

| Pair | Runs | Now | Source |
|---|---|---|---|
| `Star` -> `GetNamedProperty` | 173.3 M | inside `GetNamedPropertyStar` -> `GetNamedProperty` (100.06 M) | R17S §4.4 |
| `Ldar` -> `GetNamedProperty` | 133.5 M | mostly removed by LDAR-ELIDE | same |
| `GetNamedProperty` -> `TestEqual` | 33.8 M | open | same |
| `TestEqual` -> `JumpIfFalse` | 26.6 M | open | same |
| `LdaUndefined` -> `Return` | 22.8 M | open | same |
| `TestLessThan` -> `JumpIfFalse` | 17.6 M | open | same |
| `Mov` -> `Mov` | 17.4 M | open | same |
| `GetNamedProperty` -> `TestLessThan` | 17.0 M | open | same |
| `GetNamedProperty` -> `JumpIfToBooleanFalse` | 15.5 M | open | same |
| `TestEqualStrict` -> `JumpIfTrue` (Wide) | 8.1 M | open | same |

- Counted by a read-only script of R17S over the X2 count files (R22 §9.2) at 591064fc; total 1,657,514,120 dispatches. No file holds all pairs on the `m11-p3` dump (O).
- At the fused-dispatch price of §3.2 (about 1 cycle), each open pair is worth 0.1-0.4% of TS time (E, R17S §4.4).

---

## 6. The candidates left, and the open questions for month 12

### 6.1 Estimate table (A/B gain on jitless TS; jit the same unless named)

| Candidate | Removed unit | Low | Center | High | G8 | Basis | Source |
|---|---|---|---|---|---|---|---|
| Wide set: JUMP-INVERT + W-1 far-jump twins + W-2 per-scale Wide | the `Wide` prefix step on 19.96 M far forward jumps | x1.007 | x1.013 | x1.020 | +2 | removed overhead per prefixed op (about 13.3 cycles), work stays | R17S §4.2 |
| MH rider: a stub-cache index with no multiply | a multiply on each megamorphic probe | x1.000 | x1.002 | x1.004 | 0 | 29.1 M megamorphic hits x about 3 cycles (I) | R17S §4.3 |
| GC-BUDGET (only if a TD-3 read keeps it) | fewer minor GCs | x1.000 | x1.005 | x1.015 | 0 (I) | G | R17S §4.8 |
| NL-1 stage 1 (4 -> 3 dependent loads) | one load on the feedback path | x1.000 | x1.002 | x1.010 | 0 (+1 if disputed) | a latency lever, the K2 class of §3.2 | R17S §4.3 |
| **product, A/B** | | **x1.007** | **x1.022** | **x1.050** | | D | R17S §5 |
| at the Path B, transfer 0.8 (the one-lever read, §3.3) | | x1.006 | x1.018 | x1.040 | | D | same |
| at the Path B, transfer 0.6 (the month-11 jitless read, §3.3) | | x1.004 | x1.013 | x1.030 | | D | this report |
| **need at the low end (PBe)** | | | jitless x1.033-1.035, jit x1.030-1.048 | | | D | §4.1 |

- In the center case the set does not reach the need. In the high case it reaches the jitless need only with the 0.8 transfer (E).
- Dropped for month 12 by R17S: new fusions (0.1-0.4% each at G8 +1 to +3), a REG-SLOT retry (K2 KILL in both forms), branch-miss levers, PGO, a jit-only lever. BOLT stays last (R17S §4.4-§4.9).
- The only item with an estimated ceiling of 10% or more is a generated interpreter (pinned registers, a handler ABI with no callee saves). It is 2-4 plan months and not in M2 (DL M11-PLAN-D8; R22 §7 row 10).

### 6.2 Open questions for month 12

| # | Question | What would settle it | Cost (E) |
|---|---|---|---|
| Q1 | Why did the jitless cell not rise at the Path B after STAR-DST (§4.3)? | a two-binary A/B in the Path B harness (`m11-p0` + rule against `m11-p3`), or a Path B A/A | about 1.9-2.1 box-h (the K2 two-binary form 1.92; a 2-box Path B 2.10) |
| Q2 | Where are the cycles at `m11-p3`? The map is 3 merges old | TD-3: perf stat, top-down and perf record with `--sort comm,sym` (main thread against helpers) on the end tree | 0.4-0.5 box-h (TD-2 took 0.38; R17P §1.3 N1) |
| Q3 | How much of the wall gap is GC on the main thread, and how much is node's helper work? | the TD-3 GC arm (`EP9_GC_HELPERS=0`, same binary) | in TD-3 |
| Q4 | What is the A/B-to-Path B transfer for a removed-overhead lever (the Wide set)? | its A/B, then the next Path B with no other merge | in the lever plan |
| Q5 | How does item 3 read on Zen 3 (EPYC 7763), the named gate host model, with the alignment rule? | the `m11-p3` exit-input legs (dispatch 45265194357, OL M6-ORCH-1437; not read here). Before the rule, at `m11-p0`, the GitHub l1 7763 leg read TS jit 1.015, jitless 1.012 (OL M6-ORCH-1343; one leg, not a gate host) | 0 boxes |
| Q6 | Do the A/B boxes and the Path B boxes differ in kind (R22 Q1)? | the R22 Q1 read | 0.5 box-h (R22 §11 Q1) |

---

## 7. The parser debug-stack finding

### 7.1 The design of the worst-case stack budget (at e1c1d45b, oxc 0.158.0)

```
 caller thread                                       ep9-frontend worker thread (stack = WORKER_STACK_BYTES)
 ─────────────                                       ─────────────────────────────────────────────────────
 prescan::estimate (a token loop, no recursion)      parse: oxc_parser (recursive; no depth limit, no stack check)
   estimate > PRESCAN_CAP 25,000 -> RangeError         -> depth::check_ast_depth (layer 2, AST_CAP 20,000)
   lib.rs:352-354                                      -> oxc_semantic, scope pass (bounded by AST_CAP)
                                                       lib.rs:368, :400-404, :414
 WORKER = BASE 2 MiB + 1.5 x max(25,000 x PARSE_PER_UNIT, 20,000 x PASS_PER_LEVEL)     stack.rs:53-58
   debug PPU 6,144 -> 232,497,152 B ;  release PPU 2,560 -> 98,097,152 B              stack.rs:35-38
   const assert WORKER < 256 MiB in every build                                       stack.rs:62-68 (:67)
 budget per pre-scan unit = WORKER / 1.5 / PRESCAN_CAP = 6,199 B debug, 2,615 B release  probe.rs:148-151
 the only proof that PPU is large enough: probe::measure_forms over FORMS             probe.rs:172, :238
```

| Part | File:line | Fact |
|---|---|---|
| the formula | `crates/ep9_frontend/src/stack.rs:4-6`, `:53-58` | BASE + SAFETY x max(PRESCAN_CAP x PARSE_PER_UNIT, AST_CAP x PASS_PER_LEVEL); SAFETY = 3/2 (`:32-33`) |
| the per-unit constant | `stack.rs:35-38` | debug 6,144 B, release 2,560 B |
| the ceiling | `stack.rs:67` | `WORKER_STACK_BYTES < 256 << 20` in every build |
| the measured table | `stack.rs:9-20` | measured on aarch64-apple-darwin only; debug parse worst 5,824 B (async arrows, oxc 0.158) |
| the caps | `crates/ep9_frontend/src/depth.rs:29`, `:37` | AST_CAP 20,000; PRESCAN_CAP = AST_CAP + AST_CAP / 4 = 25,000 |
| the estimate rule | `depth.rs:33-36` | "the estimate grows at most 1 unit per AST level" for every measured form |
| the pre-scan unit | `crates/ep9_frontend/src/prescan.rs:8-15`, `:280-284` | one open bracket frame plus the chain ops pending in it |
| the comma | `prescan.rs:879-883` | a `,` clears only the expression ops of the frame: a long list at one level adds no unit |
| the measured set | `crates/ep9_frontend/src/forms.rs:41` | `FORMS`: 49 forms (H01:42: 39 forms + 10 regex forms) |
| what FORMS holds | `forms.rs:85-90`, `:103-108`, `:177-182`, `:225-230` | "call" = `f()()()`; "tagged-template" = `` f`x``x` `` (no substitutions); "spread" = array spread `[...[...a]]`; "switch" = nested `switch` |
| the budget test | `crates/ep9_frontend/src/probe.rs:148-151`, `:238` | asserts the parse slope per unit against PARSE_UNIT_BUDGET, for FORMS only |
| the depth tests | `crates/ep9_frontend/src/depth_tests.rs:64-80`, `:280-302` | `check_ast_form` runs each form at the AST cap, not at the pre-scan cap; the random mixes use the WRAP list |
| the ADR text | `docs/adr/H01-front-end-boundary.md:29`, `:53` | B2 and Costs give the formula and "about 222 MiB debug (232,497,152 B)"; the 256 MiB number is only in code |

### 7.2 What the measured set missed

| Form (B per pre-scan unit; budget debug 6,199, release 2,615) | 0.158.0 debug | 0.159.0 debug | 0.158.0 release | 0.159.0 release |
|---|---:|---:|---:|---:|
| FORMS worst (`measure_forms`) | 5,824 | 5,968 | 1,488 | 1,760 |
| spread arguments `f(...a, ..., ...f(...))`, k ≥ 6 | **6,384** | **9,216** | 1,184 | 1,616 |
| nested call with arguments (call, method, optional, async), nested last or middle, k ≥ 6 | **6,288** | **9,120** | 1,184 | 1,552 |
| `f(f(0))` (k = 1, or nested first) | 5,920 | **6,928** | 1,184 | 1,552 |
| tagged template with substitutions `` f`${0}..${...}` `` | 5,360 | **8,656** | 1,344 | 1,984 |
| long switch clause | 1,760 | **8,176** | 368 | 1,392 |
| long sequence / template / array | 3,886 / 3,776 / 3,774 | **7,216 / 7,136 / 6,464** | ≤ 992 | ≤ 1,760 |
| function statements / arrow body call | 4,224 / 5,128 | **6,592 / 6,656** | - | ≤ 2,272 |
| release worst of all 190 rows | - | - | 1,904 | 2,240 |

Source: SV (aarch64-apple-darwin, `n` 500 -> 1500, the real parse path `regex_check::parse`); bold = above the debug budget. The list length adds no cost above about 5 items (k = 6..200 read the same). Rows are in [stack-measure.tsv](artifacts/m2-month11-levers/stack-measure.tsv).

- At 0.158.0 two forms (spread arguments, nested call with arguments) were already above the debug budget. They still fit the stack with a smaller margin (§7.3). This part is pre-existing (route FRONTEND-STACK-FORMS; OL M6-ORCH-1429).
- The release build stays inside its budget at both versions.

### 7.3 The at-cap proof (debug, the real worker, aarch64 only)

| Version | Input | Measured parse stack at the cap | Worker stack | Margin (D) | Result |
|---|---|---:|---:|---:|---|
| 0.158.0 | spread arguments k = 6, n = 24,999 (estimate 25,000 = the cap) | 159,608,735 B (159.6 MB) | 232,497,152 B (232.5 MB) | 232.5 / 159.6 = **1.46x** | RangeError TooDeep(Ast), no abort |
| 0.159.0 | same | 230,410,927 B (230.4 MB) | 232,497,152 B | 232.5 / 230.4 = **1.009x** (2,086,225 B left) | RangeError TooDeep(Ast), no abort |
| 0.159.0 | n = 25,000 (estimate 25,001) | - | - | - | RangeError TooDeep(PreScan) |

- The design wants 1.5x (`stack.rs:32-33`), because the other targets are not measured (`stack.rs:9-12`, `:31`).
- At 0.159.0 the overflow line is about 9,299 B per unit (D: 232,497,152 / 25,000). x86-64 and Windows debug frames were not measured. Windows frames are larger (`stack.rs:31`).
- A real overflow ends the process: Rust std prints "has overflowed its stack" and aborts on Unix; the Windows handler returns `EXCEPTION_CONTINUE_SEARCH` (std `sys/pal/unix/stack_overflow.rs:129-130`, `sys/pal/windows/stack_overflow.rs:20-27`, read in the nightly-2026-05-24 source).
- 0.160.0 already reads 7,904 B per unit on the FORMS "comma-nested" form, and a per-unit value that fits it needs a worker stack above the 256 MiB assert, which allows at most 7,102 B per unit (DL DEPS-BUMP-2).

### 7.4 Why 3 check rounds did not see it, and the review that did

| Check | Inputs | Result | Source |
|---|---|---|---|
| the bump builder and 3 check rounds (r1, r2 with fixes; r3 PASS) | `measure_forms` on FORMS, debug and release; the depth tests; lock, bytecode and test262 equality | PASS | OL M6-ORCH-1426 |
| the side CI of the 0.159.0 head (run 45242765620, 4 targets) | the debug depth tests on FORMS; test262 parse and exec; the corpus digest | 0 failed (cancelled at 66 of 68 jobs) | OL M6-ORCH-1433 |
| a second-model review of the bump | it added 8 "long list" forms to a copy of the stack probe | "nested calls with longer argument lists exceed the documented debug budget" (unverified; the run stopped before its report) | OL M6-ORCH-1427 |
| one verifier of that claim (1 agent) | base and head, debug and release, a deep input on the real worker | verdict R: true; the tables of §7.2 and §7.3 | OL M6-ORCH-1429 |
| the hold | oxc back to 0.158.0 in DEPS-BUMP-2; `measure_forms` worst values equal the base | merged (CI 45256467421, 68 of 68) | DL DEPS-BUMP-2-OXC-HOLD; OL M6-ORCH-1441 |

Why the checks could not see it (M, from the code):
1. The budget proof measures FORMS only (`probe.rs:238`). FORMS has no spread arguments, no nested call with arguments, no tagged template with substitutions and no long switch clause (`forms.rs:85-90`, `:103-108`, `:177-182`, `:225-230`).
2. The depth tests check "no abort", not "the 1.5x margin". At 0.159.0 the worst input still fit (230.4 MB of 232.5 MB), so no test could fail on aarch64.
3. `check_ast_form` runs each form at the AST cap, not at the pre-scan cap (`depth_tests.rs:64-80`). The WRAP list of the mix test has `f(` and `[...` but no list before the nested item (`depth_tests.rs:283-302`).
4. A check round that runs the same proof on the same set gets the same answer. The 3 rounds were equality checks of the bump; none added an input.

**The general rule (R17P §2.1, rule R-COVER):** a check of a measured budget (stack, size, time, count) must (1) name the input set that the measurement covers, (2) add inputs outside that set, at least each grammar or code form that the change touches, and (3) never count "the existing test passes" as evidence for the budget. An unverified reviewer claim gets one verifier before any action.

### 7.5 How four engines bound parser recursion (upstream source, verified)

| Engine | Check | Where (upstream path:line at the commit) | On overflow |
|---|---|---|---|
| V8 | `CheckStackOverflow()`: `if (GetCurrentStackPosition() < stack_limit_) set_stack_overflow();` | `v8/v8@932ddd7ca src/parsing/parser-base.h:942-945`; called at `:2109`, `:2126` (`ParsePrimaryExpression`), `:3466` | "Any further calls to Next or peek will return the illegal token" (`:943`): the parse ends by its normal error path |
| JSC | `failIfStackOverflow()` = `if (!canRecurse()) failWithStackOverflow()`; `canRecurse()` = `m_vm.isSafeToRecurse()`, a compare with `m_stackLimit` | `WebKit/WebKit@f30433b25 Source/JavaScriptCore/parser/Parser.cpp:57`, `parser/Parser.h:1907-1910`, `runtime/VM.h:870-873`; called at `Parser.cpp:755`, `:1164`, `:1259`, `:1284`, `:2133`, `:4181`, `:4293`, `:5235` | a syntax-error-path failure, no abort |
| SpiderMonkey | `AutoCheckRecursionLimit recursion(fc_)`; `check()` -> `checkWithStackPointerDontReport(fc, __builtin_frame_address(0))` -> `checkLimitImpl(fc->stackLimit(), sp)` | `mozilla-firefox/firefox@ec287ddb0 js/src/frontend/FrontendContext.h:268-287`; called at `js/src/frontend/Parser.cpp:3917`, `:4215`, `:4351`, `:9308`, `:9555`, `:10140` | `ReportOverRecursed` (`FrontendContext.h:271`): a "too much recursion" error |
| QuickJS | `js_check_stack_overflow(rt, 0)`: `sp = __builtin_frame_address(0) - alloca_size; return sp < rt->stack_limit` | `bellard/quickjs@e585640ff quickjs.c:2094-2099`; called in `next_token` (`:23462`, check at `:23469-23471`) and `json_next_token` (`:24110`, check at `:24116-24118`) | `js_parse_error(s, "stack overflow")` |
| oxc (ep9's parser) | none | `oxc-project/oxc@3464a0d18 crates/oxc_parser/src`: 0 matches for a stack or recursion limit | the process aborts (§7.3) |

- All four compare the real stack pointer with a limit at each recursion step and return a normal error. None of them uses a worst-case byte budget.
- ep9's own VM uses the same form: a native margin and a check at each step of a recursive builtin (H23). The front end is the one place where ep9 cannot do it, because oxc gives no control during the parse.
- R17M checked the only per-token extension point of oxc 0.158.0 (`LexerConfig::byte_handlers()`): the bytes `(`, `)` and `;` bypass the table, and the real handlers are private, so no hook exists (R17M §1.2; not read again for this report).

### 7.6 The options

| Option | What changes | Fixes 0.159 / 0.160 debug | What breaks | G8 (low / high) | Cost (E) |
|---|---|---|---|---|---|
| (a) / (a') raise the debug ceiling: the measured debug PARSE_PER_UNIT + slack, a debug-only ceiling of 512 MiB, the release ceiling stays 256 MiB | `stack.rs`; FORMS gets the missing forms | yes: 9,216 + 1,024 = 10,240 B -> 368.2 MiB (D); 512 MiB allows up to 14,260 B per unit (D) | nothing JS can see; 232 -> about 368 MiB virtual reservation per compiling thread in debug | 0 / +1 | 1.0-1.5 agent-days inside the bump |
| (b) count a list frame as 2 units in the pre-scan | `prescan.rs` | yes (debug) | the estimate rule of `depth.rs:33-36`; `f(0,f(0,...))` hits the cap at 12,500 levels in release too | +1 / +1 | 2-3 agent-days |
| (c) `[profile.dev.package.<oxc>] opt-level` override | root `Cargo.toml` | only for builds whose workspace root is ep9 | `fuzz/`, spikes and any embedder build without it; the debug constant must stay at the opt-0 value | 0 / +1 | 0.5 agent-days, no safety gain |
| (d) stay on 0.158.0 | nothing | no (0.158.0 is 6,384 > 6,199) | the hold grows each release; 0.160.0 is the speed release (parser instructions -11.2% on Octane) | 0 / 0 | 0 now, a rising port cost |
| (e) a stack-pointer check in the parse (the engine form of §7.5) | a local oxc patch at the recursive entry points | yes, for every later release | a patch at each 14-day bump; reopens H23-Q10 (`docs/adr/H23-limits-and-failure-policy.md:111`) | +1 / +2 | 5-10 agent-days + a patch per bump |
| (f) lower caps in debug only | `depth.rs` | yes | a test passes in release and fails in debug for depths 15k-20k; the H01 Rules row "debug and release" (H01:42) | +2 / +2 | 1-2 agent-days |

Source: R17M §1.3 (the costs and G8 readings are R17M's estimates); the derived numbers are R17M's and were checked for this report (10,240 x 25,000 x 1.5 + 2 MiB = 386,097,152 B = 368.2 MiB).

**Recommendation of R17M (a recommendation, not a decision; the decision is a plan v17 row):** option (a'), in one set with the missing FORMS and oxc 0.160.0 with its PropertyKey port, in DEPS-BUMP-3 (about 2026-12-08). New tests: `measure_forms` with the new forms as a CI step on the 4 targets (debug), and each form at the pre-scan cap in a child process (debug and release). Fallback: if 0.160.0 needs more than 14,260 B per unit in debug, or any target aborts, hold oxc again and land FORMS at 0.158.0 with a debug PARSE_PER_UNIT of 6,656 (WORKER 251,697,152 B, under 256 MiB). Option (e) goes to month 13 as a design, with an upstream feature request as an owner call.

---

## 8. Process findings worth keeping

| Topic | Finding | Number | Source |
|---|---|---|---|
| boxes by kind (12.20 box-h before the end read) | lever A/B 8.09 (66%), cause reads 2.01 (16%), Path B 2.10 (17%), fuzz 0 | + end Path B 1.75 = 13.95 of 15.9 | R17P §1.1; DL M11-END-PATHB |
| the A/B form cost | a pooled 2-box A/B: 1.43-1.98 box-h; one more box: 0.88 box-h; a TD-form read: 0.38 box-h | each A/B came in at or under its low estimate | R17P §1.1-§1.2 |
| the A/B form resolution | 1-box A/A width 0.0227-0.0300; 2 boxes pooled (62 pairs) 0.0147-0.0169; 3 boxes (93 pairs) 0.0089 | a 1-box read passes only for a lever of about 2.5% or more (D) | R17P §1.5 |
| the cycles line | cycles and wall medians on the same pairs differ by 0.0003-0.0009; the 0.988 cycles line alone sent STAR-DST to one more box (0.9890 against 0.988) | 0.88 box-h | R17P §1.5; DL M11-P3-L16-ONE-MORE-BOX |
| checks that changed an outcome | the second-model review on code slices (the debug-stack defect; 19 findings in M11-P3); the box-script checkers (33 findings in 4 sets); the pre-registration checkers; the single verifier of a reviewer claim (verdict R) | - | R17P §2.4 |
| checks that did not | 3 tag CIs (68 of 68 each, 0 code defects); the second records reviewer (0 (a)/(b) in 3 sets); about 9 DL fact checks (0 wrong gate numbers); the 3 DEPS check rounds (missed the stack growth) | - | R17P §2.4 |
| the records load | 121 DL rows, 336,428 characters in DL §21; records 12.68 M tokens (31%) against engine work 5.76 M (14%) | 41.52 M tokens, 69 workflows, 202 agents | R17P §0, §2.3, §5.1 |
| tokens per 1% of jitless time | month 11 about 7.5 M; month 10 24 M | D | R17P §2.3; R22 §5.1 |
| the largest waits | the Mac quiet window 28.0 h; the end Path B wait for the bump 11.9 h; a weekly usage stop 6.0 h; the lever pick after the K2 KILL 3.4 h | - | R17P §2.2 |
| a lost start | LAYOUT-2 try 1 failed before any box: a top-level `set --` replaced the arguments of the detach re-run; the dry runs did not reach that path | 0 box-h lost | DL M11-PRE-LAYOUT-2-READ |

---

## 9. R22 addendum: its decisions and causes after month 11

### 9.1 Decisions D3 and D7

| R22 item | What R22 said | What happened | Status | Evidence |
|---|---|---|---|---|
| D3 | settle C2 before any new lever: the bytes-against-placement compare (X1) and a LEVER-13 A/B with REG-SLOT K2 on the first month-11 box; the experiment verifier put LAYOUT-PIN first | LAYOUT-PIN ran first (MIXED). Plan v16 changed the order: LDAR-ELIDE before K2, K2 before the LEVER-13 A/B. The LEVER-13 A/B ran as LAYOUT-2 (cause DRAW). The alignment rule merged | carried out, order changed (named) | DL M11-PLAN-D4, M11-PRE-LAYOUT, M11-PRE-LAYOUT-2-READ, M11-P1-RULE-VERDICT |
| D7 | a generated-interpreter design study (read only) in month 11 | the handler-ABI option paper exists; no generated interpreter and no handler-ABI build in M2 | done (paper); the build is open, after M2 | DL M11-PLAN-D8 |

### 9.2 Causes C1-C6

| Cause | R22 claim | After month 11 | Status | Evidence |
|---|---|---|---|---|
| C1 | on x86, ep9 T0 runs more cycles than node --jitless at about equal instructions (IPC gap) | at `m10-p5`: +5.2% cycles at 4.7% fewer instructions, CPI +10.4% (TD-2, 1 box). Month 11 removed dispatches: est. 24.86 G cycles against node 24.98 G at `m11-p3`; the wall gap is still 2-3% | confirmed at `m10-p5`; reduced (E); the rest is open (§5) | TD2:22-24; §5.2; DL M11-END-PATHB |
| C2 | the jit binary became 5.8% slower than the jitless binary at LEVER-13 | a placement draw (LAYOUT-2: S_d +0.0542, S_a -0.0058); removed by the rule on 3 trees (S_a 0.0091); end read same-round 0.997 / 0.995 | **confirmed and closed** | DL M11-PRE-LAYOUT-2-READ, M11-P1-RULE-VERDICT, M11-END-PATHB |
| C3 | the levers kept the Ir meter after the instruction gap closed | REG-SLOT -8.0% instructions gave +1.0% time; LDAR-ELIDE -3.0% gave -4.1%. Rule 93 made every hot-handler lever a time A/B | confirmed (stronger); closed as a process cause | DL M11-P2-K2-VERDICT, M11-P0-LDAR-VERDICT, M11-PLAN-D3 |
| C4 | the time meter is too coarse (1.9-6.3%) and rationed (3 A/B a month) | pooled 2-box widths 0.015-0.017; 6 reads with a time verdict (2 cause reads, 4 lever A/Bs); no count cap | closed (process) | R17P §1.5; DL M11-PLAN-D3 |
| C5 | the phase template takes 45-50% of the tokens | records 31% + plan and design 21% of 41.52 M; tokens per 1% of jitless time 24 M -> about 7.5 M | partly reduced; open | R17P §2.3 |
| C6 | the procedure tail (Mac windows, a 66 h fuzz batch) | 0 counted Fuzzilli CPU-h; the Mac block A window took 28.0 h of launcher wall; the counted batch still needs about 131 box-h and about 66 h | open, unchanged | R17P §2.2, §1.3; DL M11-PLAN-D21 |

### 9.3 R22 hypotheses (§10.2, §11) after month 11

| R22 item | Status now | Evidence |
|---|---|---|
| the jit fall is a layout effect | confirmed (DRAW, 1 box; the rule, 2 boxes, 3 trees) | DL M11-PRE-LAYOUT-2-READ, M11-P1-RULE-VERDICT |
| the box link sits +32 B (mod 64) from the cross link | not supported: the LAYOUT-PIN box read e-jit 48, e-jitless 16 (want 32 and 0): "DOES NOT AGREE" | DL M11-PRE-LAYOUT |
| the x64 frames and decode cost x86 time (REG-SLOT) | refuted: K2 KILL in both layout forms; the data exclude a 2% gain | DL M11-P2-K2-VERDICT |
| the 4-load feedback chain costs about 17% per load (Q3) | not supported at the bucket level: 22.42 against 22.41 cycles per named load, no `Star` | TD2:212, :216-217 |
| the elision of dead `S> Ldar` gives x86 time | confirmed: x1.032 (arm 1), x1.043 (arm 2) | DL M11-P0-LDAR-VERDICT |
| Q2: the buckets hold on `m10-p5` code | `Ldar` +0.790 G and Wide +0.646 G hold; the named load +0.003 G does not | DL M11-PRE-TD2-READ |
| Q5: the linux-x64 node build config | read: pointer compression 0, sandbox 0, `v8_enable_short_builtin_calls=1` | TD2:303 |
| Q7: item 3 on the EPYC 7763 | one GitHub Linux leg at `m11-p0`: jit 1.015 [1.013-1.020], jitless 1.012 [1.008-1.016]; the Windows 7763 pair 0.948 / 0.971-0.981 | OL M6-ORCH-1343 |
| Q1, Q6 (harness confound, BOLT) | not run (cut step 1) | DL M11-P3-L16-ONE-MORE-BOX |

---

## 10. Verified, not verified, limits

### 10.1 Verified

- Every lever number of §2 against its DL verdict row; the LDAR, K2 and STAR-DST pooled read texts; the CI TS rows 45021005639 and 45130289111 through OL M6-ORCH-1336 and -1406.
- The Path B series of §4.1 against PBm, PBe and DL M10-END-D3; the box-neutral values as 1 / the QuickJS-ratio medians.
- The TD-2 rows of §5.1 against TD2.
- The stack tables of §7.2 and §7.3 against SV and its logs (the `VROW` lines and the deep-input lines); every ep9 file:line of §7.1 at e1c1d45b.
- Every engine cite of §7.5 in the named upstream commit (V8, JSC, SpiderMonkey, QuickJS), and the 0-match grep of the oxc parser source at 3464a0d18; the Rust std abort lines.

### 10.2 Not verified

- The cause of the flat jitless cell (§4.3): 5 open candidates, no read.
- The per-unit cycle numbers of §2.2 and §3.2 mix the TD-2 box with the A/B boxes (E).
- The "node gets about 2.3% of wall off its main thread" reading (one box, all-thread samples; SMT and clock effects not excluded).
- The GC-like shares (a symbol-name match, 1 record per engine).
- The opcode pair counts of §5.3 (a script of R17S; `m10-p5` dump; not re-run for this report).
- The process numbers of §8 (token and wait sums of R17P; not recounted).
- The oxc lexer hook claim of §7.5 (R17M; not read again).
- The candidate gains of §6 (estimates of R17S; the model is the one that matched STAR-DST once).
- x86-64 and Windows debug stack frames (not measured anywhere).

### 10.3 Limits

- One box type (Cloudflare standard-4, EPYC 9684X, SMT on, 2 CPUs pinned); no gate host.
- One cycle map (TD-2, 11 rounds, 1 box, `m10-p5` code); none at `m11-p3`.
- The Path B reads are not paired with each other; each is 31 rounds per box. The end boxes ran TS 20-23% faster in gross time than the mid boxes.
- The A/B-to-Path B transfer is known for one lever only (§3.3).
- TS is the decision workload; richards, deltablue, raytrace, t0-branchy and t0-prop are guard rows only.

## Raw numbers

[artifacts/m2-month11-levers/](artifacts/m2-month11-levers/README.md) holds `findings.json` (one object per finding, with its status, sources and numbers) and the tables behind §2, §4 and §7 as TSV files.
