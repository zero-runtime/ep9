# Allocation cost and SIMD levers (month 13): the 7 allocation levers, the SIMD sites, the sum against the gate need, and the K0, K1, K2 and SHAPE-LOOKUP counts

## Run context

| Item | Value |
|---|---|
| Dates | 2026-11-25: the lever design and its check, the K1 regroup, then the K0, K2 and SHAPE-LOOKUP count runs and their check. This record: 2026-11-25 |
| Round | The month-13 ALLOC route. The owner direction (OL M6-ORCH-1550, -1551): the speed levers combine instruction cuts, allocation cost and SIMD on measured hot loops. 5 read-only mapper notes (A-E: ep9 paths, measured facts, V8 allocation, other engines, SIMD sites) fed one design (FD) with an adversarial check. The orchestrator regrouped the TD-3 cycle tables (K1). One counts role (slot A) ran K0, K2 and SHAPE-LOOKUP on a Mac; a read-only checker read them (CC) |
| What was known | The month-12 end Path B on `m12-p1` (fcac0863): TS medians jit 0.968 / 0.980, jitless 0.985 / 0.988; the low ends give the need jit x1.041 / x1.042, jitless x1.022 / x1.019 ([R25](25-m2-month13-plan-research.md) §1.1). Plan v18 §4.4 (the LEVER-20 pick: est. >= x1.015) and §4.5 (the counts JIT-DELTA and SHAPE-LOOKUP-COUNT) |
| ep9 trees | The counts ran at 5c3e1e11 (= origin `m2/m13`; the worktree was clean after the runs, CC (c)). FD reads the same tree. TD-3 (K1): tree 2c5002c8, box 2. The old `ic-stats` read (ICS): 0a741e5d (2026-11-11) |
| Boxes | None. K0, K2 and SHAPE-LOOKUP ran on the Mac (aarch64; the counts do not depend on the CPU). K1 reads the TD-3 box 2 files that already exist (box ep9-x86-m12-td3-2) |
| Method | Read and count only. **M** = measured (a count run, a box file, a source read). **D** = derived (arithmetic from cited numbers). **I** = inferred. **E** = estimate (basis named). **G** = guess. **O** = open. An A/B gain is a forecast, not a result: no lever in this record has a box read |
| Limits | One workload (octane-typescript). Mac counts are used with x86 cycle shares. The unit prices (cycles per grow, per byte, per dead read) are guesses inside derived bounds (FD §3). The L1 byte fall is a simulation. No wall read of any lever |

Short forms:
- FD = the lever design ([artifacts/m2-month13-alloc-simd/f-design.md](artifacts/m2-month13-alloc-simd/f-design.md)); FD:n = its line n. K1R, K0, K2, SL, CC = the files `k1-regroup.md`, `k0.md`, `k2.md`, `shape-lookup.md`, `counts-check.md` of [the artifacts](artifacts/m2-month13-alloc-simd/README.md); `K2:n` = line n.
- K0-K6 = the count runs of FD §7 (FD:244-256). C1-C5 = the census runs of K2 §0.
- P18 = plan v18 (not public at this record; P18:n = its line n). DL = [docs/decisions-log.md](../decisions-log.md), by row id. OL = the orchestrator log, by row id M6-ORCH-n (not in the repo). R23, R24, R25 = [23](23-m2-month11-levers.md), [24](24-m2-month12-offloop.md), [25](25-m2-month13-plan-research.md).
- Ratios: an A/B gain x1.0nn = t(base) / t(lever) (1.0% less time = x1.010). A gate value of an A/B gain x is x^t, with the Path B transfer t = 0.2-0.8 (FD:84). "Floor" = the LEVER-20 pick line est. >= x1.015 (P18:253).
- H0 = `EP9_GC_HELPERS=0`; HD = the default GC helpers. "Slack" = ESCAPE_SLACK, the extra in-object slots of L1 (FD:97).

---

## 0. Verdict

