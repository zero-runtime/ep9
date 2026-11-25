VERDICT: usable
counts-check (read only), 2026-11-25. Every read holds. The faults below are in the records, not the reads.

(a) Numbers, commands, raw sources, units
- OK: K0 rows are in k0/ (grow 1,467,508; N4 11,200,750 + 152,327 = 11,353,077; load totals 357,994,202 both). K5 means: -8,107 B = -0.002%. Both builds read in K0, K2 (C5) and SHAPE-LOOKUP. Units are right (cells, B per run).
- OK: sim-l1.txt has the net rows (50,392,848 B at slack 1). Global 1,024 hit rate 5,549,886 / 6,145,451 = 90.3%. The price is right: 0.918 x 1.01% - 0.20% = 0.73%, so x1.007.
- OK: the census counts the K2 rows of f-design:250. The chain rule matches f-design:94 (no derived ctor, stop at host, dict or 8 levels; cap only if n > GENEROUS). Named-load hits sum to 357.85 M; ic-stats gives 357.99 M.
- F1: there is no raw file for C1. The run was overwritten. These numbers have no source: k2 §1 table (6,485,145 cells), SLOTS 1,487,925, own_add 277,466, dict 22,855, and shape-lookup C1 (calls, calls by n, top-site table). C2 raw has the same rows within 0.4%. No read uses a number that only C1 has.
- F2: k2 §3 "IC add hit 15,801,256" is not in any raw file. C2 and C5 show 15,800,221.
- F3: census TOTAL is 420.4 MB, but gc-stats in the census build is 425.8 MB. The 1.3% gap has no note. The plain build gives 420.6 MB. With 425.8 MB as the base, the slack-1 fall is 11.8%, which is still >= 10%.
- F4 (near form): "fields added after the constructor returns" is counted as "names outside the chain". For ESCAPE_SLACK this is the right measure. It is a near form; the file must say so.

(b) Kill and pick reads (quotes from f-design)
- K0: "`store grow` < 1.0 M: L1 falls under its low" (:248). 1,467,508: ALIVE. OK.
- L1: "grows covered by the chain rule < 1.0 M of 1.47 M; ... byte fall < 5%" (:105). The pick needs "Mac wall preview >= x1.015 ... bytes fall >= 10% and grows covered >= 1.2 M".
  - F5: the k2 §5 row gives "covered" as net cells removed (1.394 M). The covered count is 1,369,767 / 1,395,702 / 1,425,582 of 1,481,595 SLOTS growths. That base is all paths, not the 1.47 M store grows. The read does not change.
  - F6: the 12.0% byte fall is a simulation (D). At slack 0, the f-design default, it is 9.6% and the pick part fails. ":97 the count K2 picks it" allows slack 1. K3 must measure it.
- L2: "< 0.3 M kills (a); < 2 M key visits kills (b), (c)" (:124). 7,544 and 360,310: KILLED. OK.
- L4: "< 0.15 M kills (a); < 0.3 M kills (b)" (:156). 538,940 and 288,455: (a) alive, (b) killed. OK.
- L5: "Kill for M2: < 8 M cells" (:170). 6.49 M: KILLED. OK.
- L3, L6, L7: correctly marked not read. L6 "off the waited path" is not read. K1 does not split the sweep or the L4 symbols.
- K5: "< 2% more bytes in the jit build: none". -0.002%. OK.
- SHAPE-LOOKUP: P18 §4.4 needs >= x1.015. x1.007: NOT MET. OK. The line-M fix is only a route (LINE-M-READ-FIX). The §4.5 output "line M readable at TD-4" is still open.

(c) Worktree
- HEAD is 5c3e1e11, the same as origin/m2/m13. origin..HEAD is empty. `git status --porcelain` is empty, census.rs is gone, and `git apply --check census.patch` passes. No commit. OK.

(d) JIT-DELTA
- No head commit is needed. ci.yml at 5c3e1e11 has bench_workloads. bench-set.mjs checks only the 40-hex form, so base = head is accepted. The rebuild guard passes with 0 differ. Both builds are made, x64 runs with --branch-sim, and keep names are <side>.<workload>.out under bench/cg and bench/cg-jitless.
- The artifact bench-set-linux-x64-<head> holds both json files and both cg dirs. The table script reads that cachegrind format (events, fl=, fn=, summary). The dispatch command is right. No file changed.
