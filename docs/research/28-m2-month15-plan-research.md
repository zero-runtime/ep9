# The month-15 plan research: why the month-14 set missed the floor, the SWEEP-LEAN x86 read path, the cuts left, and the month-15 calendar

## Run context

| Item | Value |
|---|---|
| Dates | The round: 2026-11-26 18:47-19:01 +0800 (OL M6-ORCH-1684 starts it; the 4 lens notes are written 18:48-19:01). The later data: the DL 24.5 rows at 19:03 (OL M6-ORCH-1685). This record: 2026-11-26 |
| Round | The plan-v20 research: 4 read-only lens notes (model, sweep, levers, calendar), then this writer. The writer reads the 4 notes and checks the 3 facts that carry the most weight again (§9.1). It runs no box, no build and no new measurement. The model lens runs one simulation on the month-13 odds model code (read only; 32,000 reads per case) |
| What was known at the round | Every month-14 read row of DL 24.3-24.4; the box summaries of the SIMD box and of the 2 SWEEP boxes; OL M6-ORCH-1640..-1683, with the second model read (ALLOC-SET NOT RUN) and LEVER-21 NOT RUN (OL M6-ORCH-1682). Not known: the A1H-JOIN and oxc 0.160.0 scout results, the month-14 end row, FUZZ-H, plan v20 |
| ep9 trees | DL rows at c5a4bc1c (`m2m14/prereg`; the base of this record). The month-14 base code is `m13-p1` 0b710261. Role heads: CTOR-FIT 31e7a842, SHAPE-CACHE 1993ee22 (code = 66760b14), JIT-LEAN 3a94b0f4, SIMD-DISPATCH 7411b488, SWEEP-LEAN 8386386e; the integrator `m2m14/p1-alloc-set` b23db7cf (origin; the S2 commits f0dcdb7c..0028271a are local only, OL M6-ORCH-1682). Each code cite is `git show <sha>:<path>` at the named sha. The DL 24.5 rows are at f87b2936 (`m2m14/prereg4`, written at 19:03; not in the base of this record) |
| Boxes | No box in the round. The data it reads: ep9-x86-m14-simd-1 (26 min, 0.43 box-h; OL M6-ORCH-1677), ep9-x86-m14-sweep-1 and -2 (47 min each, 0.78 box-h each; OL M6-ORCH-1682). All 3: Cloudflare standard-4, AMD EPYC 9684X |
| Upstream | `v8/v8@932ddd7ca4`, `WebKit/WebKit@f30433b25d` (the clone heads of R25-R27); the Rust std 1.95.0 source (the toolchain source on the research host) |
| Method | Read only. Every number has a cite. **M** = measured (a box, a CI run, a count file, a source read; the file is named). **D** = derived (the formula or the inputs are shown). **G** = a guess. **E** = an outside fact (the source is named). "D on G" = arithmetic on a guessed input. Note: R27 used E for an estimate. In R28, E is an outside fact only; an estimate is G or D on G. A forecast is never stated as a result |
| Limits | No month-14 A/B box ran, so every set gain is a model value. Every cycle price comes from one TD-4 box. The A/B odds come from one model (the month-13 form), with tau from 60 null cells. The lens notes are not in the repo; this record carries their numbers |

Short forms:
- DL = [docs/decisions-log.md](../decisions-log.md), by row id; DL:n = line n of it at c5a4bc1c. OL = the orchestrator log, by row id M6-ORCH-n (not in the repo).
- P19 §n = a section of plan v19 (orchestrator copy, not in the repo). V19-Dn = DL M14-PLAN-Dn.
- LM, LS, LL, LC = the R28 lens notes on the model, the sweep path, the levers and the calendar (not in the repo).
- TD4:n = line n of the TD-4 summary (one box on `m13-p1`; DL M13-TD4-READ). cg = the x64 TS cachegrind files of the JIT-LEAN gate run (base 0b710261; not in the repo).
- R25 = [25-m2-month13-plan-research.md](25-m2-month13-plan-research.md); R26 = [26-m2-month13-alloc-simd.md](26-m2-month13-alloc-simd.md); R27 = [27-m2-month14-plan-research.md](27-m2-month14-plan-research.md).
- Sets: core = CTOR-FIT + SHAPE-CACHE. S1 = core + SRC-HASH (the set of the second model read). S2 = S1 + SWEEP-LEAN.
- Ratios as R27: an A/B gain x1.0nn = t(base) / t(lever). A gate ratio = t(node --jitless) / t(ep9). "pt" = point = 100 x a ratio difference. k = cycles per removed instruction in the set line model. tau = the spread term of the null A/B cells (DL M14-PRE-AB94).

---

## 0. Verdict

```
 month 14 (DL M14-P1-ALLOC-CUT; OL M6-ORCH-1682)   ALLOC-SET NOT RUN; LEVER-21 NOT RUN; branch N; no end Path B, no TD-5
 the second model read, k 0.495                     1.009903 x 1.004725 x 1.000 = x1.014675 < x1.015 (short 0.032 pt)
 need at the m13-p1 low ends (no month-14 read)     jit x1.0173 / x1.0152          jitless x1.0070 / x1.0040   (R27 §0)
 M14-PRE-MONTH15 line (5)                           applies: K is the default; H and R are owner actions

 the round's answers (estimates, not results)
 Q1 model     the miss is a model miss, not a speed fact: 92% of the CTOR-FIT term has a G input;
              u 90..180 gives S x1.0120..x1.0181, so the floor is inside the G band (LM)
 Q2 sweep     the job shell pin (taskset CPU 0) put exec and its workers on 1 CPU: load 1.00-1.05, 0 files in 41 min (LS)
 Q3 levers    JIT-LEAN fell 57.1 M against the 100 M line; 2 new cuts: CHARCODE x1.010 / 1.015 / 1.019,
              TO-BOOL x1.004 / 1.006 / 1.009 (G) (LL)
 Q4 calendar  month-15 base: A1H-JOIN -> SIMD-DISPATCH -> DEPS-BUMP-3 (dispatch 2026-12-08, not before);
              modes check by 2026-12-03 04:49 +0800; month 14 used 1.99 of 15.9 box-h, no carry (LC)
 S2 = S1 + SWEEP-LEAN   x1.015182 (margin 0.018 pt); A/B MERGE 54.5% at tau 0.0051 (D on G)
```