```
 item                                           before the counts (FD)          after the counts (K0, K1, K2, SL)              read
 L1 CTOR-FIT   (constructor chain capacity)     x1.004 / 1.009 / 1.020          x1.005 / 1.010 / 1.018; bytes -12.0% (slack 1)  ALIVE; count parts of the pick line MET (slack 1); wall not read
 L2 TABLE-MODE (dictionary at V8's limit)       x0.995 / 1.003 / 1.012          7,544 keyed SHAPEs; 360,310 for-in keys        KILLED
 L3 SWEEP-LEAN (no dead-cell header read)       x1.0005 / 1.0015 / 1.0035       3.406 M freed cells; sweep not split by thread  ALIVE; kill line not read
 L4 LIST-FIT   (new Array() hit; one copy)      x1.000 / 1.002 / 1.005          538,940 new Array(); 288,455 growths            (a) ALIVE, (b) KILLED
 L5 ALLOC-ENTRY (constant-size entry)           x1.000 / 1.000 / 1.002          6.49 M cells (line 8 M)                         KILLED for M2
 L6 SRC-HASH   (SHA bodies, SIMD form)          x1.000 / 1.002 / 1.003          K1 sha256 0.37% / 0.35%                         ALIVE by K1; waited path not read
 L7 FAULT-AHEAD (pages filled off-thread)       x1.000 / 1.000 / 1.004          fault cost not measured                         open
 SHAPE-LOOKUP-CACHE (global (shape, key))       not priced                      x1.006 / 1.007 / 1.008; bound x1.010            under the floor: a set member only
 jit-only allocation (K5 rider)                 -                               jit - jitless bytes -0.002%                     none
```

1. **The 7 allocation levers do not close the jit cell (D).** Together they are about x1.016 A/B at the center (jit x1.017). At the gate that is x1.003 / x1.010 / x1.013 for t = 0.2 / 0.6 / 0.8. With GC-BUDGET and WIDE-FIX at their centers, the jitless cells pass from t = 0.51, and the jit cells fail at every t <= 1 (FD:45, :232-238; §3).
2. **SIMD is a rider, not a gate lever (D).** The SIMD that the plan builds gives about x1.000-1.0025; all measured SIMD rows give about x1.003-1.0045. No measured loop of 0.61% or more outside GC and allocation exists (FD:199-211; §2).
3. **The counts kill 3 of the 7 levers or parts, and leave L1 as the only lever near the floor (M, D).** L2, L4 (b) and L5 fall under their kill lines. L1's re-estimate on the counts is x1.010 at the center, still under the floor (K2:142-155; §6).
4. **L1 meets the count parts of its pick line only at ESCAPE_SLACK 1 (D).** Net bytes fall 12.0% (slack 1) against 9.6% at slack 0, the design default; covered growths 1.395 M >= 1.2 M. The wall part (Mac preview >= x1.015 on the jit build) is not read: K3 decides (K2:103-113, :147; CC F5, F6).
5. **There is no jit-only allocation cut (M).** The jit build allocates -8,107 B = -0.002% against the jitless build, and the cells by kind are equal. The jit gap must come from another place: JIT-DELTA looks for it (K0:81; K2:155).
6. **SHAPE-LOOKUP-CACHE cannot reach the floor alone (D).** A global (shape, key) cache of 1,024 entries removes 0.93 of the 1.01% of cycles in `lookup_with`, about 0.73% net: x1.007; even the bound is x1.010 (SL §4).
7. **The month-13 call holds (E).** L1 is the LEVER-20 pick only at its pick line; the plan word stays "expected NOT RUN" (P18:263); the work worth doing in month 13 is the 0-box counts (FD:242). After the counts the next step for L1 is K3, its probe arm.
8. **The check reads the counts as usable (M).** Every read holds; 6 record faults (F1-F6) are in the notes, not in the reads (CC; §8).

---

## 1. The 7 allocation levers (the design, after its check)

### 1.1 The lever table

All gains are A/B time factors with no box read (FD:22). The jit column is the same as jitless where the table says "same".

