# The month-12 off-loop read: main-thread GC against node's helper threads, the month-12 levers, the Path B read-to-read move, and the parser stack check

## Run context

| Item | Value |
|---|---|
| Dates | 2026-11-24 to 2026-11-25 |
| Round | The month-12 records of the plan-v18 research: the TD-3 cycle read (2 boxes), the WIDE-SET A/B (2 boxes), the month-12 end Path B (2 boxes), the 3 lever verdict rows, the L1 parser stack check (merged at `m12-p1`) and the off-loop research text of route R24-OFFLOOP. This report collects them. It runs no new box and no new measurement |
| ep9 trees | `m11-p3` 016a7061 (the last month-11 tag; the earlier switch read); 2c5002c8 (the month-12 base: the engine code of `m11-p3` + the DEPS-BUMP-2 lock moves; the TD-3 tree); 1ae919cc (the WIDE-SET probe head); 5ca1610c (the GC-BUDGET probe head, never on a box); `m12-p0` 2a792a87; `m12-p1` fcac0863 (the end tag; the end Path B tree). Code cites (`crates/...`, `third_party/...`, `docs/adr/...`) are at 6cbc3e2d (`m2/m12`). `git diff --stat fcac0863 6cbc3e2d -- crates third_party tools Cargo.lock` is empty, so they are also true at `m12-p1`. oxc_parser is 0.158.0 from the pin 74e5e665 with 2 local patch files |
| Boxes | Cloudflare standard-4, AMD EPYC 9684X (Zen 4; family 25, model 17, stepping 2, microcode 0x1000065), 4 vCPU, the engines pinned to CPUs 2-3. 6 boxes in month 12 before the health run: ep9-x86-m12-td3-1, -td3-2, -wide-1, -wide-2, -end-pathb-1, -end-pathb-2. The health fuzz box (DL M12-FUZZ-H) is not read here |
| Reads used | DL rows M12-PLAN-D2, -D9, -D10, -D11, M12-PRE-SWITCH, M12-PRE-MONTH13, M12-PRE-TD3-READ, M12-P2-WIDE-VERDICT, M12-P1-GC-NOT-RUN, M12-P3-PICK, M12-L1-START, M12-L1-KIND, M12-P0-LEDGER, M12-P1-LEDGER, M12-P1-W58, M12-FUZZ-H, M10-END-D3, M11-END-PATHB; the 2 TD-3 box summaries; the WIDE-SET pooled read text; the month-11 and month-12 end Path B compare files; the L1 speed rows and review records; the box rows of the usage log |
| Mac reads | The L1 parse-only speed rows (aarch64-apple-darwin, release, 5 pairs); the hole hunt of DL M12-L1-START (release, macOS arm64); the GC-BUDGET E line (Mac GC counts, DL M12-P1-GC-NOT-RUN). 0 Mac time reads |
| Upstream | V8 at tag `13.6.233.17` (commit b0a55a7d), the V8 of the box node (`v24.22.0`, V8 `13.6.233.17-node.57`, TD3:8). Each V8 cite of §1.4 was read at that tag for this report. Node's own V8 patches (`-node.57`) and the node v24.22.0 sources were not read from a checkout: node cites are marked "not verified" |
| Method | Read only. Every number has a cite. **M** = measured (a box, a CI run, a log). **D** = derived (arithmetic from cited numbers; the inputs are shown). **I** = inferred. **E** = estimate (basis named). **G** = guess. **O** = open |
| Limits | One box type (EPYC 9684X, SMT on, 2 CPUs pinned); no gate host. The TD-3 boxes read the TS cell near 1.0, not at the gate value (§1.6). Node's main-thread GC share is not measured. Each Path B read is 31 rounds per box; the 2 end reads are not paired with each other |

Short forms:
- DL = [docs/decisions-log.md](../decisions-log.md), by row id. OL = the orchestrator log, by row id M6-ORCH-n (not in the repo).
- TD3 = the TD-3 read box summary (box ep9-x86-m12-td3-2, code 2c5002c8; DL M12-PRE-TD3-READ); TD3:n = its line n. TD3-1 = the box-1 summary (ep9-x86-m12-td3-1). The summary files are not in the repo.
- PB10, PBe11, PBe12 = the Path B reads at the month-10 end (DL M10-END-D3), the month-11 end on `m11-p3` (DL M11-END-PATHB; boxes ep9-x86-m11-end-pathb-1, -2) and the month-12 end on `m12-p1` (boxes ep9-x86-m12-end-pathb-1, -2). PBe12:n = line n of the month-12 compare file. At this head the DL has no month-12 end row yet; the numbers are in [artifacts/m2-month12-offloop/](artifacts/m2-month12-offloop/README.md).
- WR = the WIDE-SET pooled read text (boxes ep9-x86-m12-wide-1, -2; DL M12-P2-WIDE-VERDICT).
- R24B = the off-loop research base text of route R24-OFFLOOP (plan v17 §4.6; not in the repo). P17 = plan v17 (DL M12-PLAN-D*). R18S, R18P = the plan-v18 research notes on speed and process (not in the repo; written before the end read: their forecasts are forecasts). L1D = the L1 design note; L1S = the L1 parse-only speed rows; RV1, RV2 = the L1 review rounds (all not in the repo; the H01 and H23 Review record rows summarize RV1 and RV2).
- R23 = [23-m2-month11-levers.md](23-m2-month11-levers.md). FD = `src/flags/flag-definitions.h` of V8 at `13.6.233.17`.
- Ratios: an A/B ratio is t(lever) / t(base), paired per round (below 1 = the lever is faster). A gate ratio is t(node --jitless) / t(ep9) (above 1 = ep9 is faster). "x1.0nn" = a time gain. "Points" = 100 x a ratio difference. Intervals are the order-statistic 95% intervals of the reads.

---

## 0. Verdict

```
 off-loop, TS jitless (TD-3 read box; box 1 in [])           ep9               node --jitless
 all-thread cycles                                             25.135 G [25.178] 25.355 G [25.222]   ep9 0.9% fewer
 task-clock / wall (CPU busy per wall second)                  1.026 [1.026]     1.040 [1.039]       node +1.4%
 GC on the JS thread (% of all cycles)                         4.43% [5.21%]     not measured (est. 1.3%)
 helper threads (% of cycles samples)                          2.88% [3.51%]     V8Worker 4.82% [4.72%]
 main-thread GC excess ep9 - node (D + I)                      0.78 G = 3.2% of ep9 main-thread cycles (2.3-4.1%)

 month-12 levers        predicted (A/B center)   measured                       verdict
 WIDE-SET               x1.013                   x1.023 (0.9775)                GUARD-FAIL (t0-loop jit 1.0155)
 GC-BUDGET              x1.005 (R24B: x1.003)    no box                         NOT RUN
 LEVER-19               floor x1.015             no box (NL-1 priced x1.004-1.005) NOT RUN
 product                x1.013                   x1.0000                        transfer NOT DEFINED

 Path B TS (box 1 / box 2)   m10 end         m11 end         m12 end         move m11 -> m12 (nearly the same VM code)
 jit                         0.882 / 0.885   0.970 / 0.974   0.968 / 0.980   x0.998 / x1.005
 jitless                     0.942 / 0.942   0.972 / 0.980   0.985 / 0.988   x1.013 / x1.008   (box 1 0.985 [0.978-1.001] straddles)
```

