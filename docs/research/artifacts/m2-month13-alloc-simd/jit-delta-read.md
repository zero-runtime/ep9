# JIT-DELTA read (run 45606203565, 2026-11-25)

Role: jit-delta-reader. Read only. 0 builds, 0 boxes.

- Run: GitHub bench mode, ref `m2/m13`, base = head = 5c3e1e11, workload octane-typescript, linux-x64, cachegrind with `--branch-sim`, the jit build and the jitless build. Both bench json files say PASS.
- Inputs: the 4 `.out` and 4 `.log` files of `bench/cg` (jit) and `bench/cg-jitless` (jitless), the 2 json summaries, `jit-delta-table.txt` and its 3 tsv files, plan v18 §0.4, §4.4, §4.5 (P18:75, :257, :269).
- Tags: **M** measured (a count in a file), **D** derived (arithmetic), **I** inferred, **E** estimate, **G** guess.
- Price model (from the table script): est. = C / (C - dIr x k). C = jit Ir / IPC 2.071 = 25.17 G cycles. k = 0.05 / 0.241 / 0.483 cycles per removed Ir (low / center / high). Break-even for x1.010: 4,984 M / **1,032 M** / 516 M Ir.

## Verdict

```
 jit - jitless Ir      +612.49 M = +1.189%   (positive rows +688.38 M, negative rows -75.89 M)
 best group (center)   compare record  +285.5 M  ->  x1.0027      keyed record +282.6 M -> x1.0027
 all positive rows     +688.4 M  ->  x1.0014 / x1.0066 / x1.0134
 lever line            x1.010 at center = 1,032 M Ir         no group reaches it; the whole delta does not reach it
 call                  JIT-DELTA prices no jit-only lever (no designer-jit run)
```

## 1. The table check

### 1.1 The A/A line (base = head = 5c3e1e11)

| Build | base Ir | head Ir | head - base | relative |
|---|---:|---:|---:|---:|
| jit | 52,129,590,719 | 52,130,369,314 | +778,595 | **+1.49e-5** |
| jitless | 51,516,972,316 | 51,517,879,304 | +906,988 | **+1.76e-5** |

- **(M)** The totals differ for the same sha. The difference is **above 1e-5** in both builds. The plan does not name a measured same-code Ir noise. It names 1e-5 as a line: the IDENTITY clause (b) "bench Ir within 1e-5" (P18:140) and the GC-BUDGET merge-head clause (P18:234). This A/A is 1.5x and 1.8x that line.
- **(M)** Where the difference is (per fn, head - base): jit `_int_malloc` +0.545 M, `unlink_chunk` +0.114 M, `__memcpy_avx_unaligned_erms` +0.063 M; jitless `_int_malloc` +0.536 M, `unlink_chunk` +0.198 M, memcpy +0.159 M. The rest is small and in the front end (scope `Builder::name`, `StringTable::intern`, a `HashMap<&str, NameId, RandomState>` insert). No handler row moves.
- **(I, not proved)** A likely cause: the std `RandomState` seed is new in each process, so the front end's map order and the malloc paths change from run to run. The bench `hash_seed: 1` does not fix that seed.
- **Effect on this read: none (D).** The A/A is 0.78-0.91 M. The JIT-DELTA is 612 M, about 700x larger.
- **Effect on the plan (hand back, not fixed here):** on octane-typescript x64 a same-sha pair can fail the 1e-5 IDENTITY line. By rule 104 (instrument first) this is a plan finding for the null ledger and plan v19.

### 1.2 Three rows by hand (fn= totals of the two head `.out` files)

| Row | jit Ir | jitless Ir | jit - jitless | table |
|---|---:|---:|---:|---|
| TestEqual | 938,422,973 | 804,493,332 | +133,929,641 | 938.42 / 804.49 / +133.93: OK |
| GetKeyedProperty_cold | 264,726,026 | 133,310,408 | +131,415,618 | 264.73 / 133.31 / +131.42: OK |
| ep9_vm::string::array_index | 144,586,514 | 80,285,811 | +64,300,703 | 144.59 / 80.29 / +64.30: OK |