| # | Lever | What it does | Removes per TS run | Est. A/B jitless low / center / high | Est. A/B jit | G8 | Size (agent-days) | Source |
|---|---|---|---|---|---|---|---|---|
| L1 | CTOR-FIT | the in-object capacity of `new F` counts the whole constructor chain; a scan in escape mode; a capacity above 8 only | 1.47 M SLOTS cells and grow misses (M); 5-13% of the bytes (D) | x1.004 / **x1.009** / x1.020; x1.003 / 1.008 / 1.018 on the GC-BUDGET base | x1.004 / 1.010 / 1.020 | +1 likely (H17 Slack tracking, layout core (the form)) | 1.0-1.5 (+0.5 probe arm) | FD:28, :89-114, :292, :298 |
| L2 | TABLE-MODE | a keyed-add object goes to dictionary mode at V8's limit; for-in with an exact key array and no has-test | per new key: 1 SHAPE + descriptor work + SLOTS doubling; per for-in: 2 Vecs + key arrays + has-tests | x0.995 / x1.003 / x1.012 (G; can lose: dictionary keyed loads are SLOW) | same | +1 (a new OM-10 trigger) | 2.0-3.0 | FD:29, :116-130, :296 |
| L3 | SWEEP-LEAN | no header read of a dead cell; then one SIMD pass over the 2 bitmaps | 3.406 M dead-cell header reads (M) | x1.0005 / x1.0015 / x1.0035 | x1.0005 / 1.002 / 1.005 | 0 (H09 fill) | 1.0 (+0.5 SIMD kernel) | FD:30, :132-146 |
| L4 | LIST-FIT | `new Array()` on the construct hit path; growth with one copy and no double write | 1 host call per `new Array()`; 1 HOLE fill per growth | x1.000 / x1.002 / x1.005 (G) | same | 0 | 1.0-1.5 | FD:31, :148-161 |
| L5 | ALLOC-ENTRY | a constant-size allocation entry with a ready word0; frame publish on the miss only | about 15-25 instructions per cell (I) | x1.000 / x1.000 / x1.002 | same | 0 (H09 fill) | 1.5-2.0 | FD:32, :163-173, :294 |
| L6 | SRC-HASH | SHA-NI and ARMv8 SHA2 bodies behind the shared dispatch (the SIMD form only) | 5.0 MB hashed per run (2 hashes per compile) | x1.000 / x1.002 / x1.003 | same | 0 (SIMD form); the lazy form needs a G8 row (H02) | 0.5-1.0 (+0.5 dispatch module) | FD:33, :175-185, :297 |
| L7 | FAULT-AHEAD | the sweeper thread commits and fills the next fresh pages | page faults and first-touch fills on the JS thread | x1.000 / x1.000 / x1.004 (G) | same | 0 | 1.5-2.5 | FD:34, :187-195, :295 |

```
 where each lever acts (TS e-jitless, TD-3 box 2; shares of 25.135 G cycles; FD:36-43)
 ┌ handlers 71.6% ── L1 (grow misses leave the store path; fields 9+ move into the object)
 ├ GC main thread 4.43% ── L1 (cells, bytes) ── L3 (dead reads) ── L2, L4 (bytes)
 ├ alloc_slow 0.68% ── L1, L2, L4 (fewer bytes to fill) ── L7 (fill and faults off-thread)
 ├ runtime slow paths (each < 0.61%) ── L1, L2, L4
 └ rest 15.15% (no class split) ── L6 (sha256) ── K1 prices the rest
```

### 1.2 How the design prices a lever (the R23 lesson)

R23 measured that a removed dead dispatch saved about 5.55 cycles, a fused dispatch about 1.04, and fewer ALU instructions inside a handler about 0. So FD prices **removed units**, not instruction shares (FD:70-85):

| Unit | Price | Mark |
|---|---|---|
| a removed cold-path run with an allocation (a grow) | 90-180 cycles | G, bound: each symbol < 0.153 G |
| a removed allocated byte | 2.64 cycles on the main thread, used at 0.4 / 0.7 / 1.0 of that | D (1.11 G / 420.6 MB) |
| a removed fill byte | 0.41 cycles | D |
| a removed dead-cell header read | 10 / 25 / 60 cycles | G |
| ALU instructions cut inside a hot handler | 0 | R23 |

### 1.3 L1 in more detail (the only lever near the floor)

- **Cause (M, code).** A constructor whose `this` leaves the frame gets no count and 8 in-object slots. 68 TS constructors start with `_super.call(this, ...)`, so every AST node has 8 slots, and fields 9+ go to a SLOTS cell that grows 4, 8, 16, 32 on the store miss path (FD:93).
- **Mechanism.** The scan gets an escape mode; a new walk of the [[Prototype]] chain adds the names of each non-derived bytecode constructor; it stops at a host function, a dictionary prototype, an accessor or 8 levels; the capacity changes only if the chain count is above 8. Each object of one constructor still starts from one root shape (FD:94-95).
- **Why not V8's rule (own + 8).** For `BinaryExpression` (own 2, total 12) it gives 10 in-object and still 1 SLOTS cell per node (FD:96).
- **Pick line (corrected by the check, FD:293).** A Mac wall preview >= x1.015 on the jit build, AND bytes fall >= 10%, AND covered grows >= 1.2 M. The TS x64 Ir fall is a support row only.

---