1. **Month 14 merged no speed lever (M).** The second model read gives S1 = x1.014675, under the floor x1.015 by 0.032 pt. ALLOC-SET is NOT RUN, and LEVER-21 is NOT RUN (OL M6-ORCH-1682; DL M14-P1-ALLOC-VERDICT at f87b2936). The m13-p1 end read stays the last gate read.
2. **The miss is inside the model's guess band (D).** The CTOR-FIT term (x1.009903) is 92% G inputs. One input moved by a small step closes the 0.032 pt; for example u 130 -> 134.8, or SHAPE-CACHE dIr 199.4 M -> 212.9 M (§2.2).
3. **The 2 price forms of CTOR-FIT differ by 2x (D).** The registered allocation form gives x1.0099. The instruction form on the measured x64 jitless fall (826,955,011 Ir) gives x1.0199. No wall read anchors either form (§2.3).
4. **S2 needs only the SWEEP-LEAN x86 line (D).** S2 = x1.015182. The x86 line read NOT READ because the box script pinned the job shell to CPU 0 (M). The test262 runner then ran its shard on 1 CPU (M: load 1.00-1.05; D: 1 worker by its default count) (§3.1). A CI dispatch with one new input can read it with 0 box-h (§3.2, C1).
5. **S2 alone does not fire the switch (D on G).** At the R27 planning t (0.626 jit, 0.662 jitless), S2 at center lifts the jit low ends to 0.992 / 0.994 and the jitless low ends to 1.003 / 1.006 (§6.2).
6. **The largest single cut is new (G).** CHARCODE (a `charCodeAt` arm) is est. x1.010 / 1.015 / 1.019 in both builds. S2 + CHARCODE at center is the smallest pair that lifts all 4 low ends to 1.000 or more (§4.4, §6.2).
7. **tau drives the A/B odds, not the form (D on G).** At tau 0.0051, S2 has MERGE 54.5% and false GUARD-FAIL 39.9% (k 14). At tau 0 it has 82.2% and 8.0%. Inside form F1b only fewer guard cells help (§2.4).
8. **The month-15 order is fixed by registered rows (M).** A1H-JOIN, then SIMD-DISPATCH, then DEPS-BUMP-3 on or after 2026-12-08. A bytecode lever (JUMP-INVERT) cannot be open at the bump boundary (§5).

---

## 1. The questions of the round

| # | Lens | Question | Answer in |
|---|---|---|---|
| Q1a | model | Why is S under x1.015, and by which input? | §2.1, §2.2, §2.3 |
| Q1b | model | Which month-15 set forms reach the floor, and what does each need? | §2.5, §6.1 |
| Q1c | model | What are the A/B odds of S2, and which form change moves them? | §2.4 |
| Q2a | sweep | Why did both SWEEP halves hit the time cap? | §3.1 |
| Q2b | sweep | What is the cheapest correct x86 read of the SWEEP-LEAN line in month 15? | §3.2 |
| Q2c | sweep | What does the check-all mode cost in CI time (Miri, test262-gc)? | §3.3 |
| Q3a | levers | Why did JIT-LEAN fall 57.1 M, not the design 120 M? | §4.1 |
| Q3b | levers | What is the price of LIST-FIT (a)? | §4.2 |
| Q3c | levers | Is MEGA-LEAN closed by NAMED-COUNT, or does a new fact reopen it? | §4.3 |
| Q3d | levers | Which cuts give the most est. gain per agent-hour? | §4.4 |
| Q4 | calendar | DEPS-BUMP-3, A1H-JOIN, SIMD-DISPATCH, the modes check, the box budget and fuzz in month 15 | §5 |

---

## 2. Model lens: the set line and the A/B odds (LM)

### 2.1 Why S1 = x1.014675 (the registered model, DL M14-P1-ALLOC-CUT)

CTOR-FIT: g = 1 / (1 - s), s = (G x u / C - m) + b x gcm x w + b x f + c.

| Term | Inputs | Value (pt) | Mark |
|---|---|---|---|
| grow | G 1,393,441 (Mac count) x u 130 / C 20.985 G (TD4:21) - m 0.28 | 0.8632 - 0.28 = 0.5832 | G (u, m), M (G on Mac, C) |
| GC | b 11.93% (Mac count) x gcm 2.59% (TD4:247) x w 0.7 | 0.2163 | M (b on Mac, gcm), G (w) |
| fill | b x f 0.68% | 0.0811 | D |
| c | a constant | 0.1 | G |
| s | sum | 0.9806 -> g = x1.009903 | D |

SHAPE-CACHE: g = C / (C - dIr x k) = 20.985 / (20.985 - 0.199364 x 0.495) = x1.004725 (D). dIr 199,363,544 is the x64 jitless TS fall against the CTOR-FIT head (M; OL M6-ORCH-1670). SRC-HASH and the riders count at their low: SRC-HASH x1.000, SWEEP-LEAN x1.0005, LIST-FIT x1.000 (G; DL M14-P1-ALLOC-GATE).

- 92% of s comes from terms with a G input (D: (0.5832 + 0.2163 + 0.1) / 0.9806 = 0.917).
- G and b are Mac counts. The gate row says "from the CTOR-FIT gate run", but the x64 gate run prints no grow count and no byte count (M; OL M6-ORCH-1663; DL M14-P1-ALLOC-CUT states it).
- k 0.495 = 0.189 G cycles of `lookup_with` (TD4:88) / 382,048,291 Ir (the K3 cachegrind) (D). It prices `lookup_with` only, and it mixes 2 trees. The gate row picked it after k 0.410 (1 / IPC 2.44) failed the set line, before any gate data (O3, DL M14-P1-ALLOC-GATE). 0.410 is a printed check line: S1 x1.0139 (DL M14-P1-ALLOC-CUT).
- SHAPE-CACHE net cost: 32.4 Ir per call saved (D: 199,363,544 / 6,147,592 calls) against the design 35.5 (G). The x64 / Mac fall ratio is 0.67 (D: 0.3986% / 0.59%).

### 2.2 One input closes the 0.032 pt (D)

| Input | Read value | Value that gives S1 = x1.015 |
|---|---|---|
| u (G) | 130 | 134.8 |
| w (G) | 0.7 | 0.803 |
| m (G) | 0.28 pt | 0.248 pt |
| c (G) | 0.1 pt | 0.132 pt |
| G (M, Mac) | 1,393,441 | 1,444,597 |
| b (M, Mac) | 11.93% | 13.20% |
| dIr (M, x64) | 199.4 M | 212.9 M (also DL M14-P1-ALLOC-CUT) |
| k (D) | 0.495 | 0.5285 |

- u from 90 to 180 gives S1 from x1.0120 to x1.0181 (D). The floor is inside the G band of one input. So the miss is not a speed fact; it is a model fact (D).

### 2.3 Two price forms, no anchor (D)

| Form | Formula | CTOR-FIT term | Registered for CTOR-FIT? | Source |
|---|---|---|---|---|
| allocation form | 1 / (1 - s), §2.1 | x1.0099 (s 0.98 pt) | yes | DL M14-P1-ALLOC-GATE, -ALLOC-CUT |
| instruction form | C / (C - dIr x k), dIr 826,955,011 (x64 jitless fall) | x1.0199 (1.95 pt) | no (SHAPE-CACHE only) | OL M6-ORCH-1658, -1663 |

