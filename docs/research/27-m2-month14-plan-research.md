# The month-14 plan research: the speed sets after the month-13 end read, the jit-only part, the month-13 process and box use, and the drift risk of each month-14 set

## Run context

| Item | Value |
|---|---|
| Dates | The round: 2026-11-26 07:53-08:01 +0800 (UL:530; OL M6-ORCH-1633 starts it, OL M6-ORCH-1634 closes it). The later data: the FUZZ-H read at 08:37 (OL M6-ORCH-1635). This record: 2026-11-26 |
| Round | The plan-v19 research: 3 read-only notes (speed, process, modes) and the plan-v19 inputs list (102 items + a routes table). This report collects them and reads each forecast against the later data. It runs no new box and no new measurement |
| What was known at the round | The month-13 end Path B read on `m13-p1` (OL M6-ORCH-1630), the TD-4 cycle read (OL M6-ORCH-1632), every month-13 verdict row. Not known: the FUZZ-H result, the exit-inputs run 45713632628, plan v19 |
| ep9 trees | `m13-p1` 0b710261 (the last month-13 tag; the end read tree); `m13-p0` 31537318; `m12-p1` fcac0863 (the month-12 end read tree). DL rows are at 67fdda4d (`m2m13/prereg-end`; 0b710261 is its ancestor, and `git diff --stat 0b710261 67fdda4d -- crates third_party tools Cargo.lock` is empty). Code cites (`crates/...`, `docs/plan/...`) are `git show 0b710261:<path>`. R26 is at bf76e194 (`m2m13/counts`; it lands on the month line with this record) |
| Boxes | No box in the round. The data it read: ep9-x86-m13-end-pathb-1 and -2 (Cloudflare standard-4, AMD EPYC 9684X, both builds, 54 min each = 1.80 box-h; OL M6-ORCH-1630), ep9-x86-m13-td4-1 (24 min = 0.40 box-h, 11 read rounds; OL M6-ORCH-1632; TD4:19). Later: ep9-fz-m13-h1 (64 min = 1.07 box-h; OL M6-ORCH-1635) |
| Upstream | `v8/v8@932ddd7ca4`, `WebKit/WebKit@f30433b25d`, `mozilla-firefox/firefox@ec287ddb04` (the clone heads of R25). Each engine cite in §1.4 and §3.1 was read again with the named commit for this report |
| Method | Read only. Every number has a cite. **M** = measured (a box, a CI run, a log, a source read). **D** = derived (arithmetic from cited numbers; the inputs are shown). **I** = inferred. **E** = estimate (basis named). **G** = guess. **O** = open. A forecast is never stated as a result |
| Limits | One end read (2 boxes) and one TD-4 box (11 rounds, medians only). One box type, no gate host. No month-14 box has run, so every month-14 set gain is open. Plan v19 was being written at the round (it was accepted on 2026-11-26 at 08:38 +0800; OL M6-ORCH-1637, -1638); this report cites no plan-v19 decision |