## 2. SIMD: the sites and the rider size

**The bound (FD:199).** No measured loop of 0.61% or more outside GC and allocation exists.

| Loop (measured symbol) | Share | What SIMD saves | Mark |
|---|---|---|---|
| TLAB `undefined` fill (`alloc_slow`) | 0.68%, IPC 0.58 | <= 0.03%: the loop waits for the line (26 cycles per 64 B line) | D |
| sweep bitmaps (`sweep_page_with`) | 1.19% / 1.64% | 0.01-0.02%; the dead-cell read (L3 (a)) is the cost | D + G |
| source SHA-256 | 0.27% (old build) | 0.23% | M + G |
| `lookup_with` linear scan | 1.01% | 0.15%; a cache removes the call | M + G |
| growth copies | < 0.61% each | through libc after the E2 test leaves the loops (L4) | I |
| sum of all rows | | about 0.3-0.45% = x1.003-1.0045 | D |
| **sum of what the plan builds** (`lookup_with` and the TLAB fill are rejected) | | **about 0-0.25% = x1.000-1.0025** | D |

Source: FD:201-209.

- **The call.** SIMD rides on levers that change the loop (L3, L4, L6, L7). It is not a gate lever. Its gain rests on one site (L6) with an old-build share (FD:211).
- **One dispatch module first (FD:213-222).** `ep9_platform::simd`, 0.5 agent-day: one detection into an `AtomicU8` bitset (x86-64 AVX2, SHA + SSE4.1, AVX-512 VL + VPOPCNTDQ; aarch64 NEON and SHA2; wasm simd128), leaf `#[target_feature]` bodies with no GC point, the scalar body as the fallback, `EP9_SIMD=` to force a body in tests. It joins the 2 tables that exist (`EP9_SIMD_SEARCH`, `EP9_SIMD_JSON`).
- **x86 test.** The SIMD bodies need one x86 Cloudflare box test in the W3-SIMD-BOX form that the orchestrator runs; it is a plan-v19 item (FD:303).
- **K1 adds:** libc `mem*` is 0.00% (e-jitless) / 0.06% (e-jit) of cycles, so a copy kernel has no measured site on TS (K1R:14).

---

## 3. The sum against the gate need, and the month-13 call

### 3.1 The sets (FD:224-230)

| Set | Levers | Center A/B (ln sum) | Mark |
|---|---|---|---|
| ALLOC-SET-1 (anchored on measured counts) | L1 + L3 + L6 | 0.0090 + 0.0015 + 0.0020 - 0.0005 (L1 x L3 overlap) = 0.0120 = **x1.012** | D |
| + the count-gated ones | + L2 + L4 + L7 + L5 | + 0.0030 + 0.0020 + 0 + 0 - 0.0010 (GC overlap) = 0.0160 = **x1.016** (jit x1.017) | G until K1 + K2 |
| low / high | all 7 | about x1.000 / about x1.045 | the high needs every high at once |

### 3.2 Against the need (ln: jit 0.0402 / 0.0411; jitless 0.0218 / 0.0188; FD:232-238)

| Case | Jitless | Jit |
|---|---|---|
| this route alone, center, t = 0.2 / 0.6 / 0.8 | x1.003 / x1.010 / x1.013: 15-17 / 44-51 / 59-68% of the need | x1.003 / x1.010 / x1.014: 8 / 25 / 33% |
| + GC-BUDGET x1.012 + WIDE-FIX x1.017 (jit x1.006), centers, GC overlap -0.002 | ln 0.0428: passes both cells from t = 0.51; at t = 0.6 the cells read 1.003 / 1.007; at t = 0.2 they fail (0.986 / 0.989) | ln 0.0327 < 0.0402: **fails at every t <= 1**; at t = 0.8 the cells read 0.986 / 0.985 |
| what the jit cell still needs | - | about x1.018 more A/B at t = 0.8 from a jit-side lever (0.0402 / 0.8 - 0.0327 = 0.0175) |

- One Path B read is good to about 1 point (P18:59), so a x1.011 gate move is at the noise (FD:240).
- The ALLOC route helps both builds the same. It does not close the gap between the jit and the jitless build (FD:241).

### 3.3 After the counts (D, this record)

The counts remove L2 (+0.0030), L4 (b) and L5 from the second row of §3.1. ALLOC-SET-1 with the K2 re-estimate of L1 (center x1.010) is 0.0100 + 0.0015 + 0.0020 - 0.0005 = 0.0130 = **about x1.013** (D on E). With SHAPE-LOOKUP-CACHE at its center (x1.007), as if independent: about x1.020 (D). Both stay under the jit need. No lever alone reaches the floor.