- k 0.495 prices `lookup_with` only; R26 §1.2 prices removed units. So neither k is a CTOR-FIT price. The forms differ 2x, and no wall read anchors either one (D).
- OL M6-ORCH-1663 records the orchestrator error: OL M6-ORCH-1658 first gave the instruction form for the set term.

### 2.4 The A/B odds (D on G; the month-13 odds code, read only; 32,000 reads per case)

Band B = 1.000e-04, the cap (M; DL M14-PRE-AA-BAND-READ: largest abs(d) 4.324e-05, 3 x = 1.297e-04). The TS jitless null cell -4.32e-5 is above 3.3e-5, so that row is a LEVER cell. This has no effect on the S2 guards. It blocks the JIT-CELL clause for a jit-only arm (D). The merge-head clause (2) on TS jitless: 1e-4 = 2.9 sd of abs(dIr - d), a false stop of about 0.4% (D).

S2 at center (TS ratio 0.98504 = 1 / 1.015182), 14 guard cells, all LEVER:

| tau | MERGE % | false GUARD-FAIL % | ONE MORE BOX % |
|---|---|---|---|
| 0 | 82.2 | 8.0 | 30.7 |
| 0.0025 (95% low) | 76.2 | 14.8 | 34.4 |
| **0.0051 (registered)** | **54.5** | 39.9 | 41.4 |
| 0.0079 (95% high) | 28.3 | 69.2 | 37.4 |

- tau(60) = 0.0051 (95%: 0.0025-0.0079) (M; DL M14-PRE-AB94, the Q form).
- S1 at tau 0.0051: MERGE 51.7%. So the 0.032 pt costs about 3 pt of odds (D).
- Other cases at tau 0.0051 (D on G): the jit fraction 0.9 (G) is in the base case; the WIDE jit/default fraction (0.19) gives MERGE 44.0%; a true ratio of 0.990 gives 12.8%.
- The re-run of the AB94 family rate: 36.8% (AB94 printed 36.6%) (D).
- Expected box-h of the S2 A/B: 3.3 + 0.414 x 1.65 = 3.98 (D).

### 2.5 What moves the odds (D on G)

| Change | Effect at tau 0.0051 | Cost or catch |
|---|---|---|
| k 8 guard cells (the F1 rows) in place of 14 | family false GUARD-FAIL 36.8% -> 22.8%, still above the 20% line of rule 104 (V19-D9); S2 MERGE 54.5% -> 67.7% | it drops the earley-boyer row (P19 §4.2) and the splay rows (a named mechanism): an owner call |
| O16 (pair noise cut) | false GUARD-FAIL 39.9% -> 42.7% | pair noise falls, tau stays: no gain |
| rule 117 | saves box-h, not odds | - |
| IDENTITY cells | none apply (O4) | - |
| a lower tau | 0.0025 gives MERGE 76.2% | env size is already equal across arms (the DL arm lines); the cause of tau is not known (G) |

- Month 14 adds 0 wall null cells to the ledger (M: no A/B box ran).
- E: V8 keeps a `DescriptorLookupCache` of 64 entries and clears it before each GC (`v8/v8@932ddd7ca4` `src/objects/lookup-cache.h:18`, `:46`). SHAPE-CACHE is the same kind of cache (flushed at every GC; DL M14-P1-SHAPE-CACHE-READ).

### 2.6 The earlier forecasts against the month-14 data

| # | Forecast (source) | Month-14 data | Status |
|---|---|---|---|
| FC-a | CTOR-FIT x1.005 / 1.010 / 1.018 (R26 §6; R27 §1.6) | gate PASS; set term x1.0099 in the registered form (DL M14-P1-CTOR-FIT-READ, -ALLOC-CUT) | a model value, no wall read |
| FC-b | JIT-LEAN jit x1.003 / 1.006 / 1.010 (R27 §1.6); design 120 / 153 / 176 M (OL M6-ORCH-1645) | 57,081,502 Ir against the 100 M line: out (DL M14-P1-JIT-LEAN-READ) | missed |
| FC-c | SHAPE-LOOKUP-CACHE x1.006 / 1.007 / 1.008 (R26 §7); the gate-row margin used x1.0060 and asked dIr >= 229.5 M (DL M14-P1-ALLOC-GATE) | gate PASS; dIr 199.4 M; term x1.0047 | under its low |
| FC-d | the set S at center >= x1.015 (V19-D5; the gate-row margin x1.0151, D on G) | x1.014675 at both model reads | missed by 0.032 pt |
| FC-e | NAMED-SET only after a 0-box count shows D1 misses on 50% or more of the probes (R27 §1.6) | NAMED-COUNT: lines 1 and 3 MET, 2 and 4 NOT MET (DL M14-P0-NAMED-COUNT-READ) | MEGA-LEAN not in the LEVER-21 list |
| FC-f | the splay fall: cause not read (R27 §4) | SPLAY-CAUSE FAIL: GC-BUDGET is not a cause by its line; node got 13.4-15.7% faster on splay, ep9 11.2-15.1% (DL M14-P0-SPLAY-CAUSE-READ) | closed by its line |
| FC-g | AA-NOISE-1E5: replace the 1e-5 line by a band B (R27 §2.7 K1) | B = 1.000e-04 (the cap) on 165 cells (DL M14-PRE-AA-BAND-READ) | done |
| FC-h | month-14 exit about 10% (V19-D1; G) | branch N | no set ran |

---

## 3. Sweep lens: the SWEEP-LEAN x86 line (LS)

### 3.1 Why both halves hit the cap (M, with one E)

```
 req.sh:221-222   nproc >= 4 check            passes (it runs before the pin)
 req.sh:31, :237  PIN_RUN=0; taskset -pc 0 $$  the job shell is on CPU 0
                  children inherit the mask    cargo build, exec and its workers: CPU 0
 exec default     --threads = available_parallelism() = 1 under the mask -> 1 worker
 steps.txt        shard-half-N rc=124 TIME CAP at +46 min, load 1.00-1.05; 0 files read
```