Short forms:
- DL = [docs/decisions-log.md](../decisions-log.md), by row id. OL = the orchestrator log, by row id M6-ORCH-n (not in the repo). UL:n = row n of the usage log (not in the repo).
- R19S, R19P, R19M = the plan-v19 research notes on speed, process, and modes (not in the repo). IN19 = the plan-v19 inputs list (#n = item n; not in the repo).
- PBe12, PBe13 = the Path B reads at the month-12 end on `m12-p1` (DL M12-END-PATHB) and the month-13 end on `m13-p1`; PBe13:n = line n of its compare file (not in the repo; its numbers are in this report and its artifacts).
- TD4:n = line n of the TD-4 summary (one box on `m13-p1`; the lines of DL M13-PRE-TD4; not in the repo).
- R25 = [25-m2-month13-plan-research.md](25-m2-month13-plan-research.md); R26 = [26-m2-month13-alloc-simd.md](26-m2-month13-alloc-simd.md); JDR, JDT = its artifacts [jit-delta-read.md](artifacts/m2-month13-alloc-simd/jit-delta-read.md) and [jit-delta-table.txt](artifacts/m2-month13-alloc-simd/jit-delta-table.txt).
- Plan v18 is not public. Its decisions are DL rows M13-PLAN-Dn (= V18-Dn); other plan-v18 text is cited as "plan v18 §n".
- Ratios: an A/B gain x1.0nn = t(base) / t(lever). A gate ratio is t(node --jitless) / t(ep9) (above 1 = ep9 is faster). "Need" = 1 / the interval low end. "Points" = 100 x a ratio difference. t = the share of an A/B gain that shows in the Path B (rule 101).

---

## 0. Verdict

```
 month-13 end read on m13-p1 (PBe13:388, :392)     jit low ends 0.983 / 0.985        jitless low ends 0.993 / 0.996
 need at the low ends (D)                          x1.0173 / x1.0152                 x1.0070 / x1.0040
 month-12 end need (R25 §1.1)                      x1.041 / x1.042                   x1.022 / x1.019
 switch EXIT-SWITCH-M13 (PBe13:395)                DOES NOT FIRE: branch N; month 14 = the pre-registered fallback exit month

 the round's answers (forecasts, not results)
 Q1  GC-2 at TD-4 line G 2.59%            top x1.0085, center about x1.002        not the first set (R19S §1)
 Q2  named load vs node --jitless         +0.380 G cycles = 1.8% of ep9 cycles    probe block 0.548 G, about 19 cycles / probe (R19S §2)
 Q3  the jit-only gap                     TD-4: +1.13% instructions, +1.34% cycles; k = 0.487 cycles per Ir (R19M §1.2)
 Q4  order                                CTOR-FIT -> RECORD-LEAN (jit only) -> NAMED-SET after a 0-box count (R19S §4.4)
     2 sets at center                     jit 0.993 / 0.995: no fire               4 sets at center: all 4 low ends 1.002-1.013 (D on E)
 process                                  month 13 = 15 h 52 min of wall; box lane busy 3.7 h; 3 idle gaps = 12.2 h (R19P §0)
```

1. **The binding cell is still the jit build, and the need fell by more than half (M, D).** Need jit x1.0173 / x1.0152, jitless x1.0070 / x1.0040 (PBe13:388, :392). On `m12-p1` the need was jit x1.041 / x1.042 (R25 §1.1). One lever merged: GC-BUDGET (DL M13-P1-GC-MERGE).
2. **GC is no longer a large gap (M).** The main-thread GC share fell from 4.43% (TD-3) to 2.59% (TD4:247), under the 4% price line of GC-2 (TD4:407). The GC helpers now save no wall: h0 / default 1.002 [0.985-1.012] (TD4:29). GC-2 has a top of x1.0085 (§1.3).
3. **The jit-only gap is instruction count plus branch misses (D).** On TD-4 the jit build runs +0.579 G instructions and +0.282 G cycles (TD4:20-21): k = 0.487 cycles per instruction, the high k of the JIT-DELTA model (0.4829, JDT:4). L1i misses are lower in the jit build. So route JIT-CACHE-SIM has its answer with 0 boxes (§1.5).
4. **The named-load gap to node --jitless is at the NL-1 reopen line (D).** +0.380 G cycles against the line 0.37 G, on one box (§1.4). The stub-cache probe block costs about 19 cycles per probe; one multiply costs 3. The ep9 table (192 KiB) is larger than V8's tables and SpiderMonkey's table (§1.4).
5. **No single month-14 set closes the jit cells (D on E).** CTOR-FIT + RECORD-LEAN at center leave jit at 0.993 / 0.995. 4 sets at center reach 1.002-1.004 on jit (§1.7). The round guessed the month-14 exit chance at about 10% (G); the plan-v18 line "no month-14 estimate until JIT-DELTA prices a jit lever" (DL M13-PRE-MONTH14) still holds at this commit.
6. **Month 13 took 15 h 52 min, not 3-6 days, and the box lane was idle 77% of it (M, D).** 6 of the 12.2 idle hours were CI waits on the box path. 6 push refusals came from briefs or orchestrator steps; 0 came from code defects (§2.1-§2.3).
7. **Month 13 used 7.68 of 15.9 box-h and 18.13 M tokens on its work rows (M).** The plan said 8.33-10.30 box-h and 30-42 M tokens (§2.3, §2.4). WIDE-FIX (no merge) took 4.84 M tokens and 2.29 box-h.
8. **The jit build must keep its records (M, source).** M2 item 10 reads gates 1-3 "with all H16/H29 profiling on" (`docs/plan/09-milestones.md:113`). So the only jit-only cut is a cheaper record form with equal facts: JIT-LEAN (§3.2).

---

## 1. Speed findings

### 1.1 The need, before and after the month-13 end read

| Read | jit low ends (box 1 / box 2) | jitless low ends | Need jit | Need jitless | Binding | Source |
|---|---|---|---|---|---|---|
| PBe12 (`m12-p1`) | 0.961 / 0.960 | 0.978 / 0.981 | x1.041 / x1.042 | x1.022 / x1.019 | jit, both boxes | R25 §1.1 |
| PBe13 (`m13-p1`) | 0.983 / 0.985 | 0.993 / 0.996 | x1.0173 / x1.0152 | x1.0070 / x1.0040 | jit, both boxes | PBe13:388, :392 (D: 1 / low end) |

- The medians on PBe13: jit 0.986 / 0.993, jitless 1.003 / 1.000 (PBe13:29, :85). The jitless cells straddle on both boxes; the read-to-read move of the same code is about 1 point (R25 §1.7), so a box draw alone can pass or fail them (I).
- Rule 100: NO FALL. The TS medians moved jit +1.83% / +1.41%, jitless +1.80% / +1.14% against PBe12 (PBe13:397-401).
- Rule 101: NOT RESOLVED in both builds (ln x < 0.030). jit: A/B product x1.0260, Path B y1.0162; jitless: x1.0223, y1.0147 (PBe13:405-406). The ratio ln y / ln x is 0.626 (jit) and 0.662 (jitless) (D). This report uses it only as a planning number, never as a rule value.
- Items 1, 2, 4 and 6 AGREE in both builds (PBe13:386-393, :148-151). Item 6 (splay) fell on all 4 cells against PBe12, and by more than 2% on 3 of 4: jit 1.159 -> 1.115 (-3.8%), 1.171 -> 1.128 (-3.7%); jitless 1.171 -> 1.121 (-4.3%), 1.141 -> 1.132 (-0.8%) (D; PBe12:30, :86; PBe13:30, :86). The cause is not read (IN19 #4).

### 1.2 The month-13 sets as read, and the R25 forecasts they settle

| Set | R25 forecast (jitless A/B low / center / high) | Read | Word | Source |
|---|---|---|---|---|
| GC-BUDGET (arm B1) | x1.007 / x1.012 / x1.017; MERGE odds 73-86% | decision cell 0.9782 [0.9711-0.9865] = x1.0223; jit cell 0.9747 = x1.0260 | MERGE B1, merged; above the high case | DL M13-P1-GC-VERDICT, M13-P1-GC-MERGE |
| WIDE-FIX (shape F-a) | x1.013 / x1.017 / x1.023; MERGE odds 57-73% | decision cell 0.9930 [0.9783-1.0012] = x1.0070; box 1 0.9842, box 2 1.0009 | ONE MORE BOX; box 3 not bought; R7 closes WIDE-SET, W-1, W-2 for M2 | DL M13-P2-WIDE-FIX-VERDICT |
| LEVER-20 (the pick) | expected NOT RUN | no candidate at the floor x1.015 | NOT RUN | DL M13-P3-PICK |
| Path B transfer t | 0.6-0.8 (R25 §1.3) | ln y / ln x 0.626 / 0.662 | NOT RESOLVED by rule 101 | PBe13:405-406 |

- GC-BUDGET cut the TS GC count from 12 to 6 and raised the TS peak RSS: jit 1.037, jitless 1.130 (DL M13-P1-GC-VERDICT). The G7 caps still hold: 0.798 / 0.800 against node (cap 1.2) and 1.279 / 1.280 against d8 (cap 1.8) (PBe13:148-151).
- The merge-head clause "bench Ir within 1e-5" read FAIL on 3 of 40 rows. 2 rows equal the A/A noise of the same code (1.80e-5, 1.12e-5); the third is a sweeper-thread race at the 12 MiB knee. The line stays as written; the call was MERGE (DL M13-P1-GC-MERGE-CALL). Same-sha A/A Ir moves 1.49e-5 / 1.76e-5 (IN19 #37). This is route AA-NOISE-1E5.

### 1.3 GC-2 at a 2.59% main-thread GC share (R19S §1)

| Fact | TD-3 (`m11-p3` code) | TD-4 (`m13-p1`) | Source |
|---|---|---|---|
| main-thread GC share | 4.43% | **2.59%** [2.56-2.77] | R25 §1.2; TD4:247 |
| all-thread GC share | 6.81% | 4.13% | R19S §1.2; TD4:247 |
| h0 (no helper) GC share | 7.27% | 3.83% | R19S §1.2; TD4:249 |
| wall h0 / default (> 1: the helpers save wall) | x1.022 | **1.002** [0.985-1.012] | R19S §1.2; TD4:29 |
| `sweep_page_with`, all threads | 1.19% | 0.84% = 0.176 G | R19S §1.2; TD4:91 |
| `ep9-gc-sweeper` thread | 0.66% | 0.46% | R19S §1.2; TD4:236 |
| GC count per TS run | 11-12 | 6 | TD4:280-283 |

```
 item 2 (sweep share)    the main thread sweeps 0.84 - 0.46 = 0.38 points or more, at most 0.84 (D)
                         all of it off the main thread: x1.0038 (main part) .. x1.0085 (every sweep row) = the top
 item 3 (helper policy)  the helpers save no measured wall (1.002 straddles): no base; <= x1.003 (G)
 GC-2 (items 2 + 3)      low x1.000 / center x1.002 / high x1.005; top x1.0085 (R19S §1.3)
```

- **Not the first set (R19S-D1).** Its center is under every other set of §1.6 and under the x1.015 pick floor (DL M13-P3-PICK). Line (1) of DL M13-PRE-MONTH14 read ABOVE ("GC-2 is the first month-14 set"; PBe13:412). DL M13-PRE-TD4 says that a plan that keeps GC-2 first must name the G FAIL; DL M13-PRE-MONTH14 says that a plan that takes another direction must name the line it overrides and the new fact. The new fact is TD4:407 (2.59% < 4%) (IN19 #6).
- **What GC work is left (I).** Node's main-thread GC share is not read: the node thread names put every sample off the main thread (TD4:239). If it is near 1.3% (R24's inference), every GC lever together gives at most about x1.013.

### 1.4 The named load: the gap to node --jitless and the engine forms (R19S §2)

| Measure | ep9 (e-jitless) | node --jitless | Gap | Source |
|---|---|---|---|---|
| cycles in the named-load handlers | 5.131 + 2.071 = 7.202 G (34.3%) | 6.058 + 0.764 = 6.822 G (32.0%) | **+0.380 G = 1.8% of ep9 cycles** (D) | TD4:71-72, :170, :174 |
| the same on TD-3 | 8.109 G | 7.865 G | +0.244 G | R19S §2.2 |
| backend-stall slots (line N) | 12.064 G | 8.088 G | +3.976 G slots = 0.663 G cycle units (/6) = 3.2% (D) | TD4:252, :409 |
| stub-cache probe block (line M) | 2.611% = 0.548 G | in `LoadIC_Noninlined` | - | TD4:270-272, :410 |
| per megamorphic probe | 0.548 G / 29.16 M = **18.8 cycles** (D) | - | - | the 29.16 M count is an old Mac count (`crates/ep9_vm/src/stub_cache.rs:28`) |

- The +0.380 G is the like-for-like gap of the NL-1 reopen fact (c) (line 0.37 G; R25 §1.4). It is at the line on one box. R19S-D5: record it as "at the line on one box"; no reopen on it alone.
- The TD-4 box has no precise sampling (TD4:35). So 18.8 cycles is the cost of the probe path with the loads before it, not of the multiply alone (I).

| Engine | Monomorphic path | Megamorphic path | Cite (verified at the named commit) |
|---|---|---|---|
| V8 Ignition | `GetNamedProperty` calls `LoadIC_BytecodeHandler` inline | primary table 4,096 entries + secondary 1,024; hash = the name's stored hash field + (map xor map >> 12), masked: no multiply; tables cleared only at a full mark-compact | `v8/v8@932ddd7ca4` `src/interpreter/interpreter-generator.cc:610-632`; `src/ic/stub-cache.h:90-93`; `src/ic/stub-cache.cc:28-39`; `src/heap/heap.cc:2363-2365` |
| JSC LLInt | the metadata table base is a pinned callee-saved register; then a structureID compare and the cached-offset load | no megamorphic cache in the LLInt get_by_id | `WebKit/WebKit@f30433b25d` `Source/JavaScriptCore/llint/LowLevelInterpreter.asm:350-358`; `LowLevelInterpreter64.asm:1640-1651` |
| SpiderMonkey | CacheIR stubs | `MegamorphicCache`: 1,024 entries; hash = (shape >> shift 1) + key hash, xor (shape >> shift 2): no multiply; a generation bump in place of a table walk | `mozilla-firefox/firefox@ec287ddb04` `js/src/vm/Caches.h:182-222` |
| ep9 (`m13-p1`) | 4 dependent loads (frame -> FunctionInfo -> unit root slot -> cell), then the ShapeId compare | rotate, xor, **multiply by 0x9E3779B1**, shift; 8,192 entries = **192 KiB**; flushed at every GC | `crates/ep9_vm/src/stub_cache.rs:41`, `:75-82`, `:148-151` |

Lessons (I): V8 and SpiderMonkey hash with add, xor and shift. That is MH. But their tables are smaller: V8 96 + 24 KiB (D: 4,096 and 1,024 entries x 24 bytes; the box node has `v8_enable_pointer_compression=0`, TD4:389), SpiderMonkey 1,024 entries. A 192 KiB table does not fit a 32 KiB L1d (E, Zen 4). An L2 hit of about 14 cycles (E) fits the 18.8 cycles better than the multiply does (not measured). ep9 went from 1,024 to 8,192 entries because most misses were conflicts (`stub_cache.rs:75-79`: load misses 363,397 -> 69,354, Mac).

| Item | Bound or estimate | Basis |
|---|---|---|
| all named-load cycles at node's level | <= x1.018 (D) | the +0.380 G gap |
| MH (no multiply) | x1.001 / x1.002 / x1.004 (D) | 1-3 cycles x 29.16 M |
| MEGA-LEAN (MH + a table that fits L1d) | x1.004 / x1.008 / x1.013 (G) | half of the block, if the L2 hit is the cost; needs a cache count |
| NL-1 stage 1 | x1.000 / x1.005 / x1.012 (E) | R25 §1.4; break-even now 0.310 G |
| SHAPE-LOOKUP-CACHE | x1.006 / x1.007 / x1.008; bound x1.010 (E) | R26 §7; `lookup_with` 0.90% (TD4:88) |

### 1.5 The jit-only gap: what TD-4 says (R19M §1.2; R19S finding 1)

| Measure (TS, `m13-p1`, 11 rounds) | e-jit | e-jitless | jit - jitless | Source |
|---|---|---|---|---|
| cycles | 21.267 G | 20.985 G | +0.282 G (+1.34%) | TD4:20-21 |
| instructions | 51.801 G | 51.222 G | +0.579 G (+1.13%) | TD4:20-21 |
| branch misses (D: per 1k x instructions) | 79.46 M | 76.63 M | +2.83 M | TD4:20-21 |
| L1i misses (D) | 11.97 M | 13.83 M | -1.86 M | TD4:20-21 |
| wall e-jitless / e-jit | | | 0.980 [0.965-1.006] | TD4:28 |
| JIT-DELTA cachegrind Ir (TD-3 era tree) | 52.1304 G | 51.5179 G | +612.49 M (+1.189%) | JDT:3 |

- k = 0.282 / 0.579 = **0.487** cycles per jit-only instruction (D). JIT-DELTA priced at the center k 0.2414; its high k is 0.4829 (JDT:4). So the high column of the JIT-DELTA table is the measured column (I, one box).
- The cachegrind delta (+1.189%) and the perf delta (+1.13%) agree within 0.06 points. The cycle delta is instructions at the jitless IPC (0.579 / 2.441 = 0.237 G) plus the branch misses (0.04-0.06 G) (R19S finding 1). No cache term is left. Route JIT-CACHE-SIM closes on this ledger (R19S-D6, R19M §4 C5).
- At TD-4 C = 21.267 G and k 0.487 (D): JDR ideas 1 + 2 (compare record + keyed cold record, 505.0 M Ir) bound x1.012; idea 1 x1.007; idea 2 x1.005; all net jit-only Ir x1.014. A real lean form keeps a no-change test (the V8 form, §3.1): est. x1.008-1.010 (R19M, E). R19S wrote "bound x1.013 at k = 1 / IPC"; at 1 / IPC the recount gives x1.012 and at the measured k x1.014 (D).

### 1.6 The month-14 candidate sets (every gain is an estimate)

| Set | Class | The hot loop it targets (measured) | A/B low / center / high | G8 | Source |
|---|---|---|---|---|---|
| **CTOR-FIT** (L1, slack 1) | allocation + named load | store grow 1,467,508 -> 74,101; bytes -9.79%; x64 TS Ir -1.71% jit / -1.74% jitless | x1.005 / x1.010 / x1.018 (R26); R19S reads the center as x1.012 (E: the Ir fall is 1.9 x the design center) | +1 likely (H17) | R26 §6; OL M6-ORCH-1567, -1575 (run 45623497089) |
| **RECORD-LEAN** = JIT-LEAN (jit only) | instruction | `record_pair_next` 129.8 M Ir (about 51 Ir per call); keyed cold record 219.5 M | jit x1.003 / x1.006 / x1.010; bound §1.5; jitless IDENTITY (I) | 0 (I) | JDR; R19S §3.2; R19M §2.2 |
| SHAPE-LOOKUP-CACHE | instruction | `lookup_with` 6.15 M calls, 76% miss; a 1,024-entry cache hits 90.3% | x1.006 / x1.007 / x1.008 | 0 (not checked) | R26 §7 |
| NAMED-SET (MH + table form + NL-1 stage 1) | instruction (memory latency) | probe block 0.548 G | x1.004 / x1.008 / x1.015 (G; overlap with CTOR-FIT not priced) | 0 / +1 | §1.4 |
| SIMD riders: the `ep9_platform::simd` dispatch module + L6 SRC-HASH; L3 sweep bitmap | SIMD | sha256 0.37%; sweep 0.84% | x1.000 / x1.002 / x1.003 and x1.0005 / x1.0015 / x1.0035 | 0 / +1 (L6 lazy form) | R26 §1.1, §2 |
| L4 (a) LIST-FIT | allocation | 538,940 `new Array()` host calls | x1.000 / x1.002 / x1.005 (G) | 0 | R26 §6.3 |
| GC-2 | GC | sweep 0.38-0.84 points on the main thread | x1.000 / x1.002 / x1.005; top x1.0085 | 0 | §1.3 |
| JUMP-INVERT | instruction (bytecode) | 417 sites, 18.0 M runs | x1.001 / x1.002 / x1.006 | 0 (I) | R25 §1.4; it opens now that R7 closed W-1 |

- **SIMD is a rider, not a gate lever (M).** No measured loop of 0.61% or more outside GC and allocation can take SIMD; all SIMD rows together are x1.003-1.0045 (R26 §2). This is the owner direction "And maybe more SIMD" (OL M6-ORCH-1551) on the measured loops; the other owner direction puts allocation cuts beside instruction cuts (OL M6-ORCH-1550).
- **RECORD-LEAN uses the JIT-CELL clause** (DL M13-PLAN-D5): it changes only the `full` handler table, so the jitless cell is IDENTITY and the jit TS cell is the decision cell. The clause has not been used yet (R19M Q2).
- **JUMP-INVERT changes bytecode.** Line (5) of DL M13-PRE-MONTH14 lets DEPS-BUMP-3 merge only at a boundary with no bytecode lever open (I: so it fits after the bump).

R19S ranking (R19S §4.4): 1 CTOR-FIT (with SHAPE-LOOKUP-CACHE as a second arm and the SIMD riders), 2 RECORD-LEAN (ideas 1 + 2; idea 3 needs a MODES row), 3 NAMED-SET only if a 0-box count shows D1 misses on 50% or more of the probes, 4 GC-2, 5 JUMP-INVERT, 6 L4 and L3 as riders. Box room: cap 15.9 - hold 6.59 = 9.31 box-h = 4 two-box reads with no retry (D; DL M13-PLAN-D10).

### 1.7 The cases against the need (R19S §4.2-§4.3; D on E; centers combined as if independent)

| Cell | low end | A/B need at t = ln y / ln x | A/B need at t = 0.8 |
|---|---|---|---|
| jit box 1 / box 2 | 0.983 / 0.985 | t 0.626: x1.028 / x1.024 | x1.022 / x1.019 |
| jitless box 1 / box 2 | 0.993 / 0.996 | t 0.662: x1.011 / x1.006 | x1.009 / x1.005 |

| Merged sets (centers) | jit A/B | jitless A/B | jit low ends (t 0.626) | jitless low ends (t 0.662) | Fires? |
|---|---|---|---|---|---|
| CTOR-FIT + RECORD-LEAN | x1.016 | x1.010 | 0.993 / 0.995 | 1.000 / 1.003 | no (jit 0.5-0.7 points short) |
| + SHAPE-LOOKUP-CACHE + NAMED-SET | x1.031 | x1.025 | 1.002 / 1.004 | 1.0095 / 1.0125 | yes, with 0.2-0.4 points on jit |
| the same 4 at t 0.8 | | | 1.008 / 1.010 | 1.013 / 1.016 | yes |

- The mid Path B line of DL M13-PRE-SWITCH (each last low end x (A/B product)^0.8 >= 1.000) needs jit x1.0218 / x1.0191 and jitless x1.0088 / x1.0050 (IN19 #9; D).
- If the jit build ran the jitless instructions, the jit cells would read about the jitless cells: the build gap at the cell is 1.003 / 0.986 = 1.0172 (box 1) and 1.000 / 0.993 = 1.0070 (box 2) (D; R19M §1.2). JIT-LEAN removes about 0.6-1.0 point of it (R19M §2.1, E). The rest must come from both-build sets.

### 1.8 The exit chances as written (every value is a guess)

| Source, time | Month 14 | Basis named by the source |
|---|---|---|
| DL M13-PLAN-D1, DL M13-PRE-MONTH14 (2026-11-25) | no estimate "until the JIT-DELTA count prices a jit lever" | both jit cells 1.8-1.9 points short at the high case |
| R19S §4.4 (2026-11-26 08:01) | about 10% | 3-4 merges at center or better (month 13 merged 1 of 3), or 2 merges and a box draw of +0.5-0.7 points on both jit cells |
| R19M §2 | no value | no single set closes the jit cells |

R19S asks the plan writer to decide if JIT-LEAN at the TD-4 price is "the priced jit lever" of the line. At this commit no plan row says so; the no-estimate line holds.

### 1.9 Forecast -> later data

| # | Forecast (source) | Later data | Status |
|---|---|---|---|
| FC1 | FUZZ-H costs about 1.08 box-h (R19P §2.1; IN19 #81; E) | 64 min = 1.07 box-h (OL M6-ORCH-1635; UL:531) | confirmed |
| FC2 | month 13 uses 6.61 + 1.08 = 7.69 box-h (R19P §0; E) | 7.68 of 15.9 (OL M6-ORCH-1635) | confirmed |
| FC3 | the FUZZ-H result: not known (IN19 #61; R19S §7) | PASS, 0 crashes, 4.0067 CPU-h, coverage 4.24% -> 9.87% (OL M6-ORCH-1635; the FUZZ-H ledger row) | done (not a forecast) |
| FC4 | items 5, 7, 9 wait for the exit-inputs run 45713632628 (IN19 #13, #56) | not read at this commit | open |
| FC5 | month 14 ends about 2026-11-27 at the month-13 pace, before 2026-12-08 (R19P §4.2; IN19 #100; I) | no month-14 phase has run | open |
| FC6 | month-14 exit chance about 10% (R19S §4.4; G) | no month-14 box | open (a guess) |
| FC7 | 2 sets at center do not fire; 4 sets at center fire (R19S §4.3; D on E) | no month-14 box | open |
| FC8 | CTOR-FIT x1.005 / x1.010 / x1.018; RECORD-LEAN jit x1.003 / x1.006 / x1.010 (R19S §3.2; E) | no month-14 box | open |
| FC9 | JIT-LEAN removes 0.6-1.0 point of the jit extra need (R19M §2.1; E) | no design, no box | open |
| FC10 | the jitless cells can pass or fail by a box draw alone (R19S §4.1; R19P §4.1; I) | no month-14 read | open |

Rows are in [forecast-later.tsv](artifacts/m2-month14-plan-research/forecast-later.tsv).

---

## 2. Process findings (R19P)

### 2.1 The month-13 time line: planned against real

| Set | Plan v18 | Real | Source |
|---|---|---|---|
| the month | 3-6 days; lever line ends 2026-11-28 to 12-01 (plan v18 §3) | **15 h 52 min**: 2026-11-25 16:01 -> TD-4 read 07:53 (D) | OL M6-ORCH-1547, -1632 |
| GC-BUDGET | 0.5 + 0.5 agent-days; boxes 2026-11-26 | part 1 16:03-16:43; boxes 21:52-22:59; MERGE B1 read 23:02; tag `m13-p1` about 06:27 | UL:474, :497-498; OL M6-ORCH-1580, -1627 |
| GC read -> tag | not planned | 7.4 h: 3 second-model review rounds, 2 merge runs (run 1 stopped on a grant gap), the merge-head 1e-5 FAIL and the call, a missed Claude lens, 2 full CI runs | OL M6-ORCH-1581..-1608, -1617..-1627 |
| WIDE-FIX | 1.0-1.5 agent-days; G5 time box 1.5 days | G5 = 9 h 42 min; boxes 01:59-03:13; ONE MORE BOX | UL:476-516; DL M13-P2-WIDE-FIX-VERDICT |
| PATCH-PREP | 1.0-1.5 agent-days | role 67 min (16:44-17:51) | UL:477 |
| tag `m13-p0` | the early tag at the PATCH-PREP merge (DL M13-PLAN-D7) | integration ready 20:05; tag 04:14 = 8.2 h later (a second full CI on a DL-only head, a lost macOS runner) | OL M6-ORCH-1563..-1616 |
| A1H-JOIN, the 0.160.0 scout | M13-P1 ride role; a scout before 2026-12-08 (DL M13-PLAN-D8, -D17) | not started: no OL row of M6-ORCH-1547..-1633 names either (I) | IN19 #20, #51, #52 |

R19P reads the agent-day estimates as 5-10 x too high, and proposes that plan v19 states effort in wall hours from measured units (R19P C7; I).

### 2.2 The box lane (D from OL times)

```
 +0800   16:01           21:52  22:59          01:59  03:13          06:29  07:23 07:51
 BOX     |---- idle 5.9h ---|GC 1.1|-- idle 3.0h --|WIDE 1.2|-- idle 3.3h --|PB 0.9|TD4|FUZZ-H 07:30-08:35
 cause    count run +        FILL       SIDECI after  3 FILL    2 full CI runs on one sha
          FILL chain         re-makes   a pre-box     stops +   (1.7 h each, serial)
          (2 GATE-FAIL)      + watcher  fix round     1 GATE-   + fuzz T2 2.2 h
                             bug ~1 h                 FAIL
```

| Gap | Hours | Where the time went (box path only) | Source |
|---|---|---|---|
| 1 | 5.9 | GC part 1 + 2 push GATE-FAILs; the count read 18:13; the FILL chain 3.65 h (the rule-105 fix-gate bench, about 60 min lost by a watcher bug, 2 FILL re-makes) | OL M6-ORCH-1549, -1554, -1558..-1561, -1569, -1572..-1577 |
| 2 | 3.0 | the pre-box review fix round made a new head, so a second CI ran 21:36-23:58 and held the rows; then 3 FILL stops (no R13 source; the odds model language; the rule-105 gate) and 1 table-check GATE-FAIL | OL M6-ORCH-1574, -1587, -1589, -1593..-1602 |
| 3 | 3.3 | the end Path B needs the tag; the tag waited for CI 45679007969 and PR run 45695743905 on the same sha 0b710261 (both 68 / 68, serial, about 1.7 h each) and the fuzz T2 run of `m13-p0` (2.2 h) | OL M6-ORCH-1608, -1617, -1618, -1624..-1627 |

- The lane was busy 3.7 h of 15.9 h (23%) (D). CI waits on the box path were about 6.7 h (D). Code defects on the path: 0 (R19P §0).
- The OL times are written times, not event times; one row corrects a time by 13 min (OL M6-ORCH-1627).

### 2.3 Box use by kind, month 13

| Kind | Runs | box-h | Share of 7.68 | Plan v18 row | Source |
|---|---|---|---|---|---|
| lever A/B (GC 67 + 60 min; WIDE 65 + 72 min) | 4 boxes | 4.41 | 57% | rows 1, 2: 5.35-6.00 | UL:497-498, :516 |
| Path B (the end read, 54 + 54 min) | 2 | 1.80 | 23% | row 5: 1.90-2.10 | UL:527-528 |
| cause read (TD-4, 24 min) | 1 | 0.40 | 5% | row 9: 0.45-0.50 | UL:529 |
| fuzz (FUZZ-H, 64 min) | 1 | 1.07 | 14% | row 6: 1.08 | UL:531; OL M6-ORCH-1635 |
| **sum** | 8 | **7.68 of 15.9** (48%) | | rows 1, 2, 5, 6: 8.33-10.30 | OL M6-ORCH-1635 |

- No box ran for: one more box (GC MERGE on 2 boxes; WIDE M4 could not pass), LEVER-20 (NOT RUN), the mid Path B (its line failed: PBe13:384), LAYOUT-PIN, a lost box. Every box ran below its plan row (R19P §2.1).
- The hold rule (DL M13-PLAN-D10) never bound: the largest check was 14.48 <= 15.9 (OL M6-ORCH-1602). Months 12 and 13 both used about half the cap (R25 §2.1; this table).
- Unit costs for month 14 (M): a lever A/B box 60-72 min; an end Path B box 54 min; a TD box 24 min; a full 68-job CI run about 1.7 h; a fuzz T2 run 2.1-2.2 h (R19P §2.1).

### 2.4 Tokens (UL rows; D by a sum)

| Window | Tokens | Rows | Agents | Source |
|---|---|---|---|---|
| the month-13 work rows (UL:473-526) | **18.13 M** | 50 | 124 | R19P §2.2 (recount: 18.13 M) |
| every row with a month-13 phase, at the inputs read (incl. plan v18 parts 1-2, 2.61 M, and the A/B form run, 0.70 M) | 21.44 M | 53 | 134 | IN19 #77 |
| the same + plan v19 part 1 (UL:530, 1.04 M) | 22.49 M | 54 | 138 | D |
| the DL M13-END-D5 window at this record: every row with a month-13 phase, BOX rows left out (UL:431-533; the same + plan v19 part 2, UL:532, 0.97 M, and the first records workflow, UL:533, 1.12 M) | 24.58 M | 56 | 147 | D; DL M13-END-D5 |
| plan v18 estimate | 30-42 M, 110-160 runs | | | DL M13-PLAN-D23 |
| month 12 | 32.03 M | | | DL M13-PLAN-D23 |

| Set (UL:473-526) | Tokens | Agents | Rows | Note |
|---|---|---|---|---|
| WIDE-FIX (no merge) | 4.84 M | 36 | 14 | 27%; + 2.29 box-h |
| M13-P0 ride roles + integration + close | 3.62 M | 23 | 8 | the close workflow 1.10 M (UL:501) |
| GC-BUDGET (merged) | 3.53 M | 29 | 14 | |
| counts + research (alloc / SIMD, K0-K3, JIT-DELTA) | 3.15 M | 15 | 5 | the owner-direction research 1.95 M (UL:478) |
| month-end sets | 1.48 M | 10 | 3 | |
| tag prep + w60 | 0.78 M | 6 | 3 | |
| PRE rows; the pick | 0.49 M; 0.24 M | 2; 3 | 1; 2 | |

- Rework (re-makes after a check or a gate): 0.74 M + a stopped merge run 0.29 M (R19P §2.2). The tag-prep fact checks that plan v18 §7.1 cut ran anyway: 0.66 M (UL:521, :522, :526).
- The class split is one reader's map of each row to a set (R19P §6).

### 2.5 Refusals, checks and review rounds (R19P §1.3-§1.5)

| Event | Count | Cause | Source |
|---|---|---|---|
| push GATE-FAIL | 6 | from briefs or orchestrator steps: 2 moved cargo targets; no oxc sync in a new worktree; no adr-lint after a text change of a cited file; an invalid ADR trailer form in a brief; an unescaped `\|` in a DL row | OL M6-ORCH-1549, -1561, -1566, -1568, -1601 |
| CI infra | 1 | a lost macOS runner; attempt 2 green; about 1.9 h on the `m13-p0` tag | OL M6-ORCH-1604, -1607, -1615 |
| GitHub API rate limit | 1 | 4 pollers at 60-90 s; rule now: at most 2 pollers, >= 120 s | OL M6-ORCH-1556 |
| watcher bug | 1 | a shell `set --` overwrote the run list; about 60 min of missed run ends | OL M6-ORCH-1569 |
| DNS refusals before a box | 2 | a short DNS fault on the Mac (about 4 min); no box made | OL M6-ORCH-1631 |
| agent STOP (right) | 6 | grant gaps, FILL sources, a "do not push" check | OL M6-ORCH-1552, -1572, -1583, -1593, -1596, -1598 |

| Review | Rounds | (a) / (b) / (c) | Changed an outcome? | Source |
|---|---|---|---|---|
| PATCH-PREP + FUZZ-SMALL-STACK | 2 lens rounds, 2 fix rounds, a fix check | 0 / 9 / some | yes: 9 lint gaps closed with red / green fixtures | OL M6-ORCH-1556, -1563 |
| WIDE-FIX, before the box | JIT review, second-model round 1, 2 verifiers, 1 fix round | 0 / 4 / 3 | **no**: the x64 G1b bytes stayed equal after the fix; the set did not merge; cost 0.60 M tokens and a second CI that held the rows 2.4 h | OL M6-ORCH-1571, -1587, -1589, -1609 |
| GC-BUDGET, after the read | second-model rounds 1-3, 2 verifiers, a Claude lens | 3 / 2 / 3 | **yes**: the merge-gate tool could pass falsely before the fix | OL M6-ORCH-1582, -1586, -1588, -1605, -1606 |

About 30 check or verify agents ran. Their 6 (a) catches were all in 4 kinds of work: a rule-96 row, a box set, role code, merge rows. The end-set check, the FILL round-3 and VERDICT checks, the pick check, the 3 tag-prep fact checks and the 2 fix checks found 0 (a) (R19P §1.5).

### 2.6 Records load

| Item | Value | Source |
|---|---|---|
| month-13 DL rows (not the plan rows), at 67fdda4d | 21 rows, 194,972 characters = 9,284 per row (D: a character count; R19P wrote 195,195 and 9,295) | DL 23.2-23.5 |
| month 12 | 3,461 per row | R25 §2.6 |
| PREREG / FILL rows | 13-18 k characters each | R19P §3 C9 |

### 2.7 Proposals for the month-14 form (R19P §3)

| # | Proposal | Evidence | Est. gain |
|---|---|---|---|
| K1 | KEEP form F1, with 2 fixes in a month-14 PRE row before any box: replace each "Ir within 1e-5" line by the H0 trace-sig equality + an A/A Ir band measured before the box; print "M4 dead: no MERGE from more boxes" beside ONE MORE BOX | 2 decisive 2-box reads; the 1e-5 line read FAIL on noise (§1.2); box 3 needed a call (DL M13-P2-WIDE-FIX-VERDICT) | no call per lever |
| K2 | KEEP the hold rule | it never bound (§2.3) | - |
| K3 | KEEP checks on rule-96 rows, box sets, role code, merge rows; CUT the tag-prep fact check | §2.5 | 0.7-1.0 M tokens |
| C1 | MOVE the second-model review and its fix round to after the read for a lever whose FILL odds give final MERGE under 50%; keep the 0-box gate, the role check and a JIT reviewer before the box | §2.5 (WIDE before the box against GC after the read) | 2-2.5 h of box path per such lever (E) |
| C2 | a FILL kit: the bench of BENCH_HEAD vs BASE, the PROOF (a) dumps, the odds model, the rule-105 gate words, all before the FILL writer starts | GC: 2 re-makes; WIDE: 3 stops, the same 4 inputs both times | 1-2 h per lever (E) |
| C3 | a brief preflight: the 4 fork sync scripts in a new worktree, adr-lint + the table check, the trailer form check; no moved or linked cargo targets | the 6 GATE-FAILs (§2.5) | about 0.5 h + 6 re-runs |
| C4 | one CI per tag sha: the tag CI is the PR run; phase-close DL rows go into the head before its CI | gap 3 (§2.2) | about 1.7 h per tag (D) |
| C5 | let the end Path B start on the pushed final head, with the tag check at the read (VOID if the tag sha differs): a form change that needs its PRE row | gap 3 = 3.3 h; 1 CI infra loss in month 13 is the VOID risk | up to 3 h per month end (E) |
| C6 | one tag per lever merge (ride roles join it), or read a running prior fuzz T2 by one OL row: a product call (DL M13-PLAN-D7 chose the early tag) | `m13-p1` waited 2.2 h for the `m13-p0` fuzz T2 | up to 2 h (E) |
| C7 | state effort in wall hours from measured units | §2.1 | a true calendar for the DEPS date |
| C8 | KEEP the watcher rules (an array, at most 2 pollers at >= 120 s, stop each watcher when its run is read) | §2.5 | - |
| C9 | cap PREREG / FILL rows: cite M13-PRE-AB93 by sha, print only own values | §2.6 | fewer re-makes (I) |

---

## 3. Modes findings (R19M)

### 3.1 What the jit build is, and what it must keep

| Fact | Source |
|---|---|
| jit build = T0 + the `full` handler table ("sib") = T0 + the record hooks; no T1 exists (T1 is text only, DL M13-PLAN-D24) | `crates/ep9_vm/src/interp/mod.rs:545`; `docs/plan/13-execution-modes.md:101` |
| jitless build = T0 + the `ics_only` table ("ics"): no record code in the fast handlers | `crates/ep9_vm/src/interp/mod.rs:548-553`; `docs/plan/13-execution-modes.md:118` (MODES-D11) |
| M2 item 10: "Gates 1-3 hold with all H16/H29 profiling on"; PBe13 reads item 10 on the jit preset: MARGINAL, NO AGREEMENT | `docs/plan/09-milestones.md:113`; PBe13:142 |
| T1 "already writes the full FeedbackVector" | `docs/plan/13-execution-modes.md:801` |
| named loads have no T0 record hook | `crates/ep9_vm/src/profile.rs:56-70` |

- **"jit preset on `ics_only` until T1 exists" is not an option (R19M-D3).** It turns off what item 10 needs and it changes MODES-D11.
- **The reference engine records in the interpreter too (M, source).** V8's `UpdateFeedback` loads the slot, ORs the new bits, compares, and stores only on a change (`v8/v8@932ddd7ca4` `src/codegen/code-stub-assembler.cc:13803-13824`); the binary-op assembler calls it with no jitless branch (`src/ic/binary-op-assembler.cc:421`, `:428`; 0 case-free hits of "jitless" in that file).
- **Correction to R19M.** R19M wrote 0 "jitless" hits in `interpreter-generator.cc` and `interpreter-assembler.cc`. That holds only for the lower-case word: a case-free count gives 11 and 8. A build with the compile-time flag `V8_JITLESS` loads `undefined` in place of the feedback vector and uses `kNoFeedback` (`src/interpreter/interpreter-assembler.h:173-187`). The run-time flag `--jitless` turns off no feedback flag in its implication list (`src/flags/flag-definitions.h:990-1012`), and the box node prints `v8_enable_lite_mode=0` (TD4:389). So node with `--jitless` still records feedback (I: no `V8_JITLESS` build key is printed). The pair "jit build vs node --jitless" stays like for like (I).
- ep9's compare record costs about 51 Ir per `record_pair_next` call and the keyed cold record about 82 Ir per cold entry (JDR §2.1, §3). So the jit-only gap is a record-form cost, and the V8 form is the target. A lean form made now for T0 is the form T1 should copy (R19M §1.2 item 6; I).

### 3.2 JIT-LEAN: the proposed jit-only part (R19M-D1, -D2)

| Item | Proposal | Why |
|---|---|---|
| Order | not the first month-14 set: line (3) of DL M13-PRE-MONTH14 read BELOW (0.984 / 0.991; PBe13:414) | a bound of x1.012 is under the x1.015 floor; it needs a line override with no gate gain |
| Form | JDR ideas 1 + 2 only: a no-change test before `compare_bits` in `record_pair_next`; a pre-test before `keyed_key` in `record_keyed_cold` | each fact kind and site stays recorded and equal; G8 0 by R19M's read |
| Read | an arm or rider of the first both-build set; decision cell = the jit TS cell (JIT-CELL clause); the jitless cell IDENTITY | the `ics` table does not change |
| Proof | the jit build's feedback vectors byte-equal between base and lever head after a TS run and the corpus; the `ics` table symbols byte-equal (the G1b form) | "a cost cut only" must be a test, not a claim; it also shows that item 10 still holds |
| Not in M2 | JDR idea 3 (window tests off the hot path) | it changes when a fact is read: a MODES design row (plan v18 §11) and an item-10 question for the owner |

### 3.3 Drift risk of the month-14 sets (R19M §3)

| Set | Touches a jit-only path? | Version or contract it can move | Room effect | Must-show for its MODES row |
|---|---|---|---|---|
| NL-1 stage 1 (an FI word) | no: both tables | VM_LAYOUT 1 if an FI offset moves | none | the VM_LAYOUT number; each FI offset that native code reads, old -> new |
| NL-1 stage 2, HA S1-C, NL-3 | no | H14 frames, the handler ABI, IC_LAYOUT | **narrows** (every tier writes the slot) | kept out of M2 (DL M13-PLAN-D24) |
| MH | no | none today (a run-time table; only `ep9_vm` reads `stub_cache.rs`) | none today; if T1 later inlines the probe, the hash becomes a native-code fact with no version owner | the git grep; a doc-13 note row for that day |
| SHAPE-LOOKUP-CACHE | no | none if it is a pure cache | an M-R9 risk only if a hit can differ from the scan | flushed at every GC and on each in-place shape change (the R26 §7 simulation did not flush, so 90.3% is an upper bound); per isolate |
| CTOR-FIT | no | H17 slack tracking (+1 likely); OBJECT_LAYOUT only if a cell form moves | an object-model rule: M-R9 applies in full | the same shape and capacity counts under `full` and `ics_only`, with and without a loaded unit |
| GC-2 | no | H11 barrier (BARRIER_VERSION 2); the TLAB limit word | none if those stay | `barrier.rs` 0 diff; the limit word unchanged |
| JIT-LEAN | **yes, only the `full` table** | none if facts are equal (FACT_SCHEMA 2, IC_LAYOUT 4) | none; later T1 records cost less (I) | the facts byte-equality test; `ics` symbols byte-equal |
| SIMD module + L3, L6 | no | L6 lazy form: H02 (a G8 row) | the lazy hash moves when the unit identity is made | one detection, a scalar body, no GC point in a SIMD body; no SIMD body called from T1 or AOT code; x86 test on Cloudflare, aarch64 on the Mac |
| DEPS-BUMP-3 | no | the corpus digest; T-SITES 22 -> 25 | none | the plan-v18 base merge row |
| A1H-JOIN (carried) | no | H37 hints | widens (the AOT hint path) | DL M13-PLAN-D17 says it lands before DEPS-BUMP-3 |

- On wasm one module cannot test for `simd128` at run time: a module with SIMD opcodes fails validation on an engine without them (I, from the WebAssembly validation model). So "runtime detect" on wasm is 2 builds or a loader choice (R19M Q5).
- Scope note (R19M §3): every named-load and allocation set above is a both-build set. A set that starts to need a frame slot, a pinned value or a sampled fact has left its goal; it goes back to the plan or the owner.

### 3.4 What w61 must hold (R19M §4)

| # | Item | Source |
|---|---|---|
| C1 | w61 at the first month-14 tag or by 2026-12-03, whichever is first | OL M6-ORCH-1619 (w60 on 2026-11-26) |
| C2 | the month-13 room report: PATCH-PREP, FUZZ-SMALL-STACK and GC-BUDGET merged (GC-BUDGET is run-time tuning that compiled code never reads); WIDE-FIX, LEVER-20, A1H-JOIN not merged; G8 0 / +7; SAFETY 0 / +2 | OL M6-ORCH-1619; IN19 #68, #70 |
| C3 | the JIT-DELTA table by fact kind, plus the TD-4 cycle side (k 0.487), as the first input of the T1 baseline design | JDR §2.2; §1.5 |
| C4 | 2 cite fixes in the w60 text (2 plan-v18 line numbers point one row too early) | R19M §4 |
| C5 | close route JIT-CACHE-SIM on the TD-4 pair; keep AA-NOISE-1E5 for plan v19 | §1.5; §1.2 |
| C6 | the month-14 must-show table with one row per set of §3.3 | §3.3 |
| C7 | the weekly deep check names one outside fact; for w61: the V8 record form | §3.1 |

The month-13 room at `m13-p1` (OL M6-ORCH-1619): 7 versions unchanged (ABI 7, OBJECT_LAYOUT 9, IC_LAYOUT 4, DEOPT_META 1, VM_LAYOUT 1, CONTAINER 4, FACT_SCHEMA 2); one stream; G8 0 of +7 spent.

---

## 4. Open items plan v19 must place (IN19; R19P §4)

| Item | State at the round | Why it matters | Source |
|---|---|---|---|
| GC-2 first vs TD-4 line G | line (1) ABOVE vs 2.59% FAIL | the plan names the line it overrides and the new fact | §1.3; DL M13-PRE-MONTH14, M13-PRE-TD4 |
| line (5) of DL M13-PRE-MONTH14 | DEPS-BUMP-3 merges "at the first month-14 phase boundary on or after 2026-12-08" | if month 14 ends before 2026-12-08, line (5) has no boundary in month 14 (I); R19P proposes a date rule for any month | IN19 #7, #100; R19P §4.2 |
| A1H-JOIN | not started; DL M13-PLAN-D17 says it lands before DEPS-BUMP-3 | it moves `Cargo.lock` and timed front-end code; before the first lever box, or after the exit | R19P §4.3; R19M Q3 |
| the 0.160.0 scout | not run | 0 box-h; tells the plan before 2026-12-08 if a stop line is near | R19P §4.3; DL M13-PLAN-D8 |
| AA-NOISE-1E5 | same-sha A/A Ir 1.49e-5 / 1.76e-5 > the 1e-5 line | by rule 96 the fix must be in a row before data | §1.2; IN19 #37, #92 |
| the null ledger | 18 new null cells in month 13 (16 A/A cells of GC read 1: medians 0.9844-1.0110, 1 of 16 above 1.010; 2 IDENTITY cells of WIDE read 1); 42 + 18 = 60 only if the 42 older cells count (I); no ledger file in the repo at 67fdda4d; tau not computed again | DL M13-PRE-AB93 ("no form change before the ledger holds 60 pooled null cells") | IN19 #38-#40 |
| the GC LEDGER row | plan v18 §7.2 lists a LEDGER row for a merged lever (5 rows); DL 23.x has PREREG, FILL, VERDICT, MERGE-CALL, MERGE and no LEDGER row | owed or not: not verified | R19P Q6 |
| the splay fall | -0.8% to -4.3% on item 6 (§1.1) | cause not read (GC-BUDGET or the box draw) | IN19 #4 |
| the exit inputs | run 45713632628 (owner information) | not read at this commit | IN19 #13, #56 |
| user calls | the oxc stack-limit request and the HB-4 report: prepared, NOT SENT; the option paper (K default) | owner actions; the status offers them | DL M13-PLAN-D9, -D28; IN19 #11 |
| past-due routes | 26 route rows due M13-P1..P3 are past the last month-13 tag | close or re-due at the month close | IN19 §4 |

HB-4: a known weakness, already public in [docs/research/20](20-bestfit-data-structures.md); its design stays out of this record.

---

## 5. Where plan v19 uses the round

Plan v19 was being written at the round. It was accepted on 2026-11-26 at 08:38 +0800 (OL M6-ORCH-1637, -1638), and its rows are DL 24.1. The round's proposed decisions, for the plan writer to take, change or reject:

| Id | Proposal |
|---|---|
| R19S-D1 | override line (1) of DL M13-PRE-MONTH14 (GC-2 first) with the fact TD4:407 (2.59% < 4%) |
| R19S-D2 | the first month-14 set is CTOR-FIT; SHAPE-LOOKUP-CACHE and the SIMD riders ride it |
| R19S-D3 | the second set is RECORD-LEAN with the JIT-CELL clause, priced at the TD-4 k |
| R19S-D4 | NAMED-SET gets a 0-box count first; MH alone is an arm, never a read |
| R19S-D5 | NL-1 fact (c) is "at the line on one box"; no reopen on it alone |
| R19S-D6, R19M-D5 | close route JIT-CACHE-SIM with the TD-4 ledger; w61 holds the room report, the TD-4 pair and 2 cite fixes |
| R19M-D1..-D4 | JIT-LEAN is not first; it gets a 0-box design and rides the first both-build set; the jit preset on `ics_only` is not an option in M2; one must-show row per set |
| R19P K1-K3, C1-C9 | §2.7 |

Open questions the notes hand to the plan writer: the x1.015 floor per lever or per set of arms (R19S Q1); which t the cases use (R19S Q6); is k 0.487 from one box enough to run the JIT-LEAN design now (R19M Q1); does an AOT unit bake an in-object slot count (R19M Q4; not verified); is C1 a rule-20 change (R19P Q1).

---

## 6. Verified, not verified, limits

### 6.1 Verified

- The PBe13 values of §0, §1.1, §1.2 and §1.7 against the compare file (PBe13:29-30, :85-86, :142, :148-151, :384-415); the PBe12 cells of §1.1 (PBe12:29-30, :85-86). The end Path B box records: both boxes AMD EPYC 9684X, standard-4, `run.sh` rc 0, destroyed at +54 min, oxc pin 74e5e665 with 2 patches.
- The TD-4 values of §1.3-§1.5 against the summary (TD4:19-35, :71-72, :86-91, :170-174, :236-283, :389, :405-410).
- The month-13 verdict values against DL M13-P1-GC-VERDICT, M13-P1-GC-MERGE-CALL, M13-P2-WIDE-FIX-VERDICT, M13-P3-PICK at 67fdda4d.
- The derived values: the needs; k; the branch-miss and L1i counts; the JIT-LEAN bounds (one correction: ideas 1 + 2 x1.012 and all jit-only Ir x1.014 at k 0.487, where R19M wrote x1.0117 and x1.0142); the ratios 0.626 / 0.662; the A/B needs and the case table; the GC-2 top; the splay falls; the wall 15 h 52 min; the box-h sums.
- The token sums by a recount of UL:473-526 (18.13 M, 50 rows, 124 agents) and of every month-13 phase row (21.44 M before UL:530; 22.49 M with it); and of the DL M13-END-D5 window at this record (UL:431-533: 24.58 M, 56 rows, 147 agents). The DL character count (194,972, not 195,195).
- The FUZZ-H result: the ledger row and its record (0 crashes, 4.0067 CPU-h, 3,606 s, 4 workers, counted false, commit 0b710261) and OL M6-ORCH-1635.
- The ep9 cites at 0b710261: `stub_cache.rs:28`, `:41`, `:75-82`, `:148-151`; `profile.rs:56-70`; `interp/mod.rs:545`, `:548-553`; `docs/plan/09-milestones.md:113`; `docs/plan/13-execution-modes.md:101`, `:118`, `:801`.
- The engine cites of §1.4 and §3.1 at the named commits. One correction to R19M (§3.1): the `V8_JITLESS` build flag appears in the V8 interpreter files.

### 6.2 Not verified

- Every month-14 set gain and every case of §1.7 (no month-14 box).
- The exit chances (guesses).
- The 29.16 M megamorphic loads per TS run (an old Mac count); the L1d / L2 split of the probe cost (no cache simulation).
- Node's main-thread GC share (the node thread names do not split).
- The G8 readings of RECORD-LEAN, the table form and SHAPE-LOOKUP-CACHE (no ADR grep).
- "A1H-JOIN and the scout did not start": no OL row names them (an inference).
- Whether node 24 is a `V8_JITLESS` build (the key is not printed; inferred from `v8_enable_lite_mode=0`).
- The wasm `simd128` statement (the validation model, not a test in this repo).
- The idle-gap hours (OL written times) and the token class split (one reader's map).
- The exit-inputs run 45713632628.

### 6.3 Limits

- One end read (2 boxes) and one TD-4 box; k 0.487 is one aggregate from medians, with no interval for the cycle ratio. The TD-4 wall pair straddles (0.980 [0.965-1.006]).
- One box type (Cloudflare standard-4, EPYC 9684X); no gate host.
- The V8 cites are from the clone head 932ddd7ca4, not from the box node's V8 13.6.233.17.
- Plan v19 is not cited: it was written after the round (accepted on 2026-11-26 at 08:38 +0800).

## Raw numbers

[artifacts/m2-month14-plan-research/](artifacts/m2-month14-plan-research/README.md) holds `findings.json` (one object per finding, with its status, sources and numbers), `levers.tsv` (the month-14 set table of §1.4 and §1.6) and `forecast-later.tsv` (the table of §1.9).