### 3.4 The month-13 call (FD:242, :305)

- L1 can be the LEVER-20 pick only at its pick line. At the center it is under the floor x1.015, and the plan word is still "expected NOT RUN" (P18:263).
- The work that is worth doing in month 13 is the count runs K0-K3 (0 boxes). They turn L1 from an estimate into a measured Ir, byte and GC-count change, and they price L2-L4.
- What the design would NOT do (FD:258-272): a wider TLAB fill alone; no TLAB pre-fill; `undefined` = 0 (H06 is Frozen); SIMD in the mark loop, the stub-cache compare, the frame fill or `lookup_with` before its cache; VO bits in bulk; V8's run-time slack tracker (measured worse) or V8's "own + 8"; an inline megamorphic-add path; V8's elements growth; mementos, pretenuring, a moving nursery; a box for any count.

---

## 4. K1: the TD-3 cycle tables by class

Source: TD-3 box 2 (ep9-x86-m12-td3-2, tree 2c5002c8), the listed symbols of `rec/<build>.cycles.{1,2,3}.sym.tsv`, the median of 3 runs, % of all cycle samples on all threads; one class per symbol (K1R:3). The patterns are the orchestrator's.

| Class | e-jitless | e-jit | node-jitless |
|---|---|---|---|
| kernel | 1.05 | 1.02 | 0.56 |
| interp handlers (`ep9h__`) | 71.67 | 70.62 | 0.00 |
| GC (`ep9_heap` trace, parallel, sweep, collect, remembered) | 6.15 | 7.18 | 0.08 |
| alloc entry + slots grow | 1.34 | 1.34 | 0.00 |
| shape / props add, for-in | 0.93 | 0.95 | 0.00 |
| `lookup_with` | 1.01 | 1.04 | 0.00 |
| string + atoms | 1.82 | 2.16 | 0.00 |
| `memcpy` / `memset` / `memmove` | 0.00 | 0.06 | 0.15 |
| sha256 / hasher | 0.37 | 0.35 | 0.00 |
| front end / parser | 0.19 | 0.26 | 0.00 |
| interp slow paths + rest of `ep9_vm` | 8.23 | 8.17 | 0.00 |
| node / V8 symbols | 0.10 | 0.05 | 92.04 |
| listed total | 92.81 | 93.16 | 92.83 |

Reads (D):
- **L2 path.** The shape and props add class with for-in is 0.93% / 0.95%. That is above the FD kill line for L2 (< 0.5% kills). K2 killed L2 by its counts, not by this share (§6).
- **L6.** sha256 0.37% / 0.35% is above its kill line (< 0.1%). Whether it runs on the waited path is not read (CC:21).
- **L3.** K1 does not split the GC class by thread, so the L3 kill line (main-thread sweep < 0.3%) is not read (CC:21).
- **jit against jitless.** GC is 1.03 points higher in the e-jit build, and string + atoms 0.34 points higher. K0 shows equal bytes and cells in both builds (§5), so the GC difference is not more allocation (I: the share is of all threads, and the GC helpers are on in this form).

---

## 5. K0: gc-stats and ic-stats at 5c3e1e11

4 release builds (jit, jitless, and each with `ic-stats`); 3 `--gc-stats` runs per build and GC form; 1 `ic-stats` run per build (K0 §1). Every run: rc 0 and the expected output.

| Count | f-design value (ICS, 0a741e5d) | K0 value (5c3e1e11) | Read |
|---|---|---|---|
| `store grow` (N1) | 1,465,706 | **1,467,508** (+0.12%) | holds; the K0 kill line (< 1.0 M) is not met: **L1 ALIVE** |
| megamorphic store work (N4) | 11,353,077 store mega misses | stub-cache store probes 11,200,750 + 152,327 = 11,353,077 | the same work; the label moved |
| load hit + miss | 328,708,540 + 29,285,662 = 357,994,202 | 357,003,892 + 990,310 = 357,994,202 | the same total |
| bytes, freed cells, GCs (H0) | 420,554,288-420,600,112 B; 3,406,019-3,408,591 freed; 10 GCs | jitless 420,569,136-420,600,112 B; jit 420,579,376-420,583,984 B; freed 3,406,021-3,408,509; 10 GCs | holds |