- M: `PIN_RUN=0` is at line 31 and `taskset -pc $PIN_RUN $$` at line 237 of the SWEEP box script req.sh. No other `taskset` line is in the box set. The form comes from the SIMD box set and the TD-4 set (the shell on CPU 0, the timed runs on CPU 2-3). The SWEEP set has no timed step, so it had no reason to pin.
- M: the test262 runner has `--threads <n>`, "default: all CPUs", with `default_threads()` = `std::thread::available_parallelism()` (`tools/test262/src/main.rs:57`, `:80-82` at 8386386e). The SHARD command of the box set passes no `--threads` (the box set build-cmd.txt, line 22).
- E: Rust std 1.95.0 `sys/thread/unix.rs:165-172`: on Linux the count is min(the cgroup quota, CPU_COUNT of `sched_getaffinity`). The repo toolchain is 1.99.1; its std source is not read here.
- M: SHARD load 1.00-1.05 for 41 min on both boxes; the build took 5 min (the box summaries and steps.txt of ep9-x86-m14-sweep-1 and -2).
- D: a half at 4 workers = (28.8 to 57.2 min) / 2 = 14-29 min (G: a box core = a hosted core). At 1 worker it is about 4x = 56-115 min, more than the 40.9 min left. `--threads 4` under the pin gives no gain.
- G: the exec header line in each half log says "1 worker processes" (not read: the lens rule).
- M: the CI job test262-gc passes no `--threads` either (`.github/workflows/ci.yml:629-631` at c5a4bc1c) on the hosted 4-CPU runners (`ci.yml:570`). The linux-x64 1/6 job took 28.8 min (m12-p0 tag CI), 57.2 min (m12-p1) and 52.3 min (m13-p1) (the rule-25 tables of those tag CI runs).
- OL M6-ORCH-1682 names the cause from the load line and calls it a (b) defect of the orchestrator design (OL M6-ORCH-1678) and of the box set: neither checked the worker count. It is a record, not a lost box (DL M14-P1-SWEEP-BOX-READ at f87b2936).

### 3.2 The cheapest correct read in month 15

| Path | What | Box-h | Needs | Marks |
|---|---|---|---|---|
| **C1 (CI)** | a dispatch input `sweep_check: off \| on` sets `EP9_SWEEP_CHECK_ALL` to "1" on the test262-gc exec step only; one step per job prints the CPU model, the avx512 flags and the SIMD-REPORT-SWEEP line (exec prints none); run with `only=windows-x64,linux-x64` | 0 | a ci.yml change on the read head; a new DL row before data (rule 96); the row says if windows-x64 counts as x86 | M: ci.yml has no EP9_SIMD or EP9_SWEEP line; D: the env is read once per process and only "1" turns it on (`ep9_platform` `simd.rs:984-1000`); workers inherit the env (`exec.rs:549`); push and PR runs do not change |
| **C2 (box)** | the SWEEP box set with no shell pin, `--threads 4`, a check for "4 worker processes" in the log, and nproc read after any pin | 2 boxes 0.67-1.17; 1 box for the whole 1/6: 35-63 min (cap 70 min = 1.17) | a new row, a new build-cmd sha256 | D: a half 14-29 min + 6 min set-up = 20-35 min; M: the EPYC 9684X has avx512_vpopcntdq (steps.txt) |

- The avx512-popcnt body (M: hosted x86 CPUs on record are EPYC 7763, 9V74, 9V45, Xeon 8370C, 8573C; DL:385, :2720, :2723, :5356). E: VPOPCNTDQ is on Intel Ice Lake and later and on AMD Zen 4 and later, not on Zen 3 (the Intel and AMD AVX-512 support tables). D: every job has sse2 and avx2; every CPU but the EPYC 7763 has avx512-popcnt. G: the CPU is a draw per job (month 3 had 10 EPYC 7763 reads, DL:2723).
- The PASS rule that C1 needs (LS proposal): one x86 1/6 job with avx512-popcnt in `bodies=`, rc 0, infrastructure errors 0. If both legs draw an EPYC 7763: re-run that job once; if still none, C2 for that body only.
- C1 runs the check-all mode in 1 shard run. A per-body `EP9_SIMD` run would take 3 (D).

### 3.3 The check-all mode and CI time

- D: Miri sets no `EP9_SWEEP_CHECK_ALL`, so the mode is off there, and 8386386e adds 0 Miri work. Only a workflow-level env would add the scalar and sse2 passes per checked page.
- The real Miri risk is 0ab800db: an sse2 shadow pass and a bitmap copy per checked page (M: the SWEEP-LEAN role check, line 23). Miri at m13-p1 took 21.5 min of the rule-25 limit 26.7 min (0.804), timeout 40 min (M: the m13-p1 rule-25 table; `ci.yml:1013`).
- NOT READ: pushes to `m2m14/*` run no CI (`on.push` = main), so 0ab800db and 8386386e have no Miri time. C1 reads it.
- test262-gc: up to 4 bodies and 4 page copies per checked page. Mac subset (1043 files): check-all 48.3 s, neon 56.1 s, scalar 88.0 s (M: the sweep count dir, checkall and gcstress logs; noisy): no cost seen. Room: rule 25 84.7 / 57.2 = 1.48x; timeout 127 / 57.2 = 2.22x (D).

---

## 4. Levers lens: the cuts left (LL)

### 4.1 JIT-LEAN fell 57.1 M, not 120 M (M: cg jit at 3a94b0f4; DL M14-P1-JIT-LEAN-READ)

```
 C1  26.0 M (design 54-73)   the lean path costs 41.0 Ir/call (design 22.5-30)
     nullish_pair_bits 21 Ir/call (design 8-9): it tests l, then r, then `other` again
       (profile.rs:1164 4, :1166 4, :1171 5, :1173 4; bits.rs:63 3)
     the rbx push (F1): dsl.rs:661 +1, :687 +1 = 5.1 M
 C2  32.9 M (design 66-103)  array_index -36.3 M (calls -1,425,065: as designed)
     but GetKeyedProperty_cold +3.19 M (+1.34 Ir/entry; design -17); libc malloc +1.8 M
 total 57,081,502 against the line ">= 100 M" (O1): FAIL; out at the cut
```

- The design estimate was 120 / 153 / 176 M (OL M6-ORCH-1645).
- earley-boyer +0.2846% (+12.77 M) (M): 3,871,032 calls, about 0 of them nullish (`:1171` 4,560 Ir). The +3.4 Ir per call is the pre-test on pairs that are not nullish. A variant must not run the test on those pairs.
- A variant (test acc first, no second test, no rbx push) gives about +25 M, so about 82 M (G). 100 M or more needs a new C2 class form; no one has designed it (G).
- Size: at 100 M the jit gain is x1.0023; now x1.0013 (D: 21.267 / (21.267 - dIr x 0.487), TD4:20-21).

### 4.2 LIST-FIT (a)