1. **The wall gap that is not CPU time matches node's extra helper CPU (M, D).** At both end reads the TS wall ratio sits 1.2-1.7 points under the CPU-time ratio. On the TD-3 boxes node runs 1.040 CPUs per wall second and ep9 1.026: node / ep9 = 1.0136 (1.4 points) (§1.1).
2. **ep9 spends 4.43% of its TS cycles on GC on the JS thread (M; box 1 5.21%).** That is 1.11 G of 24.36 G main-thread cycles. Node's main-thread GC is not measured; at an estimate of 1.3% the excess is about 0.78 G = 3.2% (range 2.3-4.1%). In main-thread work that is not GC, ep9 is about 0.47 G ahead of node (D + I) (§1.2).
3. **The 92.83% "node helper share" of the first summary is a thread-name error (M).** The summary script counted every node thread whose name is not `node` as a helper. Node's JS thread is named `MainThread` (88.02%). The real helper share is `V8Worker` 4.82% (§1.3).
4. **`node --jitless` keeps every GC thread (V8 source; I for node's own `-node.57` patches, not read; the `V8Worker` share is M).** `--jitless` turns off code generation only. Parallel scavenge, concurrent marking, concurrent sweeping and parallel compaction stay on; only `--single-threaded-gc` turns them off. ep9 on 2 CPUs has 1 mark helper that works only inside the stop-the-world pause, plus 1 sweeper thread (§1.4).
5. **Month 12 merged no speed lever (M).** WIDE-SET read x1.023 on its decision cell, above its predicted high end, and closed GUARD-FAIL on one guard cell. GC-BUDGET met both of its conditions and was NOT RUN, because its guard line would most likely give a false GUARD-FAIL. LEVER-19 was NOT RUN by its pick rule. Rule 101: A/B product x1.0000, transfer NOT DEFINED (§2).
6. **The end Path B on `m12-p1` does not fire the switch (M).** TS jit 0.968 / 0.980, jitless 0.985 [0.978-1.001] / 0.988 [0.981-0.998]. The need at the low ends is now jitless x1.019-1.022 and jit x1.041-1.042 (D) (§3).
7. **The read-to-read move on nearly the same VM code is up to +1.3% (1.3 points) on the TS cells and up to +4.9% (5.5 points) on other gate cells (M, D).** The month-11 and month-12 end reads run the same ep9 runtime VM source; from the 2c5002c8 code to `m12-p1` whole-run instructions moved by at most +0.005% per BENCH-SET row in each phase; the DEPS-BUMP-2 lock step before 2c5002c8 has no whole-run read of its own tree here. 15 of the 16 per-box values moved up; 7 of the 8 cells moved up on both boxes. The move is most probably a read draw or a placement effect, not instructions (I; §3.3).
8. **No off-loop lever list reaches the gate need in the center case (E).** The G8-0 items give about x1.008 in A/B terms at their centers. Only S2b (concurrent marking, a plan change) has a high end near the need (§4).
9. **Layer 1 of the parser stack check is built and merged (M).** 22 sites in one oxc_parser patch, a per-thread limit, a gated depth walk with 11 gated points and a visitor-graph proof test, and T-HOLES with 321 rows. Parse-only cost +0.455% of instructions on Octane x20, under the +1.0% gate (§5).
10. **A proof that enumerates the structure found more than the review did (M).** Review round 2 found one HIGH defect (an `eval` text that aborted the process) with 2 open chains. The visitor-graph proof then found 5 open cycle groups. The second-model review could not read the parser-stack code: its service filter stopped it (§5.3, §6).

---

## 1. The off-loop result

### 1.1 Wall time against CPU time

| Read | Build | Wall gate ratio (box 1 / box 2) | CPU-time ratio | CPU minus wall (points) | Source |
|---|---|---|---|---|---|
| PBe11 (`m11-p3`) | jitless | 0.972 / 0.980 | 0.989 / 0.997 | 1.7 / 1.7 | DL M11-END-PATHB; PBe11:316 |
| PBe11 | jit | 0.970 / 0.974 | 0.987 / 0.990 | 1.7 / 1.6 | PBe11:286 |
| PBe12 (`m12-p1`) | jitless | 0.985 / 0.988 | 0.997 / 1.001 | 1.2 / 1.3 | PBe12:85, :316 |
| PBe12 | jit | 0.968 / 0.980 | 0.981 / 0.996 | 1.3 / 1.6 | PBe12:29, :286 |
| TD-3 (box 2; box 1) | jitless | task-clock / wall: ep9 1.026 (1.026), node 1.040 (1.039) | - | node / ep9 = 1.0136 (1.0127) | TD3:21, :24; TD3-1:23, :26 (D) |

- The CPU-time ratio counts every thread of each process (user + sys). So the wall gap that is not CPU time is the CPU that node runs on its second CPU while its JS thread runs, less the same for ep9 (I).
- The TD-3 ratio of CPU per wall second (1.4 points) has the size of the end-read gaps (1.2-1.7 points) (D). The match is between different boxes and different harnesses.
- The clock is not the cause: cycles per task-clock second are 3.599-3.611 GHz for node and ep9 and 3.623-3.629 GHz for QuickJS on both TD-3 boxes (D, TD3:21-25). SMT and wait time are not excluded.

### 1.2 The split of the main thread (TD-3)

| Row | ep9 e-jitless (box 2; box 1) | node --jitless (box 2; box 1) | Mark | Basis |
|---|---|---|---|---|
| all-thread cycles | 25.135 G; 25.178 G | 25.355 G; 25.222 G | M | TD3:21, :24; TD3-1:23, :26 |
| JS-thread comm / listed samples | 89.94 / 92.81; 89.52 / 93.03 | 88.02 / 92.83; 87.87 / 92.58 | M | TD3:236, :239; TD3-1:238, :241 |
| main-thread cycles | 24.36 G; 24.23 G | 24.04 G; 23.94 G | D | all-thread x JS-thread / listed (I: the rows under 0.05% split like the listed rows) |
| GC on the JS thread (line G) | 4.43% [4.38-4.47] = 1.11 G; 5.21% = 1.31 G | not measured; est. 1.3% = 0.33 G (range 0.5-2.2%) | M; node E | TD3:247, :395; node: 6.1% all-thread GC-like at TD-2 (R23 §5.2) minus `V8Worker` 4.82% (I: with no compile jobs, `V8Worker` work is GC) |
| GC on all threads | 6.81%; 8.26% | `Scavenger::ScavengeObject` 3.60%; 3.65% (no other GC row in the 30 listed rows) | M | TD3:173, :247; TD3-1:175, :249 |
| main-thread cycles that are not GC | 23.24 G; 22.92 G | about 23.71 G; 23.61 G | D + I | rows above |
| **main-thread GC excess ep9 - node** | **about 0.78 G = 3.2% of ep9 main (2.3-4.1%); box 1 0.98 G = 4.1%** | | D + I | rows above |
| GC count per TS run | e-jitless 11; e-jit 13; `EP9_GC_HELPERS=0` 10 (box 1: 12; 13; 10) | not measured | M (1 `--gc-stats` run per arm) | TD3:280-283; TD3-1:282-285 |
| main-thread GC per GC | 1.11 G / 11 = 0.10 G = 0.40% of cycles (box 1 0.43%) | - | D | TD3:281 |
| JS-thread GC with 0 helpers | 7.27% (box 1 6.98%): the helpers take 2.84 points (1.77) | - | M, D | TD3:249; TD3-1:251 |
| wall, 0 helpers / default | 1.022 [1.010-1.064] (box 1 1.026 [1.004-1.073]) | - | M | TD3:29; TD3-1:31 |
| all-thread cycles, 0 helpers / default | 0.992 [0.978-1.029] | - | M | TD3:30 |

```
       main-thread cycles, TD-3 read box (G; D + I)
ep9    |======================= non-GC 23.24 =======================|= GC 1.11 =|     24.36
node   |========================= non-GC 23.71 =========================|0.33|       24.04
       ep9 is about 0.47 G ahead in non-GC work, and about 0.78 G behind in main-thread GC
```

- Line G counts every `ep9_heap` symbol on the JS thread, so `alloc_slow` (0.68% on all threads, TD3:98) and the barrier slow paths are inside it. The part that a GC policy can change is smaller than line G (I).
- The 2 boxes drift on the same tree: line G 5.21% -> 4.43%, all-thread GC 8.26% -> 6.81%, GC count 12 -> 11. The cause is not known (O).

### 1.3 The 92.83% error of the first summary

| Check | Value | Shows |
|---|---|---|
| summary line "off-main" for node | 92.83% (box 1 92.58%) | TD3:399 |
| the rows behind it | `MainThread` 88.02% + `V8Worker` 4.82% = 92.84% (rounding) | TD3:239 |
| the cause | the summary script names node's JS thread `node`; on the box that thread is named `MainThread`, so the script counted the JS thread as a helper | DL M12-PRE-TD3-READ ("a thread-name effect of the read") |
| a cross-check | node task-clock / wall 1.040: about 1.04 CPUs busy, not 2; CPU above one busy thread = (1.040 - 1) / 1.040 = 3.8% of node's CPU time | D; TD3:24 |
| the value to use | `V8Worker` 4.82% (box 1 4.72%); it is above the 3.8% floor, which fits a main thread that waits about 1% of wall (I) | TD3:239 |

No verdict line used the 92.83% value: it is a report number (DL M12-PRE-TD3-READ). The fix belongs to the next TD box set (O).

### 1.4 What `node --jitless` runs off its main thread on 2 CPUs (V8 `13.6.233.17`, b0a55a7d)

| Item | State under `--jitless` | Source (upstream path:line at the tag) |
|---|---|---|
| `jitless` | "Disable runtime allocation of executable memory." | FD:823-824 |
| what it turns off | field type tracking (FD:829), regexp native code (`regexp_interpret_all`, :832), TurboFan (:833), Sparkplug (:835-836), Maglev (:839), concurrent Sparkplug (:1114) | FD |
| lazy compile | on (`lazy`, :2555); the lazy compile dispatcher and the parallel compile tasks are `DEFINE_EXPERIMENTAL_FEATURE`, default false (:2591-2605; the macro :232-234). So lazy functions compile on the main thread (I) | FD |
| parallel scavenge | on (`parallel_scavenge`, true) | FD:2143 |
| concurrent marking | on, under `V8_ATOMIC_OBJECT_FIELD_WRITES` | FD:2159-2160; `BUILD.gn:190`, `:640-641` (`v8_enable_concurrent_marking = true`) |
| parallel marking in the atomic pause; incremental marking | on | FD:2173; FD:2129 |
| concurrent sweeping; parallel compaction, pointer update, weak-ref clearing | on | FD:2178-2185 |
| young generation | the Scavenger; `minor_ms` is off | FD:3046 |
| background GC tasks | on: `Heap::ShouldUseBackgroundThreads()` is false only when `single_threaded_gc_in_background` (default true, FD:3473) and efficiency mode are both on; efficiency mode is off at the default priority `kUserBlocking` | `src/heap/heap.cc:456-459`; `src/execution/isolate.h:2094-2099`, `:2664-2665` |
| the only switch that turns the GC threads off | `--single-threaded-gc` (or `--single-threaded`): it negates concurrent marking and sweeping, parallel compaction, marking, pointer update, weak-ref clearing and scavenge | FD:3444-3471 |
| scavenge tasks | max(1, min(new-space MB + 1, `kMaxScavengerTasks` 8, worker threads + 1)); the main thread joins the job | `src/heap/scavenger.cc:1033-1050`, `:866-868`; `src/heap/scavenger.h:231` |
| concurrent marking tasks | up to `concurrent_marking_max_worker_num` = 7 | FD:2166-2167; `src/heap/concurrent-marking.cc:333-341` |
| major sweepers | min(sweeping spaces, worker threads + 1), posted at `kUserVisible`; concurrency min(sweepers, workers + pages / 2) | `src/heap/sweeper.cc:108-142`, `:299-310` |
| semi-space size | 512 KiB x `kPointerMultiplier` to 32 MB x the same unit; growth factor 2. The box node has pointer compression off (TD3:389), so the multiplier is `kTaggedSize / 4` = 2 | `src/heap/heap.cc` (`DefaultMinSemiSpaceSize`, `DefaultMaxSemiSpaceSize`), `src/heap/heap.h:300-303`; FD:921, :2060 |
| node worker pool | 4 `V8Worker` threads by default on any host, plus a delayed-task thread | node v24.22.0 `src/node_options.h`, `src/node_platform.cc`: **not verified** (no checkout). The box shows the comm `V8Worker` (TD3:239) |

```
 2 CPUs (taskset -c 2,3)   node --jitless                                ep9 (EP9_GC_HELPERS unset: 1 helper)
 CPU 2 (JS thread)         JS | scavenge (joins) | JS | atomic pause | JS  JS | roots, first 64 K cells alone, drain | JS
 CPU 3                     scavenge | concurrent mark, sweep WHILE JS RUNS  idle | drain (inside the pause only) | sweep
```

| Work | node --jitless | ep9 at `m12-p1` | ep9 source |
|---|---|---|---|
| young collection | parallel, stop-the-world, copying | sticky-mark minor GC, stop-the-world | `crates/ep9_heap/src/heap.rs:1433-1440`, `:1491` |
| old marking | concurrent on `V8Worker` threads while JS runs | inside the stop-the-world major GC | `crates/ep9_heap/src/parallel.rs:1-21` |
| mark helpers on 2 CPUs | up to 7 tasks, bounded by the 4 workers | `min(cores - 1, MAX_HELPERS 4)` = 1; the heap's thread marks the first 64 K cells alone | `parallel.rs:93`, `:103`, `:125-128` |
| sweeping | concurrent | 1 sweeper thread when >= 16 pages are queued | `parallel.rs:47-50`, `:697`, `:1032` |
| thread names | `MainThread`, `V8Worker` | `ep9-isolate`, `ep9-gc-marker-1`, `ep9-gc-sweeper`, `ep9-frontend` | `crates/ep9_cli/src/run.rs:464-468`; `parallel.rs:541`, `:1032`; `crates/ep9_frontend/src/worker.rs:168` |

So the only background work that `--jitless` removes is compile work (I: from the V8 flags at the tag; node's `-node.57` patches were not read). Node can overlap old-generation marking with JS; ep9 can overlap only the sweep (I).

### 1.5 What an off-loop lever must remove

| Need (gate, low ends) | Main-thread cycles to remove, 1 - 1/g (read box; box 1) | Share of ep9 main-thread GC (1.11 G; 1.31 G) | Source |
|---|---|---|---|
| PBe11: jitless x1.035, jit x1.048 | 0.82-1.12 G; 0.82-1.11 G | 74-100%; 62-85% | DL M12-PLAN-D2 (the need); D |
| PBe12: jitless x1.019-1.022 | 0.45-0.52 G | 41-47%; 34-40% | PBe12:391 (low ends 0.978 / 0.981); D |
| PBe12: jit x1.041-1.042 | 0.96-0.98 G | 86-88%; 73-74% | PBe12:387 (low ends 0.961 / 0.960); D |

- R24B wrote the PBe11 need as 0.85-1.17 G = 77-105% (a linear form, g - 1). The form 1 - 1/g gives 0.82-1.12 G (D).
- To close the jit need by the off-loop part alone, ep9 must take its main-thread GC to about node's level or lower (D + I). A budget value removes GCs, not the mark work of the GCs that stay: TS minor GCs promote 60-70% of the young bytes (R24B, from a trace build; not read again here).
- The PBe12 need is smaller than the PBe11 need on nearly the same VM code. That change is the read-to-read move of §3.3 (a read draw or a placement effect; I), not a code gain.

### 1.6 The TD-3 boxes do not show the gate gap

| Cell (TS e-jitless against node --jitless) | Read box | Box 1 | Source |
|---|---|---|---|
| A/B form (11 rounds) | 0.997 [0.976-1.022] | 0.994 [0.968-1.031] | TD3:27; TD3-1:29 |
| Path B form (medians, not paired) | 1.005 | 0.994 | D: TD3:276-277; TD3-1:278-279 |
| Path B form / A/B form wall: ep9; node | 1.035; 1.038 | 0.997; 1.003 | TD3:400; TD3-1:402 |
| the end gate cells on the same engine code | 0.972 / 0.980 (PBe11), 0.985 / 0.988 (PBe12) | | §3 |

The harness form moves ep9 and node by the same amount (0.3-0.6% apart; DL M12-PRE-TD3-READ), so the harness is not the gap (M). Why the TD-3 boxes read about 1.0 while the Path B boxes read 0.97-0.99 is open (O).

---

## 2. The month-12 lever table

| Lever (phase) | What it changes | Predicted A/B gain, TS jitless (low / center / high) | Measured same-binary A/B | Verdict row | Path B transfer (rule 101) |
|---|---|---|---|---|---|
| TD-3 (M12-PRE; a cause read) | nothing: a read of 2c5002c8 with GC arms and the Q1 harness arm | - | W +0.269 G PASS; G 4.43% PASS at 4%, FAIL at 6%; N +3.506 G PASS; M "not read" (box 1 VOID for line N, so box 2 is the read of every line); 11 rounds per box, 2 boxes, 0.91 box-h | DL M12-PRE-TD3-READ | n/a |
| GC-BUDGET (M12-P1) | a larger S1 budget above 12 MiB of old bytes: arm B1 12 MiB + 5 x (old - 12 MiB), arm B2 6 x, cap 144 MiB; the first budget stays 8 MiB (the 32 MiB first budget was measured and dropped, DL M12-P1-GC-PREREG); one binary, switch `EP9_GC_BUDGET` | x1.000 / x1.005 / x1.015 (G; P17 §4 row 1); R24B after TD-3: x1.001 / x1.003 / x1.010 (E) | **NOT RUN**, 0 boxes. Both conditions met: line G 4.43% (M) and the Mac E line 0.0221 (D). Bench TS Ir of arm B1: -1.295% jit, -1.306% jitless; BENCH-SET rows within +0.002% (runs 45349020669, 45349037862) | DL M12-P1-GC-NOT-RUN | no gain: not merged |
| WIDE-SET (M12-P2) | W-1: 11 far-jump twin opcodes with no prefix; W-2: a 256-entry wide table with `_w2` handlers; one binary, switch `EP9_WIDE_FORMS` | x1.007 / x1.013 / x1.020 (P17 §4 row 2; R23 §6.1) | decision cell **0.9775 [0.9712-0.9895] = x1.023**, 60 pairs, 2 boxes; cycles 0.9814; jit TS 0.9957; default form 0.9871; all 6 MERGE lines pass; guard t0-loop jit **1.0155 [1.0016-1.0245]** > 1.010; A/A 0.9913 [0.9739-1.0116]; 2.62 box-h | DL M12-P2-WIDE-VERDICT: **GUARD-FAIL** | no gain: not merged |
| LEVER-19 (M12-P3) | the first candidate that meets its TD-3 line and an est. A/B gain >= x1.015 | every research estimate <= x1.010 (P17 §4.4) | **NOT RUN**: GC-2 out (line G < 6%); NL-1 stage 1 in the list (line N) but priced x1.005 (est.) / x1.004 (its check) < x1.015; MH closed (line M not read); JUMP-INVERT not open; F2 not fired | DL M12-P3-PICK | no gain |
| **month product** | | **x1.013 merge-weighted center; x1.003-1.010 at the Path B** (DL M12-PLAN-D2) | **A/B product x1.0000** | | **Path B y1.0108 (box 1 1.0134, box 2 1.0081); transfer NOT DEFINED** (no merged set, ln x = 0; PBe12:401-405) |

Ride roles, not lever sets (PBe12:402): STACK-FORMS (FORMS 49 -> 75 forms, debug PARSE_PER_UNIT 6,144 -> 7,408; tag `m12-p0`; DL M12-P0-LEDGER, M12-P0-W57) and L1-STACK-CHECK (§5; tag `m12-p1`). Rows are in [levers.tsv](artifacts/m2-month12-offloop/levers.tsv).

### 2.1 What decided the 3 verdicts

| Fact | Value | Mark | Source |
|---|---|---|---|
| same-code cells of the month-11 and month-12 reads | 42 (34 of them 1-box A/A cells, n 29-31) | M | DL M12-P2-WIDE-VERDICT |
| a null cell above the 1.010 guard line | 6-11% of reads (a model with the pooled noise of each row) | D | same |
| a false GUARD-FAIL on 10 null cells (WIDE-SET) / 14 null cells (GC-BUDGET) | 46-68% / 59-81% of reads | D | DL M12-P2-WIDE-VERDICT, M12-P1-GC-NOT-RUN |
| the t0-loop cost of W-2 | t0-loop runs a prefixed op on each loop pass; W-2 swaps the `Wide` handler; bench Ir of t0-loop +0.382% | M; the cause I | DL M12-P2-WIDE-VERDICT |
| box-1 A/A of t0-loop jit (same code) | 1.0190 [0.9862-1.0468] | M | WR:166 |
| the NL-1 price | break-even 0.371 G = 0.86 cycles per IC run; center 0.10-0.123 G; the whole named-load cycle gap to node +0.244 G = x1.0098 | D, E | DL M12-P3-PICK |

- **WIDE-SET's measured A/B is above its predicted high end** (x1.023 against x1.020), as LDAR-ELIDE was in month 11 (x1.043 against x1.021, R23 §2.3). The guard line, not the gain, closed it. The guard line was registered before the data and is read as printed (DL M12-P2-WIDE-VERDICT).
- **GC-BUDGET is NOT RUN by a decision row, not by a condition miss.** A corrected guard line written after the WIDE-SET data would have been a moved goal post; that row was designed, attacked and rejected (DL M12-P1-GC-NOT-RUN).
- R24B forecast that the E line would "likely fail" (B1 must remove at least 4 of 11 GCs). The Mac E line read 0.0221 (D), above 0.015. **That forecast is refuted.** With the box-1 values (5.21%, 12 GCs) B1 needs at most 8 GCs (E = 0.0174), not 7 as R24B wrote (D).

---

## 3. The Path B series

### 3.1 The reads (items 1, 2, 3, 6; box 1 / box 2)

| Item | Build | PB10 (`m10-p5`) | PBe11 (`m11-p3`) | PBe12 (`m12-p1`) | Threshold | PBe12 result |
|---|---|---|---|---|---|---|
| 1 Octane-7 vs QuickJS | jit | 1.709 / 1.724 | 1.898 / 1.886 | 1.913 [1.887-1.931] / 1.909 [1.902-1.932] | 1.33 | AGREE above |
| 1 | jitless | 1.795 / 1.802 | 1.929 / 1.934 | 1.955 [1.929-1.978] / 1.962 [1.952-1.971] | 1.33 | AGREE above |
| 2 v8-v7 vs node --jitless | jit | 1.068 / 1.054 | 1.165 / 1.162 | 1.197 [1.186-1.205] / 1.188 [1.179-1.213] | 1 | AGREE above |
| 2 | jitless | 1.107 / 1.107 | 1.182 / 1.179 | 1.212 [1.203-1.219] / 1.215 [1.206-1.224] | 1 | AGREE above |
| 3 TS vs node --jitless | jit | 0.882 / 0.885 | 0.970 [0.954-0.977] / 0.974 [0.971-0.985] | 0.968 [0.961-0.982] / 0.980 [0.960-0.998] | 1 | AGREE below |
| 3 | jitless | 0.942 / 0.942 | 0.972 [0.966-0.982] / 0.980 [0.968-0.990] | **0.985 [0.978-1.001]** / 0.988 [0.981-0.998] | 1 | NO AGREEMENT |
| 6 splay vs node --jitless | jit | 1.097 / 1.103 | 1.105 / 1.118 | 1.159 [1.128-1.179] / 1.171 [1.138-1.186] | 1 | AGREE above |
| 6 | jitless | 1.094 / 1.107 | 1.116 / 1.098 | 1.171 [1.130-1.211] / 1.141 [1.101-1.187] | 1 | AGREE above |

Sources: DL M10-END-D3; PBe11 group A; PBe12:27-30, :83-86. Rows are in [pathb-series.tsv](artifacts/m2-month12-offloop/pathb-series.tsv).

- **The switch (M):** 12 of 16 intervals sit at or above their threshold; the 4 TS intervals do not (low ends jit 0.961 / 0.960, jitless 0.978 / 0.981): **SWITCH DOES NOT FIRE** (PBe12:384-394). Branch N: month 12 is not the M2-exit month; month 13 is the pre-registered fallback exit month, not a forecast (DL M12-PRE-SWITCH, M12-PRE-MONTH13).
- **Rule 100 (M):** jit -0.24% / +0.53%, jitless +1.34% / +0.81% against PBe11: **NO FALL** (PBe12:395-400).
- **Rule 101 (M):** no merged month-12 set, so x = 1 and t = ln y / ln x is not defined. Line (2) of DL M12-PRE-MONTH13 reads below (PBe12:401-405).
- **The switch read (M):** `git diff --stat m11-p3 m12-p1 -- crates/ep9_vm crates/ep9_bytecode crates/ep9_frontend/src/bytecodegen crates/ep9_heap Cargo.lock .cargo rust-toolchain.toml` lists 5 files, 661 insertions, 84 deletions (`Cargo.lock`, `bytecodegen/func.rs` +6, `bytecodegen/stack_tests.rs`, `bytecodegen/tests.rs`, `ep9_vm/src/tests/star_dst.rs`): not empty, so PBe12 is a switch read (PBe12:381; the command was run again for this report).
- The same-round t(jitless)/t(jit) TS pair: 0.982 [0.968-0.995] / 0.997 [0.981-1.014] (PBe12:363; PBe11 0.997 / 0.995). Box 1 is below 0.985, box 2 is not, so the F2 trigger ("below 0.985 on both boxes", P17 §4.4) does not fire (D).

### 3.2 The code between the 2 end reads

| Part | Change `m11-p3` -> `m12-p1` | Run-time effect | Source |
|---|---|---|---|
| runtime VM and heap (`crates/ep9_vm/src` outside tests, `crates/ep9_heap`) | none | none | `git diff --stat m11-p3 m12-p1 -- crates/ep9_vm/src crates/ep9_heap/src`: `tests/star_dst.rs` only |
| bytecode generator | `func.rs` +6: a stack backstop at each depth step (compile time only) | none at run time; the corpus digest is equal (bytecode byte-equal) | `crates/ep9_frontend/src/bytecodegen/func.rs`; DL M12-P1-W58 (A2) |
| front end | layer 1 (§5) and STACK-FORMS | parse only: +0.455% of front-end instructions on Octane x20 for layer 1 alone (base: the STACK-FORMS code); STACK-FORMS not read alone | L1S; DL M12-P1-LEDGER |
| `Cargo.lock` | the DEPS-BUMP-2 lock moves (oxc_index, hashbrown, smallvec, cc, pin-project-lite; m11-p3 -> 2c5002c8); oxc_parser from the pinned checkout | not measured alone: no whole-run Ir read of 2c5002c8 against `m11-p3` is cited here; the bench runs of the oxc 0.159.0 side head a540dd20 (not 2c5002c8) read every row within +0.003% | DL §22 intro; DL DEPS-BUMP-2, DEPS-BUMP-2-OXC-HOLD; DL M12-L1-START |
| whole-run instructions, BENCH-SET 19 rows | largest cell +0.002% at `m12-p0` (base 4d52dd0d = the 2c5002c8 code) and +0.005% at `m12-p1` (base `m12-p0`) | at most about +0.007% in sum from 2c5002c8 to `m12-p1` (D) | DL M12-P0-LEDGER (run 45434824785; base: OL M6-ORCH-1507), M12-P1-LEDGER (run 45535516052) |
| whole-run instructions, octane-typescript | -0.001% to +0.001% at `m12-p0`; +0.001% (linux-x64 jit and jitless) at `m12-p1` | about 0 | DL M12-P0-LEDGER (run 45455189674); OL M6-ORCH-1538 (run 45535526157) |

### 3.3 The read-to-read move on nearly the same VM code

R18S decision R18S-D7 registered this use before the month-12 end data: "the move per box against PBe is a read-to-read estimate on (nearly) the same VM code". PBe12 prints it as information only; no verdict uses it (PBe12:406-409).

| Item | Build | Move PB10 -> PBe11 (3 merges) | Move PBe11 -> PBe12 (no VM lever) | Both boxes the same sign |
|---|---|---|---|---|
| 1 | jit | x1.111 / x1.094 | x1.008 / x1.012 | yes |
| 1 | jitless | x1.075 / x1.073 | x1.014 / x1.015 | yes |
| 2 | jit | x1.091 / x1.103 | x1.028 / x1.022 | yes |
| 2 | jitless | x1.068 / x1.065 | x1.025 / x1.031 | yes |
| 3 | jit | x1.100 / x1.101 | **x0.998 / x1.005** | no |
| 3 | jitless | x1.032 / x1.040 | **x1.013 / x1.008** | yes |
| 6 | jit | x1.007 / x1.014 | x1.049 / x1.047 | yes |
| 6 | jitless | x1.020 / x0.992 | x1.049 / x1.039 | yes |

Sources: §3.1 values (D; the TS moves are the unrounded values of PBe12:408-409). The gross ep9 TS time rose from PBe11 to PBe12 on both boxes: jit x1.114 / x1.148, jitless x1.099 / x1.155 (PBe11:22-23, PBe12:22-23): the end-12 boxes ran slower.

**Finding (M, D), with its limits:**
- On the TS cells the same-code move is -0.24% to +1.34% (the 4 per-box values). On all 16 per-box values it is -0.2% to +4.9%: 15 of the 16 moved up, and 7 of the 8 cells moved up on both boxes (only TS jit box 1 moved down).
- The code part of the move, in instructions, is at most about +0.007% from the 2c5002c8 code to `m12-p1` (§3.2); the DEPS-BUMP-2 lock step (`m11-p3` -> 2c5002c8) has no whole-run read of its own tree here. So **the move is most probably not an instruction change (I)**. It is a read draw (box state, comparator state), a placement effect of the non-handler code that moved, or both. The data cannot split these (I). Note that splay (item 6) is a GC-heavy row, and splay and octane-splay are BENCH-SET rows: the worst of the 19 rows moved +0.002% (M12-P0) and +0.005% (M12-P1) (DL M12-P0-LEDGER, M12-P1-LEDGER).
- The same-code move of the TS jitless cell (+0.8% / +1.3%) is as large as the month-11 "jitless did not rise" gap (R23 §4.3: x0.998 / x1.007 against an expected x1.012). This supports R23 candidate O1 (the move is inside the read-to-read noise) (I).
- R18S risk K1 forecast "1.5-3 points by box draw" for the end read. The TS cells moved -0.2 to +1.3 points: under that range; items 2 and 6 moved +2.2% to +4.9% (2.6 to 5.5 points).

**What it means for a cell that straddles 1.0 (I; a finding, not a rule change).** The jitless box-1 cell reads 0.985 [0.978-1.001]: its median is under 1.0 and its interval holds 1.0. The switch needs the low end at 1.000 or more, so this cell needs x1.022 at its low end (D). The largest same-code TS move seen is 1.3 points, and the 2 reads moved most cells in one direction. So a future switch read whose TS low ends pass by less than about 1.3 points cannot be told from a read draw with these 2 reads. M12-PRE-SWITCH already says that a straddling cell is not a pass and is not read again with more rounds; this report changes no line. Limits: 2 reads, 2 boxes each, 31 rounds; not a paired A/A in one session; one box type.

---

## 4. The ranked off-loop lever list (an estimate table) and the open questions

### 4.1 The list after TD-3 (R24B; every gain is an estimate)

Rank rule of R24B: the expected gate gain per agent-day; GC-BUDGET first because line G opened it. "Gate" = A/B x a transfer of 0.2-0.8 (rule 101; no off-loop transfer read exists).

| # | Lever | Mechanism | A/B low / center / high (E) | Gate (E) | Agent-days (E) | G8 | Basis | After month 12 |
|---|---|---|---|---|---|---|---|---|
| 1 | GC-BUDGET | fewer minor GCs: a larger S1 budget (the registered arms; the first budget stays 8 MiB, DL M12-P1-GC-PREREG) | x1.001 / x1.003 / x1.010 | x1.000-1.008 | 2.0-2.5 | 0 / +1 | 0.40% main-thread GC per GC (D); a marginal GC costs less because promotion work stays (I) | NOT RUN; carried. R18S re-estimates x1.007 / x1.012 / x1.017 from Mac counts (E): the 2 estimates conflict (O, Q3) |
| 2 | sweep share | every page of a GC goes to the sweeper; `SWEEP_START_PAGES` 16 -> 1-4 | x1.001 / x1.003 / x1.006 | x1.000-1.005 | 1.0-1.5 | 0 | sweeper comm 0.66% < 0.8 x `sweep_page_with` 1.19% (TD3:236, :87): the main thread still sweeps (D) | not in the month-12 pick (line G < 6%) |
| 3 | helper policy on 2 CPUs | start the parallel drain earlier (`START_AFTER` 64 K -> 8-16 K); a shorter idle wait | x1.000 / x1.002 / x1.006 | x1.000-1.005 | 1.0-1.5 | 0 | today's helpers take 2.84 points of JS-thread GC and save x1.022 of wall (TD3:247, :249, :29) | same |
| 4 | allocation cuts (runtime form) | fewer engine-made temporary cells on TS hot paths | x1.000 / x1.002 / x1.008 | x1.000-1.006 | 2.5-4.5 | 0 (runtime); +1 (layout) | `alloc_slow` 0.68% (TD3:98); the allocated bytes are not read | open: needs a site count |
| 5 | S2b ahead of M5 (a plan change) | concurrent marking on the helper while JS runs | x1.000 / x1.006 / x1.020 | x1.000-1.016 | 15-30 (G) | H11 core | the excess it can take is about 0.78 G = 3.2% (§1.2) | out of M2 by the plan (`docs/plan/09-milestones.md:138-139`) |
| 6 | lazy compile (L1) or parse off the main thread | compile only what runs, or parse while JS runs | x1.000 / x1.002 / x1.005 | x1.000-1.004 | 3-5 | 0 / +1 | `ep9-frontend` 0.50% of samples; h0 task-clock / wall 0.996 (TD3:236, :23) | open |
| 7 | stub cache across minor GCs | no refill misses after each GC | x1.000 / x1.001 / x1.003 | x1.000-1.002 | 1.0-1.5 | +1 | not measured | open |
| - | survival policy | grow the budget with survival | dropped | - | - | - | its falsifier fired: line G 4.43% < 6% | dropped |

```
 center A/B, G8-0 items 1-3 (E, D):   1.003 x 1.003 x 1.002 = x1.008     gate x1.002-1.006
 + item 4 center                                              = x1.010     gate x1.002-1.008
 + item 5 center (S2b, a plan change)                         = x1.016     gate x1.003-1.013
 need at the PBe12 low ends (D)                                 jitless x1.019-1.022, jit x1.041-1.042
```

- In the center case no G8-0 set reaches the need. Only S2b has a high end (x1.020 A/B) near the jitless need, and S2b is M5 work (E).
- The PBe12 need is smaller than the PBe11 need only by the read-to-read move of §3.3 (I). An estimate that uses the smaller need is not a basis for a merge or a date.

### 4.2 Open questions for month 13

| # | Question | What would settle it | Cost (E) |
|---|---|---|---|
| Q1 | What is node's main-thread GC share? (§1.2 uses an estimate) | a per-thread split of node's `v8::internal` heap symbols on comm `MainThread`; or a `node --jitless --single-threaded-gc` arm (FD:3461), the mirror of the ep9 h0 arm | inside the next TD box |
| Q2 | What is the major / minor split of ep9 GC time on TS? | a Mac `--gc-stats` read by kind (0 box) | 0 box |
| Q3 | How many TS GCs does GC-BUDGET arm B1 remove on x86, and which estimate holds (R24B x1.003 or R18S x1.012)? | the x86 count run of R18S-D2 | in the GC-BUDGET box set |
| Q4 | Why did items 2 and 6 move +2.2% to +4.9% on both boxes on the same VM code? | a 2-box Path B A/A of one tree in one session; a comparator-only check (node / QuickJS gross time per read) | about 1.75-2.10 box-h (R23 §8: a 2-box Path B) |
| Q5 | Why do the TD-3 boxes read TS near 1.0 while the Path B boxes read 0.97-0.99? | a TD-form read on a Path B box, or the Path B harness on a TD box | in the next TD set |
| Q6 | What young-generation size does node reach on TS? | `node --jitless --trace-gc` on the Mac | 0 box |
| Q7 | Do the L1 routes hold at the next oxc pin (25 sites at 0.160.0)? | DEPS-BUMP-3 with the re-port; routes OXC-VISIT-PROOF-SOURCE, OXC-SITES-LINT-GAPS, FUZZ-SMALL-STACK | in DEPS-BUMP-3 |

---

## 5. The L1 parser stack check (layer 1; merged at `m12-p1`)

### 5.1 Why it was built in month 12

| Fact | Value | Source |
|---|---|---|
| the hole hunt (release, macOS arm64, worker 98,097,152 B) | 297 input classes; 103 abort the process with a worker stack overflow; 71 of them valid JavaScript; 18 rule families of the layer-0 pre-scan (13 new); abort sizes 123,281 to 1,043,750 levels; a file of at most 4 MB is enough for 93% of the classes | DL M12-L1-START |
| a layer-0 redesign | designed, attacked and not built: its check found 5 more classes at 0 units per level, a copy cost of O(forks x depth) and valid 300-byte inputs that hit its work budget | same |
| the registered trigger | none fired as written (a counted fuzz run found no depth miss); the early build is an orchestrator call; G8 +2 as a SWAP into the freed WIDE-SET row | DL M12-L1-START, M12-L1-KIND |
| the engine form | V8, JSC, SpiderMonkey and QuickJS compare the real stack pointer with a limit during the parse | R23 §7.5 |

### 5.2 What was built

| Part | What | Cite (at 6cbc3e2d) |
|---|---|---|
| the patch | `0002-parser-recursion-check.patch`: `ParserConfig::CHECKS_RECURSION` (default false: every site compiles away) and `may_recurse(&self)`; **22 sites at 0.158.0** = 20 checks (17 token-driven recursion entries + 3 cover walks) + 2 walks made loops; a sticky stop flag outside the checkpoint, read at the exits of `parse` and `parse_expression`; `ParserReturn::stack_limit_hit` | `third_party/patches/oxc/README.md:33`; `third_party/patches/oxc/0002-parser-recursion-check.patch` |
| the pin and count | pin 74e5e665 (the vcs commit of the crates.io package); 2 patch files, local source patch count 1 of 3 (0001 is packaging) | `third_party/oxc.commit`; README §State; DL M12-L1-KIND |
| the per-thread limit | `limit = sp_entry - (W - RED)` at the thread start; RED = 1 MiB on top of the budget; a thread with no limit parses with `usize::MAX` (fail closed) | `crates/ep9_frontend/src/l1.rs:1-35`, `:85-91`; `crates/ep9_frontend/src/stack.rs:56` |
| the worker size | debug 284,545,728 B, release 99,145,728 B (BUDGET + RED; debug PARSE_PER_UNIT 7,504, release 2,560) | `stack.rs:69`, `:71`, `:89`, `:97` (D) |
| the error | `RangeErrorKind::StackLimit`, "Maximum call stack size exceeded" | `crates/ep9_frontend/src/lib.rs:217`, `:249-253` |
| the budget checks | per regex literal (groups x REGEX_PER_GROUP x 1.5) and once after layer 2 (depth x PASS_PER_LEVEL x 1.5) | `l1.rs:188-196`; `lib.rs:448-450` |
| the gated depth walk | **11 gated points**; each cycle of the oxc visitor graph passes one; the walk goes at most 8 levels past the cap | `crates/ep9_frontend/src/depth.rs:12-15`, `:150-226` |
| the visitor-graph proof | a test reads the generated `visit.rs` of the pinned oxc (263 walk functions), removes the gated nodes and asserts no open cycle; the gate count assert comes after it, so a removed gate names its cycle | `crates/ep9_frontend/src/depth_tests.rs:1167`, `:1278-1305`; OL M6-ORCH-1531 |
| T-HOLES | **321 rows = 117 abort + 201 control + 3 slow**, each in a child process on 4 targets; an abort row must end with a RangeError (a SyntaxError only for invalid text), never a signal | `crates/ep9_frontend/tests/stack_holes.tsv`, `tests/stack_holes.rs:548-571`; DL M12-P1-LEDGER |
| T-SITES | `cargo xtask oxc-sites`: every recursion of the patched checkout passes a site | `tools/xtask/src/oxc_sites.rs:1` |
| the patch suite | a copy of the patch's own tests that ep9 runs (the patched manifest has `autotests = false`) | `crates/ep9_frontend/tests/oxc_recursion_check.rs:296` |
| the ADR rows | H01:114 and H23:242, kind rewrite, G8 +1 each; Approvals H01:182, H23:597 | DL M12-L1-KIND, M12-P1-LEDGER |

Built against the registered row: 321 T-HOLES rows, not 318 (+3 eval rows of the round-2 fix), and 6 pushed heads, not 3. The ledger states the built values; the registered row is not re-worded (DL M12-P1-LEDGER; OL M6-ORCH-1538).

### 5.3 Cost and review

| Read | Value | Mark | Source |
|---|---|---|---|
| parse only, Octane x20, instructions (Mac arm64, release, 5 pairs, medians) | 29,855,263,891 -> 29,991,031,298 = **+0.455%** against the +1.0% gate | M | L1S; DL M12-P1-LEDGER |
| parse only, test262 (52,017 files) | -0.388% (rows noisy, about 3%) | M | L1S |
| the design estimate | at most 1.5% of the oxc parse, 0.57% of the ep9 front end (4 instructions per check, an upper bound) | E | L1D |
| octane-typescript whole run (CI Ir) | +0.001% (linux-x64 jit and jitless) | M | OL M6-ORCH-1538 (run 45535526157) |
| gate CI on b2e29a1a; tag CI on fcac0863 | 45512158187: 67 of 68 (the adr-lint review records, closed by the record rows); 45533359450: 68 of 68, rule 25 max 0.749 | M | DL M12-P1-LEDGER |

| Review step | Head | Result | Source |
|---|---|---|---|
| RV1: 2 Claude review lenses + a verifier | de901e67 | 9 to fix, 1 needs-run, 4 routed, 0 refuted | DL M12-P1-LEDGER |
| the review gap | after RV1 | the RV1 fixes, the speed fix, the site tool (+1,551 lines) and the fork lists had role checks but no review round; found before the merge | OL M6-ORCH-1527 |
| RV2: 2 Claude review lenses + a verifier | 1a45672d | 5 to fix, 11 routed, 0 refuted; **1 HIGH (A2-1)** | DL M12-P1-LEDGER |
| A2-1 | 1a45672d | `eval("x as a" + ".a".repeat(n))` aborted the process at n = 3,000,000 (debug) and 4,000,000 (release): the layer-2 walk had no gated point on the TS qualified-name chain | DL M12-P1-LEDGER; OL M6-ORCH-1531 |
| the fix and the proof | 8ed08541 | the proof test found **5 open cycle groups** (TS type name, import-type qualifier, namespace path, JSX member name, JSX nesting; 9 nodes) where the review found 2 chains; 6 gates added (11 in all) | OL M6-ORCH-1531 |
| the checker re-run | b2e29a1a | 3 abort inputs and 12 more chain forms at 3,000,000 (debug) and 8,000,000 (release) levels: SyntaxError or RangeError, exit 0 | OL M6-ORCH-1531 |
| a second-model review, code slice | - | 1 run, stopped by its service filter after 18 read commands, no result; the third such stop on parser-stack code on 2026-11-24 (2 on the STACK-FORMS code slice) | OL M6-ORCH-1533; DL M12-P0-LEDGER |
| a second-model review, docs and fork-pin slices | 1a45672d | docs 6 findings (4 fixed, 2 routed); fork pin 4 findings (3 routed) | DL M12-P1-LEDGER |
| the second reviewer of the ADR rows | 1a45672d, 27eee0bc | kinds confirmed; 4 class (c) items, 0 fix first | DL M12-P1-LEDGER |

Not claimed (DL M12-P1-LEDGER): "no input aborts" is by design and tested for the FORMS rows and the 117 abort and 201 control classes; it is not proven for every input. The site tool has 5 known false-pass forms (route OXC-SITES-LINT-GAPS). The regex parser and the walks after layer 2 stay bounded by a measured budget on the 4 tier-1 targets only (DL M12-P1-W58).

---

## 6. Process lessons of month 12

| Lesson | Evidence | Number | Source |
|---|---|---|---|
| every code range needs a review round before the merge | M12-P0: 5 code ranges had no review round (logged after the merge, before the tag); L1: the gap was found before the merge, and RV2 then found the HIGH A2-1 | 1 HIGH defect found by the late round | OL M6-ORCH-1515, -1527; DL M12-P0-LEDGER |
| a proof that enumerates the structure finds more than a reader | the visitor-graph proof against review RV2 | 5 open cycle groups against 2 chains | OL M6-ORCH-1531 |
| a second-model review can be blocked by its service filter on parser-stack code | 3 stops on 2026-11-24 (2 STACK-FORMS, 1 L1); fallback: 2 Claude review lenses + a verifier, and the second model on the docs and pin slices only | 0 second-model results on that code | OL M6-ORCH-1515, -1533 |
| a pre-registration draft checked at an earlier head needs its commands run again at the tag | one "no diff" claim of the health-run row was false at `m12-p1`: a comment line of `scripts/sync-lib.sh` (c99a1ee0) | 1 false claim, fixed before the row went public | OL M6-ORCH-1540 |
| a registered row is never re-worded after its data | L1 built 321 rows and 6 heads against the registered 318 and 3: the ledger states it; the GC-BUDGET own-value guard row was rejected as a moved goal post | 0 re-worded rows | OL M6-ORCH-1538; DL M12-P1-GC-NOT-RUN |
| the generic guard line is the month's limit, not the levers | false GUARD-FAIL 46-68% (10 cells) / 59-81% (14 cells); WIDE-SET closed with all 6 MERGE lines passed | 2 of 3 lever sets lost on the instrument | DL M12-P2-WIDE-VERDICT, M12-P1-GC-NOT-RUN |
| a VOID-for-a-line rerun is read in full | TD-3 box 1 lost line N; box 2 read every line; the box-1 M PASS (2.271%) was not kept | +0.47 box-h | DL M12-PRE-TD3-READ |

| Box use by kind (before the health run) | box-h | Share | Source |
|---|---|---|---|
| cause read: TD-3 (2 boxes, 26 + 28 min) | 0.91 | 17% | usage log BOX rows; DL M12-PRE-TD3-READ |
| lever A/B: WIDE-SET (2 boxes, 85 + 72 min) | 2.62 | 48% | DL M12-P2-WIDE-VERDICT |
| Path B: the end read (2 boxes, 57 + 57 min) | 1.90 | 35% | usage log BOX rows |
| fuzz (the health run is registered, not read here) | 0 | 0% | DL M12-FUZZ-H (est. 1.08) |
| **sum** | **5.43 of the 15.9 cap** (325 box-min = 5.42 h) | | D |

The planned box rows for month 12 were 4.98-7.00 box-h for the rows that always run plus WIDE-SET (R18P). The month used less because 2 lever sets did not run (DL M12-P1-GC-NOT-RUN, M12-P3-PICK). Tokens by phase (DL ledgers): M12-PRE 5,380,624; M12-P0 7,199,336; M12-P1 11,443,100; a shared M12-P0+P1 row 424,590.

---

## 7. R23 addendum: its open questions and candidates after month 12

| R23 item | What R23 said | After month 12 | Status | Evidence |
|---|---|---|---|---|
| Q1 / O1 | the jitless cell did not rise after STAR-DST; O1: a 1% move may be read-to-read noise | the same VM code moved the TS jitless cell +1.34% / +0.81% between 2 end reads | O1 supported, not proven (no paired A/A) | §3.3 |
| O2 | the box state differs between reads | the end-12 boxes ran TS 10-15% slower in gross time than the end-11 boxes, and the ratio still rose | open | PBe11:22-23; PBe12:22-23 |
| O3, O5 | the A/B binary is not the shipped binary; other changed files | no two-binary Path B A/B ran | open | - |
| O4 | STAR-DST costs more on another box | TD-3 gives per-symbol cycles at 2c5002c8, but no STAR-DST cost cell | open | TD3:71-100 |
| Q2 | where are the cycles at `m11-p3`? | TD-3 at 2c5002c8: all-thread cycles ep9 25.135 G against node 25.355 G; Wide +0.269 G; named-load backend-stall slots +3.506 G | answered (1 box read, 1 band box) | TD3:21, :24, :243, :252 |
| Q3 | GC on the main thread against node's helper work | ep9 4.43% (1.11 G) on the JS thread; node `V8Worker` 4.82%; node main-thread GC not measured | partly answered | §1.2 |
| Q4 | the A/B-to-Path B transfer of a removed-overhead lever (the Wide set) | WIDE-SET did not merge; rule 101 transfer NOT DEFINED | open | DL M12-P2-WIDE-VERDICT; PBe12:404 |
| Q5 | item 3 on Zen 3 (EPYC 7763) | not read in this report | open | - |
| Q6 | do the A/B and Path B harness forms differ (R22 Q1)? | the Q1 arm: the Path B form moves ep9 and node alike (0.3-0.6% apart) | answered for the harness form; the box draw stays | DL M12-PRE-TD3-READ |
| finding 6 | the loop is near node in cycles (E) | all-thread cycles ep9 0.9% under node at 2c5002c8 | confirmed (M, 2 boxes) | TD3:21, :24 |
| §5.2 | node gets about 2.3% of wall off its main thread (E) | node CPU per wall second / ep9 = 1.0136; `V8Worker` 4.82% of node samples | refined | §1.1, §1.3 |
| §5.2 | GC-like symbols ep9 7.8%, all threads (E) | `ep9_heap` 6.81% (box 1 8.26%) on all threads | refined (M) | TD3:247 |
| §6.1 Wide set | x1.007 / x1.013 / x1.020 | x1.023 measured; GUARD-FAIL | gain confirmed above the high end; not merged | DL M12-P2-WIDE-VERDICT |
| §6.1 MH rider | x1.000 / x1.002 / x1.004 | line M not read; MH closed for month 12 | open | DL M12-PRE-TD3-READ |
| §6.1 GC-BUDGET | x1.000 / x1.005 / x1.015, only if TD-3 keeps it | TD-3 kept it (line G PASS); NOT RUN by a decision row | open | DL M12-P1-GC-NOT-RUN |
| §6.1 NL-1 stage 1 | x1.000 / x1.002 / x1.010 | priced x1.004-1.005 against the floor x1.015 | not run | DL M12-P3-PICK |
| §6.1 product | x1.022 (A/B) | x1.0000 merged | refuted as a month result (no merge) | §2 |
| §7.6 option (e) | a stack-pointer check in the parse, a month-13 design | built and merged in month 12 | done (earlier than R23 proposed) | §5 |

---

## 8. Verified, not verified, limits

### 8.1 Verified

- Every TD-3 value of §1 against the 2 box summaries (TD3, TD3-1) and DL M12-PRE-TD3-READ; the derived rows recomputed (main-thread cycles, 1.11 G, 0.78 G, 0.47 G, 0.40% per GC, 1.0136, the need rows).
- Every V8 cite of §1.4 at tag `13.6.233.17` (b0a55a7d), read with `git show` of the tag, not a checkout.
- The lever rows of §2 against DL M12-P2-WIDE-VERDICT, M12-P1-GC-NOT-RUN, M12-P3-PICK and the WIDE-SET pooled read text (WR:18-25, :166, :174).
- The Path B values of §3.1 against PBe11, PBe12 and DL M10-END-D3; the switch, rule-100 and rule-101 lines; the switch-read diff command run again at `m12-p1`; the diff of the runtime VM and heap paths.
- The L1 code cites of §5.2 at 6cbc3e2d; the 321 T-HOLES rows; the speed medians from the 5 base and 5 head rows (ratio 1.00455).
- The month-12 box rows of the usage log: 6 rows, 0.91 + 2.62 + 1.90 = 5.43 box-h.

### 8.2 Not verified

- Node's main-thread GC share (an estimate, 0.5-2.2%); the excess of §1.2 rests on it.
- The node v24.22.0 cites (pool size 4, the delayed-task thread): no checkout; node's `-node.57` V8 patches.
- The cause of the read-to-read move of §3.3 (read draw against placement).
- The whole-run instructions of the DEPS-BUMP-2 lock step (`m11-p3` -> 2c5002c8): no read of that tree is cited.
- The cause of the box-1 to box-2 drift of TD-3 line G (5.21% -> 4.43%).
- The estimates of §4.1 (R24B) and the conflicting R18S estimate for GC-BUDGET.
- The review counts of §5.3 beyond the DL ledger row and the OL rows (the review files were read; the per-item fixing commits were not re-checked).
- Item 3 on the EPYC 7763.

### 8.3 Limits

- One box type (Cloudflare standard-4, EPYC 9684X, SMT on, 2 CPUs pinned); no gate host.
- TD-3 is 11 rounds and 3 records per arm on each of 2 boxes; 1 `--gc-stats` run per arm. The TD-3 boxes read the TS cell near 1.0, not at the gate value.
- The 2 end Path B reads are not paired with each other; each is 31 rounds per box.
- The L1 speed rows are Mac arm64 only; x86-64 parse cost was read only as whole-run CI instructions.
- TS is the decision workload; the other rows are guards.

## Raw numbers

[artifacts/m2-month12-offloop/](artifacts/m2-month12-offloop/README.md) holds `findings.json` (one object per finding, with its status, sources and numbers) and the tables behind §1, §2, §3 and §5 as TSV files.