Source: K0 §2-§5; raw `k0-gc.txt`, `k0-ic.jitless.txt`, `k0-ic.jit.txt`.

- **Why the label moved (M, code).** At 5c3e1e11 a stub-cache hit in the handler counts as an IC hit (`ic.rs:2497`, `:2820` at that commit). At 0a741e5d it counted as a megamorphic miss (K0:76).
- **K5 rider (JIT-DELTA, P18:269).** The jit minus jitless bytes (H0 means) = 420,581,424 - 420,589,531 = -8,107 B = **-0.002%**. The kill line is "< 2% more bytes in the jit build: none". The IC counts are equal too. **The jit build has no allocation part to cut on TS** (K0:81).
- **HD form.** With the default helpers the GC count varies (10-13) and the freed cells are 4.02-5.78 M. Use H0 for counts (K0:82-83).

---

## 6. K2: the census and the kill line of each lever

One count build (feature `census`, the `ic-stats` form; the patch is in the artifacts as text and is never applied). Runs C1 (first form), C2-C4 (jitless, final form), C5 (jit, final form; equal to C2 except for 1 string cell and hash-order ties). The final form makes 12 GCs in H0 (the plain build: 10); the counts move by < 0.4% (K2 §0).

### 6.1 Cells and bytes (C2 raw; K2 §1 holds the C1 table)

| Kind | Cells | Bytes | Share of bytes |
|---|---|---|---|
| object | 2,012,062 | 152,642,184 | 36.3% |
| slots | 1,483,021 | 99,213,520 | 23.6% |
| array | 1,125,020 | 45,000,800 | 10.7% |
| elements | 319,842 | 32,268,640 | 7.7% |
| context | 455,597 | 21,909,632 | 5.2% |
| function (closures) | 248,149 | 17,866,728 | 4.2% |
| the other kinds | 848,892 | 51,505,520 | 12.3% |
| **total** | **6,492,583** | **420,407,024** | |

Source: `census-c2.jitless.txt` lines 2-30 (D: the shares and the "other kinds" row). The C1 table in K2 §1 has no raw file (CC F1); C2 agrees with it to within 0.4% on SLOTS cells.

### 6.2 L1: the chain rule on the counts (C2)

- **SLOTS growths:** 1,482,899 in all; 1,481,595 on plain objects. By step: 0 -> 4 922,395; 4 -> 8 475,627; 8 -> 16 66,872; 16 -> 32 9,164; 32 -> 64 8,769; larger 72 (K2:64-75).
- **Top classes:** `Identifier` (272,321 objects, 544,641 SLOTS cells, chain capacity 14), `BinaryExpression` (160,505), `ASTList` (96,630), `CallExpression` (30,985). The chain rule covers all their growths (K2:77-99).
- **Covered by the chain rule (event count):** slack 0: 1,369,767; slack 1: 1,395,702; slack 2: 1,425,582 of 1,481,595 (K2:103).

The net effect (the simulation `sim-l1.py` on the adds per field index; it gives 1,481,582 SLOTS cells today against 1,481,595 counted; K2:105-113):

| Slack | SLOTS cells left | SLOTS cells removed | SLOTS bytes removed | Object bytes added | Net bytes removed | Out-of-line load hits moved in-object |
|---|---|---|---|---|---|---|
| 0 | 442,934 | 1,038,648 | 77,330,528 | 37,062,472 | 40,268,056 = 9.6% | 18,391,566 |
| **1** | **87,473** | **1,394,109** | 94,404,336 | 44,011,488 | **50,392,848 = 12.0%** | 19,853,387 |
| 2 | 61,266 | 1,420,316 | 95,687,296 | 51,894,872 | 43,792,424 = 10.4% | 19,980,151 |

- **ESCAPE_SLACK 1 is the best value** (the design default was 0). At 1, the extra slot holds `Identifier.text` (272,320 objects) and the `container` field of the 4 symbol classes (K2:113).
- **Named-load hits (K2 §4).** In-object 172.36 M (59.5% of field hits); out of line (SLOTS) 117.33 M (40.5%). L1 at slack 1 moves 19.85 M of them in-object: 17% of the out-of-line hits, 5.5% of all named-load hits. On x86-64 the hit path reads the SLOTS word on every field hit, so the gain is the second cache line, not instructions (FD:64).

### 6.3 The kill and pick lines (K2 §5, read by CC (b))