- The 0.1% line = 51.5 M = 96 Ir per call (D, on 538,940 `new Array()` host calls; R26 §6.3).
- One `new Array()` today (M, cg jitless): `Construct_cold` 14, `host_construct` 24, `is_constructor` 11, `array_constructor` 43 = 92 Ir. Add the `slow::call` entry and `call_host_from_bytecode` (not split, G 40-80). `new_array` (39) stays.
- Fall 110-160 Ir per call = 0.12-0.17% (G): it passes the line by 1.2-1.7x. A/B x1.0014-1.0020 (D on G, k 0.495). About 4 agent-h (G).
- At its registered low (x1.000) it adds 0 to a set (LM). It lifts a set only with a priced term set before its gate data; at x1.002, S1 x1.002 = x1.0167 (D).

### 4.3 MEGA-LEAN: a new store fact

- NAMED-COUNT lines 2 and 4 price the load probes only; they read NOT MET, so MEGA-LEAN is not in the LEVER-21 list (M; DL M14-P0-NAMED-COUNT-READ).
- New fact (M: the NAMED-COUNT read file and ic-stats): the store probe at `stub_cache.rs:205` in `SetNamedProperty` has D1mr 9,355,182 on 11,353,077 probes = 0.82 per probe (D1mr / Dr = 0.41 of the store reads).
- The same price form gives store 111 M + load 139 M cycles = x1.012 (D).
- Limit: `SetNamedProperty` is 7.61% of cycles and 9.01% of instructions (TD4). So most misses are hidden, and 9.5 cycles per miss is a top bound (G).
- E: V8 has separate load, store and define stub-cache tables, each with 4,096 + 1,024 entries (`v8/v8@932ddd7ca4` `src/execution/isolate.h:1380-1382`; `src/ic/stub-cache.h:90-93`).

### 4.4 The rank by est. gain per agent-hour (the A/B values and the hours are G)

| # | Cut | Evidence | A/B low / center / high | agent-h |
|---|---|---|---|---|
| 1 | S2: the set + the SWEEP-LEAN x86 re-read (fix the worker count) | x1.014675 x 1.0005 = x1.0152 (D) | x1.015 | 6-8 |
| 2 | TO-BOOL: a `fast_truthy` arm; a cell that is not a string or a BigInt gives true | `slow::to_boolean` 9,972,499 calls, 24 Ir each (M, cg); TD4 1.06% of cycles, 0.49% of instructions | x1.004 / 1.006 / 1.009 (the header miss may stay) | 3-4 |
| 3 | CHARCODE: a `charCodeAt` arm in `CallProperty1_cold` for a flat string and an int32 index | 13,307,418 calls (D: `string.rs:606` count / 7); `call_property::<1>` 137 + `char_code_at` 67 Ir per call (M, cg); TD4 1.39 + 0.63 + 0.68% | x1.010 / 1.015 / 1.019, both builds | 8-12 |
| 4 | MEGA-LEAN load + store | §4.3 | x1.004 / 1.008 / 1.012 | 10-14 |
| 5 | LIST-FIT (a) | §4.2 | x1.001 / 1.0015 / 1.002 | 4 |
| 6 | JIT-LEAN variant | §4.1 | jit x1.0018 | 5 |

- Rows 1, 2 and 3 together give about x1.037 at the center (D on G: 1.015182 x 1.006 x 1.015, as if independent).
- E: V8 runs `charCodeAt` as a Torque JavaScript builtin: embedded code with no C++ entry, also in jitless mode (`v8/v8@932ddd7ca4` `src/builtins/builtins-string.tq:177-190`). JSC marks it `CharCodeAtIntrinsic` and maps it to `charCodeAtThunkGenerator` (`WebKit/WebKit@f30433b25d` `Source/JavaScriptCore/runtime/StringPrototype.cpp:147`; `runtime/VM.cpp:785-787`).
- E: V8 `JumpIfToBooleanTrue` puts `BranchIfToBooleanIsTrue` in line: one map load and 3 bit tests, no call (`src/interpreter/interpreter-generator.cc:2222-2231`; `src/codegen/code-stub-assembler.cc:1895-1908`). JSC LLInt `jtrue` / `jfalse` read `m_type` and `m_flags` in line (`Source/JavaScriptCore/llint/LowLevelInterpreter64.asm:2170-2190`).

---

## 5. Calendar lens: the month-15 calendar (LC)

### 5.1 The order at the month-15 base (D from DL rows)

```
 2026-11-26/27  month-14 end row (branch N, one DL row); FUZZ-H (1 box, about 1.08 box-h; counted false)
 by 12-03 04:49 the modes check w61 (as w60a on the m2/m14 head: no m14 tag)      V19-D16
 month-15 base  A1H-JOIN (READY line 2026-12-07) -> SIMD-DISPATCH (7411b488, no gain claim)   V19-D13
 2026-12-08     DEPS-BUMP-3 dispatch, "not before"; merge at the first boundary on or after it  V19-D12, MONTH15 (6)
                no lever between its box start and its verdict row; no bytecode lever open
 then           the first Path B after the bump is a switch read (the oxc pin and patch dir on the switch list)
 2026-12-22, 2027-01-05  the next bump dates
```

### 5.2 DEPS-BUMP-3 (M: DL M14-PRE-MONTH15 line (6); V19-D12; P19 §9)

- Dispatch on 2026-12-08, "not before" (the 14-day user rule), from the newest pushed head of the plan month that is open on that date.
- Merge at the first phase boundary on or after 2026-12-08, in whatever plan month is open, with no lever between its box start and its verdict row and no bytecode lever open. With no open month on that date: the first base merge of the next month, before any box. A lever A/B read and an end Path B never straddle it. A1H-JOIN merges first.
- Month 14 ends about 2026-11-27/28 (G), so the bump merges in month 15 (D).
- The DEPS pin-fill G8 row (0 / +1) lapses in month 14 and is held in each later month cap until the bump merges (M; DL M14-PRE-G8).
- The oxc 0.160.0 scout (branch `m2m14/oxc-0160-scout`, stop lines S1-S9, never merged) runs before 2026-12-08 (M; OL M6-ORCH-1683; V19-D13).

### 5.3 A1H-JOIN and SIMD-DISPATCH

- A1H-JOIN never merges into the month-14 lever line. In branch N it merges at the month-15 base, before the bump. If it is not READY on 2026-12-07, one DL row flips the order (it names DL M12-P1-A1H-MOVE), and the join is written on oxc 0.160.0 after the bump (M; V19-D13). Its role and check started at 18:46 on `m2m14/a1h-join` (M; OL M6-ORCH-1683).
- SIMD-DISPATCH (`m2m14/simd` 7411b488) was not in a merged set, so it merges at the month-15 base with A1H-JOIN. It claims no gain, and its diff makes the next switch list non-empty (M; V19-D13).
- Its x86 box passed: 0.43 box-h; 11 PASS, 0 FAIL, 8 NOT ON THIS CPU, 0 NOT READ; sha-ni x7.386 over scalar; corpus digest f199320e (M; OL M6-ORCH-1677; DL M14-P0-SIMD-BOX-READ at f87b2936). The Mac armv8-sha2 body is x7.72 (M; OL M6-ORCH-1677).
- The CI gate name of this merge is not read here (G: the rule-76 side CI form).
- SWEEP-LEAN (8386386e) is not READY; any month-15 use needs a new row (M; OL M6-ORCH-1682).

