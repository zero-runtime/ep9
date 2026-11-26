# F-DESIGN: combined levers for octane-typescript on x86-64 (instruction cuts + allocation work + SIMD)

Role: LEVER DESIGNER (route ALLOC, month 13). Written 2026-11-25. Read only: no build, no box, no run, no gh.
Owner direction (2026-11-25): "Instruction cuts is really helpful, but should combine with memory allocation optimization" and "And maybe more SIMD".

## 0. Short names and marks

| Name | What |
|---|---|
| S | the orchestrator scratch root (not in the repo; no path is given) |
| T: | the read-only ep9 tree at 5c3e1e11 (= origin m2/m13 head); `T:crates/...` and `T:docs/...` are repo paths at that commit |
| A, B, C, D, E | the 5 mapper notes of the ALLOC route (not in the repo): `a-ep9-paths.md`, `b-measured.md`, `c-v8-alloc.md`, `d-other-alloc.md`, `e-simd-sites.md` (A:17 = line 17 of file a) |
| P18 | plan v18 (not public at this record; P18:n = its line n) |
| TD3 | the TD-3 summary of box 2 (not in the repo; its numbers are in R24) (the read box, tree 2c5002c8; heap and VM code = 5c3e1e11, B:26) |
| ICS | the TS-STORE role `ic-stats` output (not in the repo) (the `ic-stats` build of the TS-STORE role head 0a741e5d, 2026-11-11, Mac a64, one TS run) |
| GCS | 2 `--gc-stats` lines of the month-12 GC-BUDGET role (not in the repo) (Mac, arm 0, helpers 0, `--gc-stats`) |
| HA | the HA-STAGE-1 design (not in the repo) |
| W | the workload `typescript-compiler.js` of `third_party/bench-octane/` (synced by `scripts/sync-bench.sh`) |
| DL | `T:docs/decisions-log.md`; H17 = `T:docs/adr/H17-object-layout.md`; R23 = `T:docs/research/23-m2-month11-levers.md` |

Marks: (M) measured; (D) derived, with the arithmetic; (I) inferred; (G) a guess.
All gains are A/B time factors (x1.010 = 1.0% less time). No box read exists for any lever below.

## 1. The answer