- **(M)** The sum of all fn= cost lines equals the `summary:` line in both files (52,130,369,314 and 51,517,879,304).
- **(M)** The table's hook view (§2, §3: 349.5 M) counts only `profile.rs` lines and `record*` fns. Std code inlined from a hook (`atomic.rs:3994` header loads, `bool.rs`, `cell.rs`, `ptr`) and `kinds.rs` stay under the handler. Thus §3 is a low bound of the record cost. The groups below use the full handler rows.

## 2. The groups, priced

### 2.1 Why the rows grow (line reads of the `.out` files)

- **GetKeyedProperty_cold +131.4 M: the jit build does NOT take the cold path more often (M).** The per-entry lines are equal in both builds: `dsl.rs:1503`, `exec.rs:81`, `interp/mod.rs:190` = 2,380,543 each. The Mac census K2 counted 2,380,650 cold bodies (R26 §6). The jit build does more work per entry: `record_keyed_cold` (`profile.rs` +47.4 M, `kinds.rs` +38.5 M, header atomics +19.1 M, `object.rs` +6.8 M, rest). That is about 55 Ir per cold entry (D).
- **string::array_index +64.3 M (I, from the source):** `keyed_key` in `record_keyed_cold` calls it for each string key (`profile.rs:840`). The jitless build calls it only from the generic keyed get (80.3 M). With it the keyed cold record costs about 82 Ir per cold entry (D).
- **TestEqual +133.9 M (M):** the same entries in both builds (`hint.rs:879` = 43,685,814). The jit handler adds the pair test of the `x == null` arm (`nullish_pair_bits` + `adds_bit`, `profile.rs:1111-1176`; `profile.rs` +53.4 M; 6.62 M entries take it), the std code inlined into that test (+54.6 M: header atomics, `bits.rs`, `bool.rs`, `cell.rs`, `ptr`) and +21.0 M on `ops.rs` (`:102`, `:107`, the nullish test). The entry and dispatch lines move from `interp/mod.rs` (-38.8 M) to `dsl.rs:275` (+43.7 M): net +4.9 M. In total about 3.1 Ir per entry (D).
- **record_pair_next::<76> +129.8 M (M):** jit only, 2,527,781 calls, about 51 Ir per call (D). The jitless build runs `skip_next::<76>` there (-7.6 M).
- **Call window (M):** `CallProperty0` +44.0 M = `profile.rs:762` + `ops.rs:689`, 22.26 M each: 2 Ir per call (the `call_window_after_unit` test).
- **GetKeyedProperty +50.6 M (M):** `keyed_hit_kind` on the array hit path (`profile.rs:891`, `:896`).
- **Negative rows -75.9 M (M):** the jitless cold handlers jump one more time (to the sib cold handler): `Wide_cold` -38.2 M, `CallProperty1_cold` -14.7 M, `JumpIfToBooleanFalse_cold` -6.9 M; Bi doubles in those rows.

### 2.2 Prices (est. A/B on the jit TS cell if the whole group goes; low / center / high)

| Group (fact kind) | Rows | dIr (M) | low | center | high |
|---|---|---:|---:|---:|---:|
| Compare record | TestEqual, TestEqual_cold, record_pair_next, TestGreaterThanOrEqual, TestLessThan | 285.5 | x1.0006 | **x1.0027** | x1.0055 |
| Keyed cold record | GetKeyedProperty_cold, SetKeyedProperty_cold, string::array_index | 219.5 | x1.0004 | **x1.0021** | x1.0042 |
| Call window | CallProperty*, CallUndefinedReceiver*, record_call_next, record_call_out | 107.8 | x1.0002 | **x1.0010** | x1.0021 |
| Keyed hit kind | GetKeyedProperty, SetKeyedProperty | 63.2 | x1.0001 | **x1.0006** | x1.0012 |
| Other positive rows | Add_cold, ForInNext_cold, CreateClosure_cold, malloc, ... | 12.5 | x1.0000 | x1.0001 | x1.0002 |
| **All positive rows** | | **688.4** | x1.0014 | **x1.0066** | x1.0134 |
| Net (jit runs the jitless Ir) | | 612.5 | x1.0012 | x1.0059 | x1.0119 |