### 5.4 The modes check (M: V19-D16; P19 §11)

- Due by 2026-12-03 04:49 +0800 (w60 + 7 days). With no m14 tag it runs on the `m2/m14` head as w60a and holds the same rows.
- It holds: the 7 version numbers, the corpus digest, the oxc pin with patch count and T-SITES, G8 spent / cap, SAFETY, one bytecode stream per mode; MC-w60-1 (PATCH-PREP, FUZZ-SMALL-STACK and GC-BUDGET merged; WIDE-FIX, LEVER-20 and A1H-JOIN not merged; G8 0 / +7); the JIT-DELTA table with the TD-4 pair (+1.13% instructions, +1.34% cycles, k 0.487); the 2 w60 cite fixes; the JIT-CACHE-SIM close; the V8 `UpdateFeedback` outside fact.
- The A1H-JOIN merge and the bump base merge each get their own check (P18 §11, as LC reads it).

### 5.5 The box budget and fuzz

- Month 14 used 0.43 + 0.78 + 0.78 = 1.99 of 15.9 box-h (D; OL M6-ORCH-1682).
- The cap is 15.9 outside an exit batch, with no raise and no carry-over (M; V19-D11). The 13.91 box-h left (D) do not move to month 15. Whether plan v20 keeps 15.9 is open (G).
- The hold rule: 2.10 + 1.08 + 3.41 = 6.59 box-h, or 7.71 after an ABI, opcode or REPRL merge. A wave starts only if spent + its high value + its retry + the hold is 15.9 or less (M; P19 §8.2).
- cargo-fuzz T2 runs at each m14 tag; with no tag none runs in month 14 (D; P19 §8.3). FUZZ-H is one box at the month end (campaign 1 always, about 1.08 box-h; campaign 2 only after an ABI, opcode or REPRL merge), counted false, its DL row first (M; V19-D22). The counted batch (about 131 box-h) is branch E only.

### 5.6 Month-14 process errors on record (M: OL rows)

| OL row | Error | Kind | Cost |
|---|---|---|---|
| M6-ORCH-1647 | 2 count worktrees made by hand with no oxc sync: GATE-FAIL | orchestrator setup | one re-push; the push script now runs `sync-oxc.sh` |
| M6-ORCH-1650 | the count brief named no commit form for count heads: adr-lint trailer and drift FAIL | brief gap | one re-make |
| M6-ORCH-1661 | a `git reset --soft` of an unpushed commit by an implementer (result clean) | rule break | none; named in the restart brief |
| M6-ORCH-1663 | the CTOR-FIT set term given in the instruction form (x1.0199), not the registered form (x1.0099) | orchestrator read | corrected before the cut |
| M6-ORCH-1678, -1682 | the SWEEP box design did not check the worker count | (b) design defect | 1.56 box-h, no x86 read; S2 not readable in the wait |
| M6-ORCH-1685 | the SIMD box count in OL M6-ORCH-1677 (6 PASS, 6 NOT ON THIS CPU) is wrong: 7 and 5 | orchestrator read | the DL row holds the true count |

---

## 6. The month-15 options

DL M14-PRE-MONTH15 line (5) reads: "all of (1)-(4) below, or no set merged ─► the status restates the option paper with K (keep the cell and go on) as the default; H and R are owner actions" (M; DL:6625). No set merged, no end Path B and no TD-5 ran, so lines (1)-(4) read below, and line (5) applies (M; OL M6-ORCH-1684). So plan v20 is a K plan. This record states H and R; it does not choose them.

### 6.1 K: the ranked set options (est.; the order is by est. gain per agent-hour and by readiness)

| Rank | Option | est. A/B (TS jitless model, k 0.495) | Box-h | agent-h | What it needs first |
|---|---|---|---|---|---|
| K-1 | **S2** = S1 + SWEEP-LEAN, after the x86 line reads PASS | x1.015182 (D); margin 0.018 pt = the SWEEP-LEAN low (G); MERGE 54.5% at tau 0.0051 (D on G) | C1 0 or C2 0.67-1.17, then the A/B about 3.98 (D) | 6-8 (G) | a v20 set row before the SWEEP data (the month-14 wait is spent); the SWEEP read row (C1 or C2); the S2 integrator (local commits f0dcdb7c..0028271a); a word on whether the month-14 gate reads stand on the month-15 base |
| K-2 | S2 + TO-BOOL as a member | x1.0213 at center (D on G) | the same A/B | +3-4 (G) | a design, a role, a gate line priced before its data |
| K-3 | S2 + CHARCODE as a member | x1.0304 at center (D on G); CHARCODE alone x1.010 / 1.015 / 1.019 | the same A/B | +8-12 (G) | a design, a role, a gate line priced before its data |
| K-4 | core + JUMP-INVERT | x1.0157 at the JUMP-INVERT low x1.001, x1.0167 at its center (D on G; JUMP-INVERT x1.001 / 1.002 / 1.006, R27 §1.6) | the A/B | not priced (no design) | a design; it changes bytecode, so it cannot be open at the DEPS-BUMP-3 boundary (D from MONTH15 line (6)); OL M6-ORCH-1685 names it a v20 option |
| K-5 | S1 + LIST-FIT priced | x1.0167 at x1.002 (D on G); 0 at its registered low | the A/B | 4 (G) | a role, a gate, 1 bench pair, a priced term set before its gate data |
| K-6 | MEGA-LEAN load + store | x1.004 / 1.008 / 1.012 (G) | about 0.4-0.5 for one cycle read (G) | 10-14 (G) | a v20 row with the store fact and new lines before data (rule 96); one box read of the probe block cycles, load and store split |
| K-7 | JIT-LEAN variant | jit x1.0018 (G) | the A/B | 5 (G) | it must not test non-nullish pairs (earley-boyer); the JIT-CELL clause is blocked by the TS jitless LEVER row (§2.4) |
| tool | CTOR-FIT x64 count (G and b on x64) | 0 (it fixes the source gap, not the 0.032 pt) | 0 (1 CI dispatch) | small (G) | a count head in the NAMED-COUNT form (`ep9_cli` + `ep9_vm/ic-stats` "store grow=", `ep9 --gc-stats` H0 "allocated_bytes="), never merged; a tools ref so that `gc-count.mjs` keeps the bytes (today it reads only collections, `:148`) |