| # | Lever | Combines | Removes per TS run | Est. A/B jitless (low / center / high) | Est. A/B jit | G8 | Size (agent-days) | Month |
|---|---|---|---|---|---|---|---|---|
| L1 | **CTOR-FIT**: the in-object capacity of `new F` counts the whole constructor chain | instructions + allocation + bytes + cache lines | 1.47 M SLOTS cells (M), 1.47 M grow misses (M), 5-13% of the allocated bytes (D) | x1.004 / **x1.009** / x1.020 (D on M counts + G unit costs); x1.003 / 1.008 / 1.018 on the GC-BUDGET base (§10 C7) | the same | **+1 likely** (H17 Slack tracking "layout core (the form)", §10 C1); 0 only if the reviewer agrees | 1.0-1.5 (+0.5 probe arm) | **13: the LEVER-20 pick only if the count K3 reads at its pick line; else the first set of plan v19** |
| L2 | **TABLE-MODE**: a keyed-add object goes to dictionary mode at V8's limit; for-in with an exact key array and no has-test | instructions + allocation | per new table key: 1 SHAPE (64 B) + descriptor work + SLOTS doubling; per for-in: 2 Vecs + 2-3 key arrays + 1 has-test per key (A:18-19) | x0.995 / x1.003 / x1.012 (G; no count; can lose: dictionary keyed loads are SLOW, §10 C5) | the same | +1 (a new OM-10 trigger) | 2.0-3.0 | v19, after the counts K1 + K2 |
| L3 | **SWEEP-LEAN**: no header read of a dead cell; then one SIMD pass over the 2 bitmaps | memory reads + SIMD on a measured symbol | 3.406 M dead-cell header reads (M) | x1.0005 / x1.0015 / x1.0035 (D on M count + G cost) | x1.0005 / x1.002 / x1.005 | 0 (H09 fill) | 1.0 (+0.5 SIMD kernel) | v19 (rides L1's set) |
| L4 | **LIST-FIT**: `new Array()` on the construct hit path; growth with one copy and no double write | instructions + allocation + libc SIMD copy | 1 host call per `new Array()`; 1 full HOLE fill per growth | x1.000 / x1.002 / x1.005 (G; no count) | the same | 0 | 1.0-1.5 | v19, after K1 + K2 |
| L5 | **ALLOC-ENTRY**: a constant-size allocation entry with a ready word0; frame publish on the miss only | instructions per cell; a JIT-ready fast path | about 15-25 instructions per cell (I) x about 5 M cells (D + G) | x1.000 / x1.000 / x1.002 (R23: ALU cuts inside a handler read about 0; §10 C3) | the same | 0 (H09 fill) | 1.5-2.0 | with the T1 baseline design (M3), or v19 if K2 shows > 8 M cells |
| L6 | **SRC-HASH**: a lazy source SHA-256, else SHA-NI / ARMv8 SHA2 bodies behind the shared dispatch | SIMD on a measured loop (the SIMD form only; §10 C6) | 5.0 MB hashed per run: 2 hashes per compile of 2.51 MB (`builder.rs:949`, `verify.rs:167-168`; §10 C6) | x1.000 / x1.002 / x1.003 (M share on an old build + G) | the same | 0 (SIMD form); the lazy form touches Frozen H02 ABI core: a G8 row | 0.5-1.0 (+0.5 dispatch module) | v19 (0 boxes: judge by counters) |
| L7 | **FAULT-AHEAD**: the sweeper thread commits and fills the next fresh pages | work moved off the main thread + SIMD fill | page faults and first-touch fills on the JS thread | x1.000 / x1.000 / x1.004 (G; no count; §10 C4) | the same | 0 | 1.5-2.5 | v19, only after the count K6 |

```
 where each lever acts (TS e-jitless, TD3 box 2; shares of 25.135 G cycles)
 ┌ handlers 71.6% ── L1 (grow misses leave the store path; fields 9+ move into the object)
 ├ GC main thread 4.43% ── L1 (cells -30%, bytes -5..13%) ── L3 (dead reads) ── L2, L4 (bytes)
 ├ alloc_slow 0.68% ── L1, L2, L4 (fewer bytes to fill) ── L7 (fill and faults off-thread)
 ├ runtime slow paths (grow_add, reserve_field_slots, shape add, for-in: each < 0.61%) ── L1, L2, L4
 └ rest 15.15% (no class split) ── L6 (sha256) ── the regroup K1 prices the rest
```

**Honest sum (§6, corrected by the check, §10):** the 7 levers together are about x1.016 A/B at the center (jit x1.017; about x1.000 low, about x1.045 if every high holds). At the gate that is x1.003 / x1.010 / x1.013 for a transfer of 0.2 / 0.6 / 0.8. The need is jitless x1.022 / x1.019 and jit x1.041 / x1.042. With GC-BUDGET and WIDE-FIX both merged at their centers, the jitless cells pass from a transfer of about 0.51. **The jit cells do not pass at any transfer at the centers.** This route does not close the jit cell alone. (Before the check: x1.019, t 0.48.)

## 2. Facts this role read (new, or different from a mapper file)

| # | Fact | Value | Source | Mark |
|---|---|---|---|---|
| N1 | SLOTS growths on the store miss path (`grow_add`) per TS run | **1,465,706** | ICS:7 (`store grow=`); counter at `T:crates/ep9_vm/src/ic.rs:2305` | M (0a741e5d) |
| N2 | The grow count holds at 5c3e1e11 | (corrected, §10 C9) `git diff --stat 0a741e5d HEAD` (the ICS head): slack.rs 8 lines (the `GetNamedPropertyStar` scan arm, `T:crates/ep9_vm/src/slack.rs:173-180`), props.rs 33 (the `reserve_field` front split and the PROPS size form), ic.rs 685, object.rs 236, shape.rs 70, construct_ic.rs 8; a grep of the ic.rs diff finds no change to the grow rule (`wr` -> `wr_keep`, x86 hit forms, docs) | git, read only | I |
| N3 | AST-chain objects per TS run | `ASTSpan` constructor 796,649 runs; `AST` constructor 773,909 runs | ICS:13-17 (megamorphic store sites `ASTSpan@0`, `@1`, `AST@2`, `@3`, `@6`: one miss per run of the site) | M |
| N4 | Megamorphic stores from these 2 base constructors | 2 x 796,649 + 8 x 773,909 = 7,784,570 of 11,353,077 megamorphic store misses (69%) | ICS:2, :13-17; 8 `this.x =` stores in `AST` (W:765-772), 2 in `ASTSpan` | D (the 5 sites not listed are I) |
| N5 | Allocated bytes, freed cells, GCs per TS run (Mac, helpers 0, arm 0) | 420,554,288-420,600,112 B (role-check:40; the 2 GCS files: 420,600,112 and 420,579,376); **3,406,019-3,408,591 freed cells** over all 10-GC TS runs; 10 GCs. At 4-7 GCs a TS run frees 3.76-4.15 M cells (M, the `--gc-stats` lines of the GC-BUDGET role runs) | GCS; the GC-BUDGET role check (role-check:40) | M |
| N6 | F17 (R24 falsifier "bytes < 2 x peak live") | 420.6 MB / (119 MiB = 124.8 MB) = 3.37: the falsifier does not fire | N5; the M13-P1 ready note (ready-1:30); the R24 source note (r24-offloop:440) | D |
| N7 | Handler runs per TS run | `Construct` 2,362,471; `SetKeyedProperty` 1,405,802; `SetNamedProperty` 65,509,369 | HA:94, :99, :100 (counts at 591064fc) | M |
| N8 | Main-thread GC cost per allocated byte; `alloc_slow` per byte | 1.11 G / 420.6 MB = 2.64 cycles per byte; 0.171 G / 420.6 MB = 0.41 cycles per byte = 26 cycles per 64 B line | TD3:98; B:67; N5 (Mac bytes on x86 cycles: the byte count does not depend on the CPU, I) | D |
| N9 | Upper bound of the store-growth path on x86 | `SetNamedProperty_cold`, `grow_add`, `reserve_field_slots` are in no top-30 list: each < 0.61% = 0.153 G | TD3:69-100 (the last row is 0.61%) | D bound |
| N10 | H17 "layout tuning (gated)" is **not Frozen today**; but the slack **count rule** is not in it (check, §10 C1) | the Status list holds 6 parts and not this one; it freezes "at the A4 gate" = M8 exit; "before it a retune is a normal change, and G8 does not count it". H17:158 gives Slack tracking "layout core (the form); layout tuning (the numbers)", and H17:145 states the escape rule in words; H17:181 lists only numbers | H17:5, :7, :145, :158, :181; `T:docs/plan/09-milestones.md:39` | M (text); the reading of the count rule as "the form" is I |
| N11 | The numbers in that part | `{}` capacity 4, no-count capacity 8 (`slack::GENEROUS`), the scan limits, the OM-10 numbers 1020 / 64, the SLOTS growth max(need, 2 x old, 4) | H17:181 | M |
| N12 | The OM-10 **trigger list** is Frozen | "layout core (the OM-10 triggers as built)" | H17:164, :5 | M |
| N13 | ep9 has **no** run-time slack tracker | the tracker was measured worse (2 root shape families: richards +12.7%, deltablue +9.1% instructions) and replaced by the bytecode count | `T:crates/ep9_vm/src/slack.rs:20-27`; H17:145 ("replaces the run-time tracker") | M. C:33 ("built, V8 style, N = 7") is not right |
| N14 | The x86 named-load hit reads the SLOTS word on every field hit and selects | `select_unpredictable(k == FIELD_IN_OBJECT, o, s)`: no instruction is saved by an in-object field; the second cache line (the SLOTS cell) is | `T:crates/ep9_vm/src/ic.rs:2357-2362` | M (code) |
| N15 | Kernel time bound | cycles / task-clock: ep9 3.609 GHz, node 3.599, qjs 3.623. If the cycles are user-only, ep9 has at most 1 - 3.609 / 3.623 = 0.4% more kernel time than qjs | TD3:21, :24, :25 | D; the "if" is not verified (B:84) |
| N16 | A cheap sweep already exists | pages with no dead cell skip the walk; the walk is word-level with `count_ones`; only dead cells take `size_at` | `T:crates/ep9_heap/src/heap.rs:1891-1898`, `:1927-1945`, `:1976-1999` | M (code) |

Mapper A marked W1, W2 and W6 "G8 +1 (I)" (A:17, :18, :22). N10-N12 refine that: the **numbers** are tuning, G8 0; a **new OM-10 trigger** is layout core, G8 +1. (Check, §10 C1:) the count **rule** that L1 changes (a chain walk, an escape mode) is not a number of H17:181; H17:158 marks the Slack tracking form as layout core, so L1 is **+1 likely (I)**, not 0.

## 3. How each estimate is made (the R23 lesson)

R23:46-47 (M): a removed dead dispatch saved 5.55 cycles; a fused dispatch about 1.04; fewer ALU instructions inside a handler about 0. Instruction counts gave -0.13 to 1.39 time per 1% (R23:46). So this file prices **removed units**, not instruction shares:

| Unit | Price used | Basis |
|---|---|---|
| a removed cold-path run with an allocation (a grow) | 90-180 cycles (G): 135-175 instructions (I, from `ic.rs:2272-2312` + `props.rs:262-280` + the cold body) at an IPC of 1.0-1.5 (GC and slow-path symbols read 0.58-1.70, TD3:77-98) | bound N9: the 2 symbols are each < 0.153 G |
| a removed allocated byte | 2.64 cycles on the main thread (N8), used at 0.4 / 0.7 / 1.0 of that (dead-at-birth bytes cost less than live ones, I) | N8 |
| a removed fill byte | 0.41 cycles (N8) | N8 |
| a removed dead-cell header read | 10 / 25 / 60 cycles (G: a cold line, next to its neighbours) | bound: `sweep_page_with` 0.299 G all threads (TD3:87) |
| ALU instructions cut inside a hot handler | 0 (R23:47) | R23 |
| wider SIMD stores on a loop that waits for memory | at most 5% of the loop (D: 4 SSE2 stores per line cost 1-2 of the 26 cycles per line, N8) | N8 |

Base: ep9 main-thread cycles 24.36 G (the wall proxy, B:29); 1% = 0.2436 G. Jit base 24.86 G (B:140).
Gate value of an A/B gain x: x^t, t = 0.2-0.8 (P18:67, DL M12-PRE-SWITCH; one read gave 0.78, R23:48; STAR-DST read about 0.2 pooled, P18:67).
F1 first-read M1 odds: about 26% at a cell of 0.993 and about 67% at 0.988 (P18:217); linear between them, a lever at x1.009 (0.991) has about 42% (D: 26 + 2 x 8.2).

## 4. The lever cards

### L1. CTOR-FIT (rank 1)

| Part | Text |
|---|---|
| Cause (M) | A constructor whose `this` leaves the frame gets no count and 8 in-object slots (`T:crates/ep9_vm/src/slack.rs:28-31`, `:56`; the scan returns `None` at a `Reg` operand that holds `this`, `:191-195`, as in `_super.call(this, x)`; at a register list, `:202-213`, for 3+ arguments). 68 TS constructors start with `_super.call(this, ...)` (W: `grep -c` = 68; W:764, :1046). So every AST node has 8 slots; fields 9+ go to a SLOTS cell that grows 4, 8, 16, 32 on the store miss path (`T:crates/ep9_vm/src/props.rs:262-280`; `ic.rs:2272-2312`). 62 of 69 classes have more than 8 fields (A:122) |
| Mechanism | (1) `slack::this_store_names` gets a second mode: at a leave of `this` it sets `escaped` and goes on (today: `return None`). (2) A new `chain_names(rt, proto)`: walk the [[Prototype]] chain of `F.prototype`; at each object read the own data property `constructor`; if it is a bytecode constructor that is not DERIVED (`slack.rs:274` test), scan its code in mode (1) and add its distinct names (compare the atom bits of the constants, as `:319-328` does). Stop at a host function, a dictionary-mode prototype, an accessor, or 8 levels. (3) `first_capacity` (`slack.rs:296-334`): `n_chain = own names + chain names + the prototype-field extra (as today) + ESCAPE_SLACK`; result = `Some(n_chain)` **only if `n_chain` > GENEROUS**, else today's answer. (4) Nothing else changes: the capacity stays one byte per FunctionInfo (`:284-291`), chosen at the first construct, one root shape per constructor. About 120 lines in `slack.rs` + tests in `tests::slack` |
| Why this form | It keeps the 2 rules that M5-P2 measured: the count comes from bytecode and prototypes only (no execution policy, `slack.rs:36-37`), and every instance of one constructor starts from one root shape (`slack.rs:17-27`). The "only above 8" rule makes the lever monotone: no object gets less in-object room than today, so a row with no class above 8 chain fields is byte-identical in shapes and in bench Ir (an IDENTITY cell by construction) |
| Why not V8's rule (own + 8) | For `BinaryExpression` (own 2, total 12) it gives 10 in-object: still 1 SLOTS cell and 1 grow per node (A:113). The chain count gives 12 and 0 cells. V8 itself keeps a PropertyArray on these nodes (A:112-120), so CTOR-FIT puts ep9 ahead of V8 here, not level |
| ESCAPE_SLACK | 0, 1 or 2 slots when `this` also leaves to a call that is not the super call (`Identifier` adds `text` in `setText`, W:1051, :1053-1060). A tuning number; the count K2 picks it. Each unused slot costs 8 B per object: 2 slots on 0.77-1.47 M objects = 12-23 MB (D), so the default is 0 |
| Removes per object (D) | `Identifier` (15 fields): 216 B in 3 cells with 2 grow misses and 4 copied slots becomes 144 B in 1 cell, 0 misses (A:124). `BinaryExpression`: 136 B in 2 cells becomes 120 B in 1 |
| Removes per run | **1,465,706 SLOTS cells and grow misses (N1)**: about 30% of all cells (G: 420.6 MB at about 85 B per cell = about 5 M cells; N5 gives >= 3.4 M). SLOTS bytes: 1,465,706 x 48 B = 70.4 MB (all first grows) to 773,909 x 48 + 691,797 x 80 = 92.5 MB (D on N1, N3) = 17-22% of the bytes. The fields move in-object: +35 to +38 MB (G: 3 fields per one-grow object, 6.5 per two-grow object). **Net 35-54 MB = 8-13% of the bytes; 5-13% with slack (D)**. Dead cells: up to 0.69 M old SLOTS cells die at their second grow (D) |
| Cache lines | A field 9+ is read from a second cell today (N14). In-object it is on the object's own lines. Every child pointer of an AST node (`operand1`, `members`, `text`, `sym`) is a field 9+ (D: fields 0-9 are the `ASTSpan` + `AST` fields, W:754-772). Line N read +3.506 G backend-stall slots on named loads against node (the TD-3 read facts, box 2 line N). The out-of-line share of the 328.7 M load hits (ICS:1) is not counted: K2 |
| Est. A/B, jitless (D on M + G) | grow path: 1.4657 M x 90 / 130 / 180 cycles = 0.13 / 0.19 / 0.26 G = 0.54 / 0.78 / 1.08% of 24.36 G; less the hit that stays for a megamorphic site (0.29 / 0.28 / 0.18 points, G: not derived) = 0.25 / 0.50 / 0.90%. GC: bytes 5 / 9 / 13% x main-thread GC 4.43% (box 1: 5.21%) x 0.4 / 0.7 / 1.0 = 0.09 / 0.28 / 0.68%. Fill: 0.68% x 5 / 9 / 13% = 0.03 / 0.06 / 0.09%. Cache lines: 0 / 0.1 / 0.3% (G). **Sum 0.37 / 0.94 / 1.97% = x1.004 / x1.009 / x1.020**. (Check, §10 C7:) the GC part is a step function: one GC is about 0.40% (B:126), and a 9% byte cut moves 0 or 1 GC of 10. On the GC-BUDGET base where L1 lands (main-thread GC about 3.2%): 0.34 / 0.86 / 1.81% = **x1.003 / x1.008 / x1.018** |
| Est. A/B, jit | the same parts; GC main thread 4.99% (B:140): **x1.004 / x1.010 / x1.020**. Not a jit-only gain |
| R23 check | The gain is removed calls, removed cells and removed bytes. No part is an ALU cut inside a handler. The cache-line part is the least sure |
| Count first (0 boxes) | K0 (minutes): `ic-stats` on 5c3e1e11 reads `store grow` again. K3: the probe arm `EP9_SLACK_ARM` (the GC-BUDGET arm form, removed at the merge) on the GitHub bench mode, x64, both builds: TS Ir, allocated bytes, GC count, the 19 BENCH-SET rows; + a Mac wall preview (the E-line form) |
| Kill numbers | grows covered by the chain rule < 1.0 M of 1.47 M; or the TS x64 Ir fall < 0.5%; or the byte fall < 5%. **Pick line for LEVER-20 (P18:253, est. >= x1.015; corrected by the check, §10 C2): a Mac wall preview >= x1.015 on the jit build (the binding build, P18:59) and bytes fall >= 10% and grows covered by the chain rule >= 1.2 M. The TS x64 Ir fall is a support row only (R23:46).** (Was: Ir fall >= 0.9% and Mac preview >= x1.012, which is under the floor.) Between the 2 lines: plan v19 as a set with L3 and L4 |
| Rule 105 | at x1.009 the first-read M1 odds are about 42% (§3); at x1.008 (the GC-BUDGET base) about 34% (D: 26 + 8.2). Above the pick line (x1.015) about 67%. The odds curve is the GC-BUDGET one (P18:217); its use for L1 is I |
| Guard rows it may hurt | Rows with ES5 class chains above 8 fields: box2d, pdfjs, deltablue (its chain resolves; totals are small, so most constructors stay at 8: IDENTITY). Risk: a constructor chain that names many fields which a run never stores (a larger object, G7 memory rows). The K3 BENCH-SET read names each row whose Ir moves above 1e-5; every other row is IDENTITY by PROOF (a) (no bytecode change) and (b) |
| G8 / ADR | **+1 likely (check, §10 C1); 0 only if the reviewer agrees.** The numbers are "layout tuning (gated)", not Frozen (N10, N11). But H17:158 marks the Slack tracking form as "layout core (the form)" (Frozen at M1a, H17:5), and H17:145 states the escape rule in words. A chain walk and an escape mode change that rule; they are not numbers of H17:181. G8 row to name: H17 "Slack tracking", layout core (the form), the As-built text of H17:145. Fits the LEVER-20 row 0 / +1 (P18:159) |
| Versions | none. OBJECT_LAYOUT, IC_LAYOUT, ABI, VM layout, FACT_SCHEMA do not move: the capacity lives where it lives today (word0 flags, shape `counts`, one FunctionInfo byte; H17:158). One bytecode stream; the corpus digest does not change |
| SIMD | none in the lever. Its growth copies that are left use the shared copy and fill kernels of §5 |
| Modes room | Opens room. A T1 load of an in-object field is 1 load under a shape guard; out of line it is 2. A T1 inline allocation needs one constant size per root shape and no SLOTS cell. The capacity stays "a fact of the run" that compiled code reads and never bakes (H17:158, MC-w20-12): unchanged. AOT: no hint or unit field changes. The scan reads no preset (M-R9 holds, `slack.rs:36-37`) |
| GC-BUDGET | Bytes fall at every budget; the GC-count part shrinks with a larger budget: arm 0 10 GCs -> about 9 (D: 10 x 0.91); arm B1 6 -> 5 or 6. The per-cell mark and sweep part stays. Overlap: about -0.1 point of this lever if B1 merged first (I). CTOR-FIT lands after the GC-BUDGET merge (the base drift rule, P18:199); the GC merge check "count per row equals arm B1" (P18:234) is not touched, because it is read before |
| WIDE-FIX | no shared code and no bytecode change. Only the base drift rule: the box read of one must not span the merge of the other |
| Size, month | 1.0-1.5 agent-days + 0.5 for the probe arm and K3. Month 13 as LEVER-20 **only at the pick line** and with box room (the hold rule, P18:393); else plan v19, first set |

### L2. TABLE-MODE (rank 2 by possible size, rank 5 by what is known)

| Part | Text |
|---|---|
| Cause (D, code) | `StringHashTable.table` takes every identifier as a keyed add (W:519-530). ep9 keeps it in fast mode up to 1020 properties or 64 live transitions (`T:crates/ep9_vm/src/shape.rs:229-233`, `:938`): each new key that has no transition makes a SHAPE (64 B, not the bump path: `T:crates/ep9_heap/src/heap.rs:730-780`), a descriptor append or a prefix copy (`shape.rs:1077-1118`), a transition record and a SLOTS doubling. V8 goes to dictionary mode when a keyed-origin add passes max(12, in-object) out-of-object fields (`V8 13.6.233.17 src/objects/map-inl.h:207-216`, A:18). for-in has no enum cache: 2 Rust Vecs, a key array that grows 4, 8, 16, and a has-test per key (`T:crates/ep9_vm/src/property.rs:1313-1440`; `T:crates/ep9_vm/src/interp/ops.rs:1837-1862`; A:19) |
| Mechanism | (a) In the keyed store slow path, before `props::own_add`: a string-key add from `SetKeyedProperty` on an object with more than max(12, capacity) out-of-object fields calls `props::to_dictionary` first. (b) `for_in_keys`: count first (own count + the enumerable counts of the chain), one exact key array, a fixed stack buffer for the seen set; skip the chain walk under the prototype validity cell when no prototype has an enumerable key. (c) `ForInNext`: no has-test while the receiver's ShapeId equals the one stored at `ForInEnumerate` (the V8 enum-cache check); a dictionary receiver keeps the test |
| Removes | per new key: 1 SHAPE + the descriptor and transition work; per for-in: 2 mallocs, 1-3 cells, n lookups. Bound: `SetKeyedProperty` runs 1,405,802 times per run (N7), and that count holds the array appends too |
| Est. A/B | x0.995 / x1.003 / x1.012, both builds (G). No symbol of this path is in a top-30 list, so each is < 0.61% (TD3:69-100). (Check, §10 C5:) (a) can lose: in ep9 an IC case on a dictionary receiver stays `SLOW` (H17:150, H16 K12), and TS reads `this.table[key]` before every add (W:519-530). K2 counts these keyed loads before (a) is built |
| Count first; kill | K1 (the regroup of the TD-3 symbol tables): the sum of `shape::add_with`, `new_shape`, `alloc_shape`, `descriptors_for_child`, `record_transition`, `props::own_add`, `for_in_keys`, `for_in_has`. **Kill: the sum < 0.5% of cycles.** K2: SHAPEs made by keyed adds (< 0.3 M per run kills (a)); `for_in_keys` calls x keys (< 2 M key visits kills (b), (c)) |
| Guard rows | rows that build an object by keyed adds and then read it by name: dictionary receivers are `SLOW` IC cases (H16 K12, H17:150). pdfjs, earley-boyer, code-load, JetStream rows: all are LEVER cells |
| G8 / ADR | (a) **+1**: a new OM-10 trigger changes "layout core (the OM-10 triggers as built)" (N12). (b), (c) 0. No version move (I: (c) uses the existing for-in register triple; not checked in the bytecode ABI text) |
| SIMD | none. A SIMD key compare in `lookup_with` (E:84, x1.0015) is not worth it before SHAPE-LOOKUP-COUNT |
| Modes room | neutral: V8 has the same rule, and a T1 treats a dictionary receiver as generic in both engines. (c) gives T1 a simple for-in guard (one ShapeId compare) |
| GC-BUDGET / WIDE-FIX | fewer shape-space cells and SLOTS bytes: the same sign as L1. No shared code with WIDE-FIX |
| Size, month | 2.0-3.0 agent-days; plan v19, only after K1 + K2 |

### L3. SWEEP-LEAN (rank 3; the best fit for "allocation + SIMD")

| Part | Text |
|---|---|
| Cause (M, code) | The release sweep reads the header of each dead cell (`size_at`, `T:crates/ep9_heap/src/heap.rs:1987`, `:2262-2268`) only to add `freed_bytes` (`:1998`). Readers of `freed_bytes`: stats and tests (C:161; D:264); the CLI prints `freed_cells` only (`T:crates/ep9_cli/src/run.rs:214-215`). 3.406 M such reads per TS run (N5) |
| Mechanism | (a) In the not-CHECK, not-zap sweep: `freed_cells += (vo & !mk).count_ones()`; no `size_at`. `freed_bytes` is exact only when a stats switch or a test asks. (b) Then the walk of a page is 3 pure passes over 512 words: `alive = vo & mk`, a population count, the 2 store-backs, and the line marks. A line is 32 granules = one u32 half of a bitmap word (`heap.rs:2510-2512`), so "a live cell starts in this line" is a u32 != 0 test: 1,024 lanes per page. One SIMD kernel does these passes; the rare "last cell reaches the next line" fix (`heap.rs:2040-2080`) stays scalar |
| Removes | 3.406 M header loads per run on lines that the mutator left long ago; about 20-43% of them go with L1 already (overlap) |
| Est. A/B | reads 3.406 M x 10 / 25 / 60 cycles = 0.034 / 0.085 / 0.204 G on all threads; the main-thread part is about 45% (D: 0.53 of 1.19 points, D:82) = 0.06 / 0.16 / 0.38% of 24.36 G: **x1.0005 / x1.0015 / x1.0035** jitless. Jit: the sweep symbol is 1.64% against 1.19% (TD3:48, :87): x1.0005 / x1.002 / x1.005. The SIMD kernel alone: 0.01-0.02% (E:124): it is not the gain, (a) is |
| Count first; kill | K0: `--gc-stats` `freed_cells` per build (have: 3.406 M on the Mac). K1: the main-thread share of `sweep_page_with`. **Kill: main-thread sweep < 0.3% of cycles.** A Mac `perf`-like record must show the sweep rows fall |
| Guard rows | splay, earley-boyer, raytrace (GC-heavy): the change removes work only; the checked sweep (GC-stress, CHECK) keeps every read |
| G8 / ADR | 0: a fill of H09 in the DL M9-P3-WPRE-2 form (DL:5477): no Decision sentence or Rules row changes; the heap stays parsable; the verifier walk and the zapping stay (H09:47). A reviewer may dispute it (the same note as DL:5477). (Check:) the fill note must say that the release sweep no longer reaches the "heap corrupt: unregistered kind" panic on a dead cell (`heap.rs:2265-2267`); CHECK keeps it |
| SIMD dispatch | §5: x86-64 AVX2 (4 words per step; AVX-512 VL + VPOPCNTDQ on 256-bit lanes where detected), SSE2 base; aarch64 NEON (`cnt`); wasm simd128 (`i8x16.popcnt`); scalar = today's loop |
| Modes room | neutral: GC-internal. A leaf with no GC point |
| GC-BUDGET | fewer GCs sweep the same dead cells in fewer passes: no overlap in the read count; the per-page part falls with the GC count |
| Size, month | 1.0 agent-day for (a), 0.5 for (b) after the dispatch module; plan v19, in L1's set |

### L4. LIST-FIT (rank 4)

| Part | Text |
|---|---|
| Cause (D, code) | `new Array()` never hits: the construct hit needs an interpreted callee (`T:crates/ep9_vm/src/construct_ic.rs:112-116`), so it runs `slow::call` -> `call_host` -> `natives::array_constructor` -> `new_array` -> `initial_shape` (A:21). `ASTList` does it for each list (W:995). Growth writes the new cell twice: a HOLE fill of all slots (`T:crates/ep9_vm/src/object.rs:886-893`), then the copy (`:1322-1329`), each slot with the E2 test (`object.rs:380-382`) |
| Mechanism | (a) A construct-hit arm for the realm's `%Array%` with 0 arguments: the `literal_site::fast_empty_array` body (`T:crates/ep9_vm/src/literal_site.rs:328-344`). (b) `store_element_slow`: one E2 test per cell, `ptr::copy_nonoverlapping` for the old part (libc picks its SIMD body at run time), the HOLE fill only for the tail, with the §5 fill kernel. (c) The same form for `reserve_field_slots` and `copy_descriptors` |
| Removes | per `new Array()`: the host-call chain (about 300-500 instructions, G); per growth: `cap` slot stores and `2 x cap` E2 tests |
| Est. A/B | x1.000 / x1.002 / x1.005 (G). Bounds: `Construct` runs 2.36 M times, of them 0.80 M are AST nodes (N7, N3); keyed stores 1.41 M (N7) |
| Count first; kill | K2: `array_constructor` calls from `Construct` with 0 arguments (< 0.15 M kills (a)); `store_element_slow` growths by old capacity (< 0.3 M kills (b)). K1: the symbols' cycle sum < 0.25% kills the lever |
| Guard rows | rows with `new Array(n)` loops (navier-stokes, zlib, crypto): only the 0-argument form changes, so they are IDENTITY if (a) tests the count first |
| G8 / ADR | 0. If (a) needs a new handler kind, it is an append to the H16 "IC lists (append-only)" part (H17:183 names the part): G8 0, IC_LAYOUT note only. The growth number does not change (C:150: ep9's x2 already copies less than V8's rule) |
| SIMD | (b), (c) through libc `memcpy` (glibc ifunc, AVX-512 on the box: TD3:226; libSystem NEON; wasm `memory.copy`) + the §5 fill kernel |
| Modes room | (a) is the allocation-site form a T1 inlines (a constant-size array header). Good |
| Size, month | 1.0-1.5 agent-days; plan v19, after K1 + K2 |

### L5. ALLOC-ENTRY (rank 6; for the modes room, not for the gate)

| Part | Text |
|---|---|
| Mechanism | A second entry `Heap::alloc_fixed(kind, SIZE, word0)` for kinds whose size the call site knows: no `kind_sizes` load and no 2 size tests (`T:crates/ep9_heap/src/heap.rs:697-708`); one word0 store with the shape and flags ready, in place of 3 writes and a barrier test (`heap.rs:929-932`; A:24); `Runtime::enter` only on the miss (C:163). The VO bit stays (H10 conservative scan) |
| Removes | about 15-25 of the 30-40 instructions per object (I, A:58) x about 5 M cells (G) = about 0.1 G instructions = 0.2% of Ir |
| Est. A/B | x1.000 / x1.000 / x1.002 (check, §10 C3; was 1.001 center): these are ALU and store cuts inside handlers, which read about 0 at the box (R23:47). `Construct` self time is 0.90% for 2.36 M runs = 96 cycles per run (TD3:93; N7) (D), and the entry is a small part of it |
| Count first; kill | K2: cells per run by kind. **Kill for M2: < 8 M cells.** DL W4-HEAP-ALLOC-UNCHANGED (DL:541): one cold arm moved LLVM inlining by +0.40% Ir, so the bench Ir of all 19 rows is read first |
| G8 / ADR | 0: the H09 fill form of DL M9-P2-WPRE-4 (DL:5420; "carried to the M2-exit G8 review with the T1 inline TLAB facts") |
| Modes room | **This is the door a JIT needs**: a bump with a constant size and one header store is what T1 inlines (C:24, CSA:1420-1538). Design it once for T0 and T1 |
| Month | with the T1 baseline design; not a month-13 or gate lever |

### L6. SRC-HASH (rank 5; the only pure SIMD site with a measured share)

| Part | Text |
|---|---|
| Cause | `UnitBuilder::new` hashes the whole source with a portable SHA-256 at every compile (`T:crates/ep9_bytecode/src/builder.rs:945-949`), and the verifier hashes it again to compare (`T:crates/ep9_bytecode/src/verify.rs:167-168`): 2 x 2.51 MB = 5.0 MB per run, 0.068 G / 5.0 MB = 13.5 cycles per byte, a normal portable rate (D; check §10 C6); 0.27% of TS cycles on the m9-p5 build (E:83; `T:docs/research/22-m2-stuck-causes.md:129`). It runs in the front-end job (the `ep9-frontend` worker while the caller blocks, `T:crates/ep9_frontend/src/worker.rs:1-12`): on the wall path, outside the 24.36 G main-thread base; that thread is 0.46-0.50% of TD3 samples (TD3:236-239), a bound for this tree (D) |
| Mechanism | First: compute `source_hash` only when a unit is written or checked (removes the work; needs an H02 read). Else: SHA-NI and ARMv8 SHA2 bodies behind the §5 dispatch, the portable body as the fallback and on wasm |
| Est. A/B | x1.000 / x1.002 / x1.003 (E:122). Too small for a wall read: judged by counters |
| Count first; kill | K1 or a Mac count: `sha256` share and its thread. **Kill: < 0.1% of cycles, or it runs off the waited path** (E:149) |
| G8 / versions | 0 if the digest stays byte-equal (the SIMD form). (Check, §10 C6:) the lazy form touches the source-hash field of the Frozen H02 ABI-core header (H02:5, :51) and the verifier compare (`verify.rs:167-168`): it needs a G8 row or a proof. This plan takes the SIMD form only |
| Modes room | an AOT unit and a cache key need the hash: the lazy form must compute it on those paths. Neutral |
| Month | plan v19 or a ride role; 0 boxes |

### L7. FAULT-AHEAD (rank 7)

| Part | Text |
|---|---|
| Mechanism | The sweeper thread commits, touches and `undefined`-fills the next fresh pages while JS runs (JSC `useWarmUpMarkedBlocks`, D:97); the TLAB takes them with `tlab_filled` at the page end. The fill uses the §5 fill kernel |
| Est. A/B | x1.000 / x1.000 / x1.004 (G; check §10 C4, was 1.001 center). Bound N15: at most 0.4% more kernel time than qjs; this bounds only ep9 minus qjs, and it rests on "cycles are user-only" (not verified, B:84). A fill on another core does not help by itself: the lines then come from that core's cache (I) |
| Count first; kill | K6: page faults per TS run, ep9 and node (Mac `/usr/bin/time -l`; `perf stat -e page-faults` on the next box that runs for another reason). **Kill: faults x the fault cost < 0.2% of cycles** (D:265) |
| G8 | 0 (H09 fill form). Memory rows (G7) may rise: pages are committed earlier |
| Month | plan v19, only on the count |

## 5. SIMD: the dispatch plan, and what SIMD can and cannot do here

**The honest bound.** No measured loop of 0.61% or more outside GC and allocation exists (B:47). The measured loops and what a real SIMD body saves:

| Loop (measured symbol) | Share | SIMD saves | Mark |
|---|---|---|---|
| TLAB `undefined` fill (`alloc_slow`) | 0.68% (TD3:98), IPC 0.58 | <= 5% of the loop = <= 0.03%: it waits for the line, 26 cycles per line (N8) | D |
| sweep bitmaps (`sweep_page_with`) | 1.19% / 1.64% (TD3:87, :48) | 0.01-0.02% (E:124); the dead-cell read (L3 (a)) is the cost | D + G |
| source SHA-256 | 0.27% (old build) | 0.23% (E:122) | M + G |
| `lookup_with` linear scan | 1.01% (TD3:90) | 0.15% (E:123); the planned cache removes the call | M + G |
| growth copies | < 0.61% each | through libc, after the E2 test leaves the loops (L4) | I |
| sum (all rows) | | about 0.3-0.45% = x1.003-1.0045 | D |
| **sum of what this plan builds** (check, §10 C8: `lookup_with` and the TLAB fill are rejected in §8) | L6 SHA bodies <= 0.23% (old-build share) + L3 kernel 0.01-0.02% | **about 0-0.25% = x1.000-1.0025** | D |

So "more SIMD" is right as a **rider** on levers that change the loop (L3, L4, L6, L7). It is not a gate lever. Today its gain rests on one site (L6) with an old-build share.

**One dispatch module (0.5 agent-day; first, once).** `ep9_platform::simd` (both `ep9_heap` and `ep9_vm` depend on `ep9_platform`: `T:crates/ep9_heap/Cargo.toml:12`, `T:crates/ep9_vm/Cargo.toml:17`). It joins the 2 tables that exist (`EP9_SIMD_SEARCH`, `EP9_SIMD_JSON`; DL:5474 open item (c); E:21).

| Item | Rule |
|---|---|
| Detection | once per process into one `AtomicU8` bitset: x86-64 `is_x86_feature_detected!` for `avx2`, `sha` + `sse4.1`, `avx512vl` + `avx512vpopcntdq`; aarch64 NEON at compile time, `is_aarch64_feature_detected!("sha2")`; wasm32 `cfg(target_feature = "simd128")` |
| Bodies | each a leaf `#[target_feature]` function: no call, no GC point, no keep path (`T:crates/ep9_vm/src/interp/keepcall.rs:28-36`); `vzeroupper` before a return from ymm code; AVX-512 only behind detection (DL:5474; a compile-time `avx512f` build stays a `compile_error!`, `keepcall.rs:65-69`); "only on 256-bit lanes" is this design's choice (I), not a DL fact |
| Fallback | the scalar body is today's code; `EP9_SIMD=scalar|sse2|avx2|...` forces a body for tests and FASTDIFF |
| Kernels | `fill_u64(ptr, n, pattern)` (L4, L7, the TLAB fill); `sweep_words(vo, mk, keep) -> live, dead` with the line-lane test (L3); `sha256_blocks` (L6). Copies use `ptr::copy_nonoverlapping` (libc dispatch) |
| Tests | aarch64 on the Mac; x86 forms (scalar, SSE2, AVX2, AVX-512 VL) on a Cloudflare box that the orchestrator runs (the W3-SIMD-BOX-2 form, DL:5457); that box is box time that the hold rule (P18:393) does not count: a v19 item (check §10); wasm simd128 by the standalone build (`T:crates/ep9_vm/src/string/search.rs:42-44`), since no CI job builds it (E:55) |
| Modes | a later JIT or AOT unit calls the same leaves by a plain C ABI; an AOT unit checks the `ep9_unit` CPU bits at load (`T:crates/ep9_unit/src/host.rs:100-135`), not in its code (E:143) |

## 6. The best combined set, and the sum against the need

| Set | Levers | Center A/B (ln sum) | Why |
|---|---|---|---|
| ALLOC-SET-1 (anchored on measured counts) | L1 + L3 + L6 | 0.0090 + 0.0015 + 0.0020 - 0.0005 (L1 x L3 overlap) = 0.0120 = **x1.012** | one object path, one count build, one dispatch module |
| + the count-gated ones (corrected by the check, §10) | + L2 + L4 + L7 + L5 | + 0.0030 + 0.0020 + 0 + 0 - 0.0010 (GC overlap) = 0.0160 = **x1.016** (jit x1.017) | all (G) until K1 + K2. (Was 0.0040 + 0.0020 + 0.0010 + 0.0010 - 0.0010 = 0.0190 = x1.019) |
| low / high | all 7 | about x1.000 / about x1.045 | the low now holds L2 at x0.995; the high needs every high at once: not expected |

Against the need (ln: jit 0.0402 / 0.0411; jitless 0.0218 / 0.0188). Numbers corrected by the check (§10 C10); the old values are in `f-design.pre-check.md`:

| Case | Jitless | Jit |
|---|---|---|
| This route alone, center, at t = 0.2 / 0.6 / 0.8 | x1.003 / x1.010 / x1.013: covers 15-17 / 44-51 / 59-68% of the need | x1.003 / x1.010 / x1.014: covers 8 / 25 / 33% |
| + GC-BUDGET x1.012 + WIDE-FIX x1.017 (jit x1.006), centers (P18:217-218), GC overlap -0.002 | ln 0.0428: passes both cells from t = 0.51 (D: 0.0218 / 0.0428); at t = 0.6 the cells read 1.003 / 1.007; at t = 0.2 they fail (0.986 / 0.989) | ln 0.0327 < 0.0402: **fails at every t <= 1**; at t = 0.8 the cells read 0.986 / 0.985 |
| What the jit cell still needs | - | about x1.018 more A/B at t = 0.8 from a jit-side lever (D: 0.0402 / 0.8 - 0.0327 = 0.0175). JIT-DELTA (P18:269) is the count that looks for it |

- Every number here is an estimate. One Path B read is good to about 1 point (P18:59), so a x1.011 gate move is at the noise.
- The ALLOC route helps both builds the same. It does not close the gap between the jit and the jitless build.
- **Month 13:** L1 can be the LEVER-20 pick only at its pick line (§4 L1, as corrected in §10 C2). At today's center (x1.009; x1.008 on the GC-BUDGET base) it is under the floor x1.015 (P18:253), and the honest plan word is still "expected NOT RUN" (P18:263). The work that is worth doing in month 13 is the count runs K0-K3 (0 boxes): they turn L1 from an estimate into a measured Ir, byte and GC-count change, and they price L2-L4.

## 7. The order of the count runs (0 boxes)

| # | Count | Where | Cost | Opens | Kill line |
|---|---|---|---|---|---|
| K0 | `--gc-stats` (bytes, freed cells, GCs) on TS for the jit and the jitless build; an `ic-stats` build for `store grow` and the megamorphic store sites, at 5c3e1e11 | Mac, existing switches (`T:crates/ep9_vm/Cargo.toml:38`) | minutes | confirms N1, N3, N5; shows if the jit build allocates more (a rider for JIT-DELTA) | `store grow` < 1.0 M: L1 falls under its low |
| K1 | Regroup of the TD-3 full symbol tables (`out-2/rec/*.sym.tsv`, `out-1`) by class: the grow path, the shape-add path, for-in, `array_constructor`, growth, closures and contexts, sha256, libc mem*, sweep by thread | the orchestrator (data on disk; this role may not read `rec/`) | 0 boxes, 0 builds | x86 cycle prices for L1 (instruction part), L2, L3, L4, L6; closes the 15.15% tail (B:78) | per lever, §4 |
| K2 | CENSUS: a count build (a cfg feature in the `ic-stats` form; not committed): cells and bytes by kind; SLOTS cells by capacity and by constructor, and "covered by the chain rule: yes / no"; fields added after the constructor returns, by class (ESCAPE_SLACK); the out-of-line share of named-load hits; SHAPEs from keyed adds; `for_in_keys` calls and keys; `array_constructor` 0-argument calls; `store_element_slow` by old capacity; closures and contexts | Mac (counts do not depend on the CPU) | 0.5 agent-day | L1's slack number; L2, L4, L5 | per lever, §4 |
| K3 | The CTOR-FIT probe arm on the GitHub bench mode, x64, both builds: TS Ir, bytes, GC count (the `gc-count` step form), 19 BENCH-SET rows; + the Mac wall preview | GitHub + Mac | 1.0-1.5 agent-days (it is the lever) | the LEVER-20 pick row for L1 | §4 L1 kill and pick lines |
| K4 | SHAPE-LOOKUP-COUNT with the caller split (keyed table loads, for-in has-tests) | already planned (P18:270) | - | L2 (c); the cache lever | - |
| K5 | JIT-DELTA rider: allocated bytes and cells by kind, jit minus jitless | already planned (P18:269) | - | a jit-only allocation cut, if one exists | < 2% more bytes in the jit build: none |
| K6 | Page faults per TS run, ep9 and node; do the perf lists hold kernel samples | Mac now; a box that runs for another reason | minutes | L7 | §4 L7 |

Order: K0 and K1 today (no build). K2 and K3 share one agent (one compiling slot). K4, K5 ride their planned dispatches. No count needs a box.

## 8. What I would NOT do, and why

| Not | Why | Source |
|---|---|---|
| A wider TLAB fill as a lever (AVX2 / AVX-512 stores alone) | the fill waits for the line (26 cycles per line; IPC 0.58): <= 0.03% | N8; TD3:98 |
| Remove the TLAB pre-fill (fill at the allocation, V8 form) | the fill is the line fetch that the init stores would pay; the handlers get larger | N8; A:30 |
| `undefined` = 0 so `clzero` or the zero page can fill | H06 value encoding is Frozen; every tier bakes it | C:180 |
| SIMD in the mark loop, the stub-cache compare, the frame fill, `lookup_with` (before its cache) | memory bound, the same µop count, measured worse (-1.5% on a64), or the call goes away | E:86, :93, :96, :84 |
| VO bits in bulk at TLAB retire | H09 + H10 safety review for ALU cuts that read about 0 | D:268; R23:47 |
| V8's run-time slack tracker, or V8's "own + 8" | the tracker was measured worse (N13); "own + 8" still leaves 1 SLOTS cell on 60+ classes | `slack.rs:20-27`; A:112-120 |
| An inline megamorphic-add path for the base-constructor stores (N4) | the whole cold store body is < 0.61% of cycles (N9); it is an IC lever (the MH / NL-1 family, closed: P18:260, :276), not an allocation lever | TD3:69-100 |
| V8's elements growth (old x 1.5 + 16) | ep9's x2 copies fewer elements | C:150 |
| Mementos, pretenuring, a moving nursery, concurrent mark | no effect in an interpreter (C:112); Frozen H10 / H11 / H12; S2b is out of M2 (P18:276) | C:167; D:276-284 |
| A change of any registered line, or a second lever in the GC-BUDGET or WIDE-FIX read | rule 96; the base drift rule | P18:117, :199 |
| A box for any count above | every count is 0 boxes; the box room goes to the 2 carried sets and the hold rule | P18:393 |

## 9. Not verified

- N1, N3, N4 are from one `ic-stats` run at 0a741e5d (2026-11-11). N2 says why they should hold; K0 reads them again.
- The cycles per grow, per dead read and per byte are (G) unit prices inside (D) bounds. No x86 disassembly was read.
- The share of the 1.47 M grows that the chain rule covers is not counted (K2). Grows by fields added in methods after the constructor are not covered.
- The cell count per run (about 5 M) is (G); only freed cells (3.406 M) are measured.
- N4 names 5 megamorphic sites from the list and infers the other 5.
- The G8 reading of L1 (now +1 likely, §10 C1) and of L3, L5 (H09 fill, 0) can be disputed by a reviewer; L2 (a) is +1 by N12.
- N15 rests on "the cycles are user-only", which the summary text does not say.
- L2 (c) and L6 (lazy) touch H16 / H02 texts that this role did not read.
- The Mac byte and cell counts are used with x86 cycle shares; the x86 `gc/` files were not read (brief).

## 10. Check corrections

The adversarial check (`g-check.md`, 2026-11-25) changed these items in place. The text before the check is in `f-design.pre-check.md`.

| # | Item | Was | Now | Evidence |
|---|---|---|---|---|
| C1 | L1 G8 | 0 / +1, "the count rule is tuning" | **+1 likely (I)**; 0 only if the reviewer agrees. G8 row: H17 "Slack tracking", layout core (the form) | H17:158 "layout core (the form); layout tuning (the numbers)"; H17:5 (layout core Frozen at M1a); H17:145 states the escape rule in words; H17:181 lists only numbers |
| C2 | L1 pick line | Ir fall >= 0.9% + bytes >= 10% + Mac preview >= x1.012 | Mac wall preview >= x1.015 on the jit build + bytes fall >= 10% + covered grows >= 1.2 M; Ir only a support row | P18:253 floor x1.015; P18:59 the jit build binds; R23:46 Ir does not predict time |
| C3 | L5 center | x1.001 | x1.000 (low / high x1.000 / x1.002) | §3: ALU and store cuts inside handlers price 0 (R23:47) |
| C4 | L7 center | x1.001 | x1.000 (G) until K6 | no count; N15 bounds only ep9 minus qjs kernel time, and its "user-only" premise is not verified (B:84) |
| C5 | L2 | x1.000 / 1.004 / 1.012 | x0.995 / 1.003 / 1.012 (G) | an IC case on a dictionary receiver stays SLOW (H17:150, H16 K12); TS reads `this.table[key]` before each add (W:519-530) |
| C6 | L6 | 2.5 MB hashed; the lazy form G8 0 | 5.0 MB (2 hashes per compile); the SIMD form only at G8 0; the lazy form needs a G8 row (H02 ABI-core header field) | `builder.rs:949`, `verify.rs:167-168`; H02:5, :51; 0.068 G / 5.03 MB = 13.5 cycles/B (D) |
| C7 | L1 on the base it lands on | x1.004 / 1.009 / 1.020 | the same on today's base; **x1.003 / 1.008 / 1.018 on the GC-BUDGET base** (D: main-thread GC about 3.2%); the GC part is a step (0 or 1 GC of 10; 1 GC = 0.40%, B:126) | P18:199; B:126 |
| C8 | §5 SIMD sum | x1.003-1.0045 | what the plan builds: x1.000-1.0025 | the old sum held `lookup_with` (0.15%) and the TLAB fill (0.03%), which §8 rejects |
| C9 | N2, N5, §3, L1 Cause, §5 Bodies cites | 914618e8 base; bytes 420,599,344-420,600,368; P18:119; `slack.rs:202-209`; "256-bit lanes (DL:5474)" | 0a741e5d base with ic.rs 685 changed lines (grep: no grow-rule change); 420,554,288-420,600,112 B, freed 3,406,019-3,408,591; P18:67; `slack.rs:191-195` (Reg) or `:202-213` (3+ args); "256-bit lanes" is a design choice (I) | git diff --stat at T; GCS files; role-check:40; P18:67; slack.rs |
| C10 | §1 / §6 sums | x1.019 (jit x1.020); jitless pass from t 0.48; jit needs x1.015 more | **x1.016 (jit x1.017)**; at the gate x1.003 / 1.010 / 1.013; jitless passes from t 0.51; jit fails at every t and needs about x1.018 more at t = 0.8 | D from C3-C5 |
| C11 | L3 note | - | the release sweep no longer reaches the "heap corrupt" panic on dead cells (`heap.rs:2265-2267`); the H09 fill note says it. Freed cells do not fall with fewer GCs (3.76-4.15 M at 4-7 GCs, M), so "no overlap with GC-BUDGET" holds | GC-BUDGET `--gc-stats` lines |
| C12 | SIMD x86 box | "no count needs a box" | true for the counts; the L3 (b) and L6 SIMD bodies need one x86 Cloudflare box test (W3-SIMD-BOX form), which the hold rule does not count: a v19 item | DL:5457; P18:393 |

The top 3 that survive: **L1 CTOR-FIT** (x1.003 / 1.008 / 1.018 on the GC-BUDGET base; G8 +1 likely), **L3 SWEEP-LEAN** (x1.0005 / 1.0015 / 1.0035; G8 0), **L6 SRC-HASH in the SIMD form** (x1.000 / 1.002 / 1.003; G8 0). ALLOC-SET-1 stays x1.012. No lever reaches the floor. In month 13 the work is K0-K3 (0 boxes).
