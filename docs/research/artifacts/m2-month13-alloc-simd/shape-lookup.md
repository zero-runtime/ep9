# SHAPE-LOOKUP-COUNT: `shape::lookup_with` calls and a simulated (shape, key) cache, octane-typescript at 5c3e1e11

Role: counts-mac (slot A), 2026-11-25. Plan row: P18 §4.5 SHAPE-LOOKUP-COUNT (P18:270); f-design K4 (`f-design.md:252`).
Build: the K2 census build (`k2.md` §0; patch `census.patch.txt`, NOT committed; the worktree is clean again). Mac aarch64, H0. The counts do not depend on the CPU or the build: the jit census run gives the same call counts.

## 1. Method

- `lookup_with` gets `#[track_caller]`, and so do `shape::lookup` and the `props` wrappers that call it (`find_own`, `find_own_in`, `own_get`, `own_get_with`, `own_value_or_proto`, `own_replace`, `own_replace_with`, `own_store_if_writable`, `own_define`, `own_delete`). So a **site** = the first Rust caller outside these wrappers (40 sites in TS).
- Each call records: keys `n` of the shape; the work (key compares of the linear scan, `n <= LINEAR_MAX` = 8, or probes of the hash index); found or not; the context (`k2.md` §0).
- Simulated caches of (ShapeId, key) -> answer (a found index or "absent"; both kinds are cached, as V8's DescriptorLookupCache does):
  - per site: LRU of 1, 2 and 4 entries;
  - global: direct-mapped, 256 and 1,024 entries, index = (shape >> 3) xor (key x 0x9e3779b97f4a7c15, rotated 29).
- A hit "removes" the call. The work-weighted hit rate weighs each hit by its compares or probes.
- The census does not count its own lookups.

## 2. Counts (4 runs: C1 10 GCs; C2-C4 12 GCs)

| Count | C1 | C2 | C3 | C4 |
|---|---|---|---|---|
| `lookup_with` calls | 6,180,686 | 6,145,451 | 6,145,451 | 6,145,451 |
| found | 1,474,974 (23.9%) | 1,444,451 (23.5%) | = | = |
| work (compares + probes) | 16,634,967 | 16,787,155 | 16,385,779 | 16,450,263 |
| linear scan / hash index / empty shape | 3,102,051 / 2,900,441 / 178,194 | 3,115,152 / 2,851,735 / 178,564 | = | = |

Calls by keys `n` (C1): 0: 178,194; 1: 1,417,550; 2-8: 1,684,501; 9-16: 1,807,843; 17-64: 726,273; 65-512: 366,325.
The work moves a little from run to run (the hash index probe length depends on atom hashes); the call counts do not.

### The caller split (C2)

| Context | Calls | Share | Found | Work share | Global 1,024 hit rate |
|---|---|---|---|---|---|
| keyed load cold (`GetKeyedProperty`) | 3,110,660 | 50.6% | 18.6% | 46.9% | 85.7% |
| named store miss (`slow::set_named`) | 1,301,134 | 21.2% | 12.2% | 17.6% | 96.1% |
| keyed store cold (`SetKeyedProperty`) | 956,934 | 15.6% | 12.4% | 24.3% | 99.2% |
| for-in has-test (`for_in_has`) | 301,473 | 4.9% | 99.9% | 5.7% | 80.9% |
| other | 241,185 | 3.9% | 42.8% | 2.8% | 98.2% |
| named load miss (`slow::get_named`) | 234,065 | 3.8% | 78.2% | 2.6% | 86.8% |

### The top Rust sites (C1; line numbers of the patched tree)

| Site (function) | Calls | Per-site LRU hits 1 / 2 / 4 |
|---|---|---|
| `property.rs` `get_with_receiver` (the [[Get]] walk) | 3,539,560 | 141,204 / 285,622 / 1,317,941 |
| `property.rs` `ordinary_set_walk` (2 sites) | 773,929 + 357,687 | 89,362 / 123,597 / 199,106 and 95,568 / 162,829 / 215,622 |
| `ic.rs` `walk` (case build) | 376,852 | 4,030 / 7,866 / 67,211 |
| `property.rs` `has_own_with` (for-in) | 307,413 | 0 / 0 / 0 |
| `property.rs` `own_property` | 297,510 | 111,870 / 147,564 / 195,573 |
| `property.rs` `add_own_property` | 297,479 | 111,863 / 147,557 / 195,566 |
| `ic.rs` `store_pre`, `load_case`, `checked_case` | 116,704; 50,218; 50,105 | small |

76% of the calls do not find the key: most are steps of a prototype-chain walk. `own_property` and `add_own_property` look up the same (shape, key) twice per keyed add (297 k calls each).

## 3. Hit rates

| Cache | Call hit rate (C1 / C2 / C3 / C4) | Work-weighted hit rate |
|---|---|---|
| per site, 1 entry | 9.9% (all runs) | 14.3-14.7% |
| per site, 2 entries | 15.6% | 20.2-20.5% |
| per site, 4 entries | 37.7% | 44.3-45.1% |
| global, 256 entries | 83.2 / 82.0 / 82.1 / 83.1% | 84.4-85.7% |
| **global, 1,024 entries** | **90.3 / 90.3 / 90.2 / 90.3%** | **91.7-91.9%** |

A per-site cache is poor here: the hot sites (the [[Get]] walk, the set walk, the for-in has-test) see many shapes or many keys. A global cache keyed by (shape, key) catches the repeats across sites.

## 4. The SHAPE-LOOKUP-CACHE price

Inputs: `lookup_with` = 1.01% of TS cycles e-jitless, 1.04% e-jit (TD-3 box 2; K1 `k1-regroup.md:12`; 0.254 G of 24.36 G). Per call: 0.254 G / 6.15 M = 41 cycles (D). Unit price of a probe on every call (hit or miss): 5 / 8 / 10 cycles (G: a hash multiply, one load of a 16-24 KiB table, 2 compares).

| Case | Removed `lookup_with` work (hit rate x 1.01%) | Probe cost (6.15 M x 5-10 cycles) | Net | A/B |
|---|---|---|---|---|
| low (global 256, call rate 82%, probe 10) | 0.83% | 0.25% | 0.58% | **x1.006** |
| **center (global 1,024, work rate 91.8%, probe 8)** | **0.93%** | 0.20% | **0.73%** | **x1.007** |
| high (global 1,024, work rate 92.2%, probe 5) | 0.93% | 0.13% | 0.81% | **x1.008** |
| bound (100% hits, probe 0) | 1.01% | 0 | 1.01% | x1.010 |

- Against 1.0% of TS cycles: the cache removes at most **0.93 of the 1.0%** (gross), and about 0.73 net.
- Against the floor x1.015 (P18:253): the center is **x1.007 = 0.49 of the floor** (ln ratio). Even the bound (all hits, free probe) is x1.010. **PICK LINE NOT MET. SHAPE-LOOKUP-CACHE alone cannot reach the floor.** It is a set member only (same sign in both builds; jit gross 0.95%).
- Two cheaper parts the count shows (no cache): (a) `own_property` + `add_own_property` repeat one lookup per keyed add (297 k extra calls = about 4.8% of the calls); (b) the for-in has-test (301 k) is the L2 (c) path, and L2 is KILLED by its K2 count (`k2.md` §5). Neither is near the floor.
- Not verified: a real cache must flush when a shape changes in place (a UNIQUE or prototype shape) and at each GC (a freed shape id can come back). The simulation does neither, so its hit rate is an upper bound (10-12 GCs per run: a small loss, I).

## 5. The line-M read tool (P18 §4.5, second half): a route, not fixed here

Cause (read from the box 2 files): the TD-3 box 2 files `rec/e-jitless.cycles.{1,2,3}.ann.err` (not in the repo) each hold `annotate ep9h__ics__tail__GetNamedProperty rc=139` and `annotate (2) ep9h__ics__tail__GetNamedProperty rc=139`: `perf annotate` crashed (signal 11) on that handler in all 3 records. The handler has 9.4-9.7% of each record (the TD-3 box 2 summary, part M), so no record was usable and line M read NOT COMPUTED. The TD-3 summarizer (`summarize.mjs`, not in the repo) did what its rule says.

The fix is in the box script (the TD-3 box script `req.sh` `annot()`, line 242-251; not in the repo) and in the summarizer: when `perf annotate` fails, write the samples per address of the handler from `perf script -F ip,sym` (or `perf report --sort sym,srcline` with addresses), and let `summarize.mjs` read that form as the local percent; plus a self-test case (a crash file) and the dry run. It needs a box run to prove it. That is not a small `tools/` scratch fix, and this role may not run a box. **Route: LINE-M-READ-FIX** (opus; 0.5 agent-day; rides TD-4; P18 §4.5 row TD-4).