- On the tool row: G and b are program events, so expect x64 = Mac (G). A moving CTOR-FIT term needs measured u (the TD-4 records). It is fair only if v20 re-prices every G input of both core terms (and k on one tree) by one rule, before the read, and accepts a fall (LM).
- A re-read of SHAPE-CACHE on the month-15 base can drop S2: the 0.018 pt margin equals 7.6 M of SHAPE-CACHE dIr (D).

### 6.2 The cases against the need (D on G; the m13-p1 low ends; the R27 planning t: 0.626 jit, 0.662 jitless; the jitless-cell model price is used for both builds, G)

A/B need at that t: jit x1.0278 / x1.0244, jitless x1.0107 / x1.0061 (D: exp(ln(1 / low end) / t)).

| Set at center | est. A/B | jit low ends | jitless low ends | All 4 >= 1.000? |
|---|---|---|---|---|
| S1 | x1.0147 | 0.992 / 0.994 | 1.003 / 1.006 | no |
| S2 (K-1) | x1.0152 | 0.992 / 0.994 | 1.003 / 1.006 | no |
| core + JUMP-INVERT center (K-4) | x1.0167 | 0.993 / 0.995 | 1.004 / 1.007 | no |
| S2 + TO-BOOL (K-2) | x1.0213 | 0.996 / 0.998 | 1.007 / 1.010 | no |
| S2 + CHARCODE (K-3) | x1.0304 | 1.002 / 1.004 | 1.013 / 1.016 | yes, by 0.2-0.4 pt on jit |
| S2 + TO-BOOL + CHARCODE | x1.0366 | 1.005 / 1.007 | 1.017 / 1.020 | yes |

- One set at the floor x1.015 does not fire the switch on the jit cells (D). The jit cells need about x1.024-1.028 in A/B terms.
- t is a planning number only; rule 101 read NOT RESOLVED in month 13 (R27 §1.1).

### 6.3 H and R: owner actions (stated, not chosen)

| Option | What it is | Who acts | Facts that bear on it |
|---|---|---|---|
| H | a bare-metal x86-64 gate host (performance governor, turbo off, SMT sibling off, isolated cores) | the owner buys or rents it | tau sets the A/B odds: S2 MERGE 82.2% at tau 0 and 54.5% at 0.0051 (§2.4, D on G); the cause of tau is not known, so a quiet host may lower it (G); R25 §4 gave the chance values (G); 0 Cloudflare box-h |
| R | adopt the registered option-R text (DL M11-PRE-OPTION-R): item 10 on linux-x64 reads the same-round bound t(jit) / t(jitless) <= 1.03 on both boxes, read only after the jitless cell passes item 3 | the owner says "adopt R"; the orchestrator writes 1 DL row | the m13-p1 end pair t(jitless) / t(jit) was 0.984 / 0.991 (DL M14-PRE-MONTH15 values), so t(jit) / t(jitless) = 1.016 / 1.009 (D), under 1.03; the jitless low ends need x1.0070 / x1.0040 (R27 §0); S2 at center gives jitless 1.003 / 1.006 (§6.2, D on G); the 2 open points of R25 §4 stay (does the bound read the jit half of item 3; median or interval); in month 12 the box-1 interval reached 1.033 (R25 §4) |

---

## 7. What plan v20 must register before any month-15 data (rule 96)

| # | Input | Why | Source |
|---|---|---|---|
| P1 | a new set row for the month-15 set (members, the set line model, the cut rule, the wait) | the month-14 wait is spent; DL M14-P1-ALLOC-VERDICT is the one NOT RUN row | §2, §6.1; DL M14-P1-ALLOC-CUT |
| P2 | whether the month-14 gate reads (CTOR-FIT, SHAPE-CACHE, SRC-HASH) stand on the month-15 base (after A1H-JOIN, SIMD-DISPATCH and maybe the bump), or are read again | a re-read can drop S2 (0.018 pt = 7.6 M dIr) | §6.1 |
| P3 | one price rule for every G input of the core terms (u, m, w, c; G and b by Mac or x64; k on one tree), and the form of the CTOR-FIT term (allocation or instruction) | the 2 forms differ 2x and the floor is inside the G band | §2.2, §2.3 |
| P4 | the SWEEP-LEAN x86 read row: C1 or C2, the worker-count check, the avx512-popcnt rule (one re-run; then C2 for that body), whether windows-x64 counts as x86 | the month-14 row is spent (NOT READ) | §3.2 |
| P5 | a priced term for each new member (LIST-FIT, TO-BOOL, CHARCODE, JUMP-INVERT) before its gate data | a rider at its low adds 0 | §4, §6.1 |
| P6 | the A/B form: tau 0.0051 kept or not; the guard cell count k (14 or the 8 F1 rows: an owner call, it drops earley-boyer and splay); rule 104 (a rate above 20% stays an open finding) | the false GUARD-FAIL of S2 is 39.9% at k 14 | §2.4, §2.5; V19-D9 |
| P7 | the MEGA-LEAN row with the store fact and new lines, and its cycle read, if v20 takes it | NAMED-COUNT priced the loads only | §4.3 |
| P8 | the DEPS-BUMP-3 date rule, the held G8 pin-fill row, the scout read (S1-S9), a boundary with no lever box or verdict open, the bump after A1H-JOIN, and the oxc pin and patch dir on the switch list | V19-D12, V19-D13 bind in month 15 | §5.2 |
| P9 | the JUMP-INVERT place (after the bump merge) if it is a member | no bytecode lever may be open at the bump boundary | §6.1 |
| P10 | the month-15 box cap and the hold rule | no carry-over; the 15.9 form is not given for month 15 | §5.5 |
| P11 | the month-15 switch row and the month-16 direction lines (the M14-PRE-SWITCH and M14-PRE-MONTH15 forms) | branch N repeats the form (G) | DL M14-PRE-SWITCH, M14-PRE-MONTH15 |

---

## 8. Open questions

| # | Question | Why it is open | Who |
|---|---|---|---|
| O-1 | Do the month-14 gate reads stand on the month-15 base? | no plan row says so | plan v20 (P2) |
| O-2 | Which CTOR-FIT price form is fair, and on which k? | no wall read anchors either form | plan v20 (P3) |
| O-3 | What causes tau 0.0051? | env size is already equal; the cause is not read (G) | a cause read or H |
| O-4 | Does windows-x64 count as x86 for the SWEEP line? | the row of month 14 names the x86 boxes only | plan v20 (P4) |
| O-5 | What Miri time do 0ab800db and 8386386e add? | `m2m14/*` pushes run no CI | the C1 run |
| O-6 | Can a JIT-LEAN variant reach 100 M without an earley-boyer cost? | the C2 class form is not designed | a design |
| O-7 | How many store-probe misses are hidden? | 7.61% of cycles for 9.01% of instructions; 9.5 cycles per miss is a top bound | a box cycle read |
| O-8 | Are the CHARCODE and TO-BOOL gains real? | new cuts, no design, G gains | designs and counts |
| O-9 | k 14 or 8 guard cells? | it drops named rows | the owner |
| O-10 | Does plan v20 keep the 15.9 box-h cap? | not given | plan v20 (P10) |
| O-11 | Is A1H-JOIN READY by 2026-12-07? | the role started at 18:46 | the role read |
| O-12 | H or R? | owner actions | the owner |