| Lever | Kill line (FD) | Count | Read |
|---|---|---|---|
| L1 CTOR-FIT | covered grows < 1.0 M; byte fall < 5%; TS x64 Ir fall < 0.5% | `store grow` 1,467,508; net cells removed 1.394 M (slack 1); bytes -12.0% (slack 1), -9.6% (slack 0); Ir not read | **ALIVE** |
| L1 pick line | Mac wall >= x1.015 (jit) AND bytes >= 10% AND covered >= 1.2 M | bytes MET at slack 1 only; covered MET; wall NOT READ | **count parts MET at slack 1; K3 decides** |
| L1 re-estimate | - | the FD method with the K2 counts: x1.005 / x1.010 / x1.018; GC-BUDGET base x1.005 / x1.009 / x1.016 | center under the floor |
| L2 TABLE-MODE | (a) SHAPEs from keyed adds < 0.3 M; (b), (c) for-in key visits < 2 M | (a) 7,544; (b), (c) 360,310 | **KILLED** |
| L3 SWEEP-LEAN | main-thread sweep < 0.3% of cycles (K1) | freed cells 3.406-3.409 M confirm the count; K1 does not split the sweep | **ALIVE; kill line not read** |
| L4 LIST-FIT | (a) 0-argument `array_constructor` < 0.15 M; (b) `store_element_slow` growths < 0.3 M | (a) 538,940; (b) 288,455 (79% are the first cell of an empty array) | **(a) ALIVE; (b) KILLED** |
| L5 ALLOC-ENTRY | cells < 8 M per run (for M2) | 6.49 M | **KILLED for M2** |
| L6 SRC-HASH | sha256 < 0.1%, or off the waited path | K1: 0.37% / 0.35% | **ALIVE by K1** |
| L7 FAULT-AHEAD | faults x fault cost < 0.2% (K6) | Mac: 23,344-23,442 page reclaims, 1 page fault, sys 0.04 s of 2.54 s; node --jitless 19,072-19,971 reclaims; the fault cost is not measured | **open** |
| K5 rider | < 2% more bytes in the jit build | -0.002%; cells by kind equal (C5) | **none** |

Other K2 counts (K2 §3): adds to plain objects 16,070,485; SHAPEs made 12,121 (keyed-set cold 7,544); moves to dictionary mode 27,004; `GetKeyedProperty` cold bodies 2,380,650 (500,885 on a dictionary receiver); `for_in_keys` 28,372 calls with 360,310 keys.

---

## 7. SHAPE-LOOKUP-COUNT (plan v18 §4.5)

The census build also counts every `shape::lookup_with` call. A site is the first Rust caller outside the lookup wrappers (`#[track_caller]`). Caches of (ShapeId, key) -> answer are simulated per site (LRU of 1, 2, 4) and globally (direct-mapped, 256 and 1,024 entries); a hit removes the call (SL §1).

| Count | Value | Source |
|---|---|---|
| `lookup_with` calls per TS run | 6,145,451 (C2-C4); 6,180,686 (C1) | SL:16-26 |
| found | 23.5% (C2): 76% of the calls are steps of a prototype-chain walk that do not find the key | SL:21, :51 |
| caller split (C2) | keyed load cold 50.6%; named store miss 21.2%; keyed store cold 15.6%; for-in has-test 4.9%; other 3.9%; named load miss 3.8% | SL:28-37 |
| per-site cache, 1 / 2 / 4 entries | call hit rate 9.9 / 15.6 / 37.7% | SL:53-61 |
| **global cache, 1,024 entries** | **call hit rate 90.2-90.3%; work-weighted 91.7-91.9%** | SL:61 |

**The price (SL §4).** `lookup_with` = 1.01% of TS cycles e-jitless (1.04% e-jit) = 0.254 G, so 41 cycles per call (D). A probe costs 5 / 8 / 10 cycles (G) on every call.

| Case | Removed work | Probe cost | Net | A/B |
|---|---|---|---|---|
| low (global 256, 82%, probe 10) | 0.83% | 0.25% | 0.58% | x1.006 |
| **center (global 1,024, 91.8% work, probe 8)** | **0.93%** | 0.20% | **0.73%** | **x1.007** |
| high (global 1,024, 92.2%, probe 5) | 0.93% | 0.13% | 0.81% | x1.008 |
| bound (100% hits, probe 0) | 1.01% | 0 | 1.01% | x1.010 |