- **(D)** No group reaches x1.010 at the center. The largest is 0.27 of the line (ln ratio). Even the full positive delta is x1.0066 at the center. Only the high case of the full delta passes x1.010, and that is not a cut that a lever can make.

### 2.3 Ir against the wall gap

```
 end read m12-p1 (vs node --jitless):  jit 0.968   jitless 0.985   ->  jit build about 1.8% slower (box 1; box 2: 0.980 / 0.988 = 0.8%)
 what Ir explains (net +612 M):        low 0.12%   center 0.59%   high 1.19%
 not seen by Ir (center, box 1):       about 1.2 points
 TD-3 perf (R18S §3):                  jit build +1.0% instructions, +2.5% cycles   -> 1.5 points of cycles are not instructions
```

- **Branch misses (M, D): the Bim column does not explain the gap; it points the other way.** jit - jitless: Bim -32.0 M, Bcm -1.7 M, Bi -71.2 M. At 13 cycles per miss that is about -0.42 G cycles (-1.65% of C): by this model the jit build should be faster. The top Bim rows are handlers with no record code: `LdaUndefined` -10.5 M (jit 0), `LdaCurrentContextSlotStar` -8.7 M. That is code layout in the simulator's simple per-address indirect predictor, not the predictor of a real x86 core. So the Bim column is not usable evidence for this gap (I).
- **What Ir cannot see (I):** (1) real branch misses (history-based indirect prediction, with a different layout in each build); (2) I-cache and iTLB misses: the record code makes the hot handlers larger; (3) D-cache and store traffic: the record stores write the feedback vector (`put_int`, `add_bits`); (4) stack and register costs of the larger jit handlers (spills, saves).
- **Counts that would see it:** (a) 0 boxes: the same bench mode with `--cache-sim=yes` on both builds (I1mr, D1mr, D1mw, LLmr per handler): it sees (2) and (3); (b) on an x86 box (the TD-4 form): a perf pair per build (cycles, instructions, branch-misses, L1-icache-load-misses, iTLB-load-misses, per-symbol cycles): it sees (1) to (4) on real hardware.

## 3. The call (plan v18 §4.5, P18:269)

**JIT-DELTA prices no jit-only lever (no designer-jit run).**

- For the jit cells (P18:75): the jit half still has no designed lever. The month-14 and month-15 estimates stay unwritten. Even a cut of all the jit-only Ir is x1.0059 at the center, against a need of x1.041 / x1.042 on the jit cells. A jit-only Ir cut cannot close the jit half. The jit half needs levers in both builds, or a lever on the cycles that Ir does not see. This is a plan v19 decision, not settled here.
- JIT-REC (P18:257) stays "not priced": no row is near x1.015.

### Candidate ideas for plan v19 (each priced at its full group; a real cut is smaller)

| # | Idea | dIr bound (M) | est. low / center / high |
|---|---|---:|---|
| 1 | **Compare record lean:** a no-change test before the full `compare_bits` in `record_pair_next` (51 Ir per call now; the LEVER-8 form, which today covers only the `TestEqual` nullish arm), and less inlined std code in that nullish arm | 285.5 | x1.0006 / x1.0027 / x1.0055 |
| 2 | **Keyed cold record pre-test:** test the slot for "no new bit possible" before `keyed_key`, so `string::array_index` and the `kinds.rs` reads run only when a bit can change (82 Ir per cold entry now) | 219.5 | x1.0004 / x1.0021 / x1.0042 |
| 3 | **Window tests off the hot path:** `keyed_hit_kind` on the keyed hit and `call_window_after_unit` on each call (one budget compare each) | 171.0 | x1.0003 / x1.0016 / x1.0033 |

- All 3 at their bound: 676 M, x1.0065 at the center (D). Ideas 1 and 2 keep each fact equal (a cost cut only). Idea 3 changes when a fact is read: it needs a MODES design row before a box (P18:257).

## 4. Not verified

- The k values are the table script's model, not a measured cost per Ir on x86.
- The cause of the A/A difference is not proved.
- The "not seen by Ir" points use one box of the end read; the read-to-read move on the same code is -0.2% to +0.5% (jit) and +0.8% to +1.3% (jitless) (P18:119).
- One workload (octane-typescript), one target (linux-x64).