---

## 9. Verified, not verified, limits

### 9.1 Checked again by this writer (the 3 facts that carry the most weight)

1. **S1 = x1.014675, short by 0.032 pt (D, checked).** The terms and the product are in DL M14-P1-ALLOC-CUT at c5a4bc1c (DL:6647). A recount with `python3 -I` gives s 0.98064 pt, g 1.0099035, SHAPE-CACHE 1.0047249, S1 1.0146752, short 0.0325 pt, dIr need 212.87 M, and S2 1.0151825; u 90 and 180 give x1.01196 and x1.01809. All match LM and the DL row.
2. **The SWEEP cause chain (M, checked).** `PIN_RUN=0` (req.sh:31) and `taskset -pc $PIN_RUN $$` (req.sh:237) are the only pin in the SWEEP box script; the nproc check is at req.sh:221-222, before it. The SHARD command (build-cmd.txt:22) passes no `--threads`; `default_threads()` is `available_parallelism()` (`tools/test262/src/main.rs:80-82` at 8386386e). Both box summaries read `rc=124 TIME CAP (+46 min, load 1.05 1.02 1.00)` and `load 1.00 1.00 1.00`, 15 of 30 prefixes, 0 files. The Rust std 1.95.0 source reads `sched_getaffinity` and `CPU_COUNT` at `sys/thread/unix.rs:165-172`. CI test262-gc passes no `--threads` either (`ci.yml:629-631`).
3. **Line (5) and line (6) of DL M14-PRE-MONTH15 (M, checked).** The words of §6 and §5.2 are copied from DL:6625 at c5a4bc1c.

Also read again: the band B and tau(60) values (DL M14-PRE-AA-BAND-READ, M14-PRE-AB94); the store probe count (the NAMED-COUNT read file, line 13: D1mr 9,355,182, Dr 19,238,024); the odds table against the model lens output (S2 k 14: 82.2 / 76.2 / 54.5 / 28.3; family k 14 36.8%, k 8 22.8%); OL M6-ORCH-1682, -1684, -1685; the option-R text (DL M11-PRE-OPTION-R).

### 9.2 Not verified

- Every set gain and every case of §6.2 (no month-14 A/B box).
- The TO-BOOL, CHARCODE, MEGA-LEAN, LIST-FIT and JIT-LEAN variant gains and hours (G; the cg counts are LL reads).
- "1 worker processes" in the half logs (the lens rule did not read them).
- The avx512-popcnt support table (E, by the lens; not read again).
- The engine cites of §2.5 and §4 at the named commits (LM and LL read them; not read again here).
- The month-14 end date (G) and the A1H-JOIN READY date.
- The CI gate name of the SIMD-DISPATCH merge.

### 9.3 Limits

- One TD-4 box gives every cycle price (k 0.495, k 0.487, gcm, C).
- The odds model is the month-13 form with one tau; it has no wall null cell from month 14.
- t (0.626 / 0.662) is one month's ratio, not a rule value.
- One box type (Cloudflare standard-4, AMD EPYC 9684X); no gate host.
- The DL 24.5 rows (f87b2936) were written during this round and are not in its base.

## Sources

- DL rows at c5a4bc1c: M14-PLAN-D9, -D11, -D12, -D13, -D16, -D22; M14-PRE-AB94, M14-PRE-G8, M14-PRE-SWITCH, M14-PRE-MONTH15; M14-P1-ALLOC-GATE; M14-P1-CTOR-FIT-READ, M14-P1-JIT-LEAN-READ, M14-P0-NAMED-COUNT-READ, M14-P0-SPLAY-CAUSE-READ; M14-P1-SHAPE-CACHE-READ, M14-P1-ALLOC-CUT, M14-PRE-AA-BAND-READ, M14-P0-SIMD-BOX, M14-P1-SWEEP-BOX; M11-PRE-OPTION-R; M13-TD4-READ. DL rows at f87b2936 (`m2m14/prereg4`): M14-P0-SIMD-BOX-READ, M14-P1-SWEEP-BOX-READ, M14-P1-ALLOC-VERDICT, M14-P2-PICK.
- OL M6-ORCH-1640 to -1685.
- Plan v19 §4.2-§4.5, §8.2, §8.3, §9, §11 (orchestrator copy).
- The month-14 designs: the ALLOC-SET design and the JIT-LEAN design (orchestrator copies).
- The month-14 role checks (SWEEP-LEAN, line 23).
- The month-14 count dirs: shape, shape-gate, ctor, jitlean-gate (cg), sweep (checkall, gcstress), set, aa-band, named (n1 read file).
- The box summaries, box records and step logs of ep9-x86-m14-simd-1, ep9-x86-m14-sweep-1 and ep9-x86-m14-sweep-2; the SWEEP box script req.sh and build-cmd.txt; the TD-4 box script (the pin form).
- The rule-25 tables of the m12-p0, m12-p1 and m13-p1 tag CI runs.
- ep9 code: `tools/test262/src/main.rs`, `tools/test262/src/exec.rs`, `crates/ep9_platform/src/simd.rs` (8386386e); `.github/workflows/ci.yml` (c5a4bc1c); the ep9 source lines that the cg files name (`stub_cache.rs`, `profile.rs`, `bits.rs`, `dsl.rs`, `string.rs`; read by LL at the cg trees).
- [R25 §4](25-m2-month13-plan-research.md) (the option paper), [R26 §1.2, §6.3, §7](26-m2-month13-alloc-simd.md), [R27 §0, §1.1, §1.6, §1.7](27-m2-month14-plan-research.md).
- Outside: `v8/v8@932ddd7ca4` (`src/objects/lookup-cache.h`, `src/execution/isolate.h`, `src/ic/stub-cache.h`, `src/builtins/builtins-string.tq`, `src/interpreter/interpreter-generator.cc`, `src/codegen/code-stub-assembler.cc`); `WebKit/WebKit@f30433b25d` (`Source/JavaScriptCore/runtime/StringPrototype.cpp`, `runtime/VM.cpp`, `llint/LowLevelInterpreter64.asm`); the Rust std 1.95.0 source (`library/std/src/sys/thread/unix.rs`); the Intel and AMD AVX-512 support tables.
- This record has no artifacts dir: every number it uses is in this text.