- **PICK LINE NOT MET.** The center is x1.007 = 0.49 of the floor (ln ratio). SHAPE-LOOKUP-CACHE is a set member only (same sign in both builds).
- The simulation does not flush on an in-place shape change or at a GC, so its hit rate is an upper bound (I: 10-12 GCs per run, a small loss).
- 2 smaller parts with no cache: `own_property` and `add_own_property` look up the same (shape, key) twice per keyed add (297 k extra calls); the for-in has-test (301 k) is the killed L2 (c) path.
- **The line-M read tool.** In all 3 TD-3 box 2 records `perf annotate` crashed (rc 139) on `GetNamedProperty`, which holds 9.4-9.7% of each record, so line M was NOT COMPUTED. The fix (samples per address from `perf script` when annotate fails, read by the summarizer, with a self-test) needs a box run: **route LINE-M-READ-FIX** (opus, 0.5 agent-day, rides TD-4). The plan-v18 output "line M readable at TD-4" stays open (SL §5; CC:23).

---

## 8. The count check (CC)

**Verdict: usable.** Every read holds. The faults are in the records, not in the reads (CC:1-2).

| # | Fault | Effect on a read |
|---|---|---|
| F1 | C1 has no raw file (the run was overwritten): the K2 §1 table (6,485,145 cells), SLOTS 1,487,925, `own_add` 277,466, dictionary moves 22,855 and the SL C1 parts have no source | none: C2 raw has the same rows to within 0.4%, and no read uses a C1-only number. This record gives the C2 cell table (§6.1) |
| F2 | K2 §3 "IC add hit 15,801,256" is in no raw file; C2 and C5 show 15,800,221 | none |
| F3 | the census TOTAL is 420.4 MB, but the census build's gc-stats line gives 425.8 MB; no note explains the 1.3% gap (the plain build: 420.6 MB) | on 425.8 MB the slack-1 fall is 11.8%: still >= 10% |
| F4 | "fields added after the constructor returns" is counted as "names outside the chain" | a near form; for ESCAPE_SLACK it is the right measure |
| F5 | the L1 row of K2 §5 gives "covered" as net cells removed (1.394 M); the covered count is 1,369,767 / 1,395,702 / 1,425,582 of 1,481,595 growths on all paths | none |
| F6 | the 12.0% byte fall is a simulation (D); at slack 0, the design default, it is 9.6% and that pick part fails | FD:97 lets K2 pick the slack; K3 must measure it |

CC also confirms: the kill and pick reads quote FD correctly; both builds were read in K0, K2 (C5) and SL; the worktree was clean after the runs and nothing was committed; and the JIT-DELTA dispatch needs no head commit (CC (b)-(d)).

---

## 9. What follows

| Item | Next step | Boxes |
|---|---|---|
| L1 CTOR-FIT | K3: the probe arm on the GitHub bench mode, x64, both builds (TS Ir, bytes, GC count, the 19 BENCH-SET rows) + a Mac wall preview; ESCAPE_SLACK 1; the pick needs the wall >= x1.015 on the jit build | 0 |
| L3, L6 | price on a thread split of the sweep and the hash; ride L1's set in plan v19 with the dispatch module | 0 (the SIMD bodies need one x86 box test later) |
| L4 (a) | plan v19 | 0 |
| L7 | K6: page faults with a measured cost | 0 |
| SHAPE-LOOKUP-CACHE | a set member in plan v19, priced at x1.007 | 0 |
| the jit cell | JIT-DELTA (GitHub bench mode, base = head 5c3e1e11): what the jit build runs more, priced by the removed-unit model | 0 |
| line M | route LINE-M-READ-FIX, rides TD-4 | rides TD-4 |

## 10. Not verified

- Every A/B gain is an estimate with guessed unit prices; no lever has a box or wall read.
- The Mac byte and cell counts are used with x86 cycle shares. The x86 `gc/` files were not read (FD:284).
- The class of an object is `proto.constructor`; 851 objects read as "(none)" (K2:159).
- The chain rule is simulated at the first add or growth of a class, not at its first construct (K2:160).
- The escape-mode scan reads every name stored on `this`, also on paths that do not run; such a name takes a slot (K2:163).
- The K1 class patterns are the orchestrator's, read on mangled names; the tail under the list cutoff (7.2% of cycles) is not split (K1R:3).
- The SHAPE-LOOKUP hit rates are an upper bound (no flush simulated).
- The G8 readings of L1 (+1 likely) and of L3, L5 (H09 fill, 0) can be disputed by a reviewer (FD:281).
