# K1: regroup of the TD-3 box-2 cycle symbol tables by class (orchestrator, read only, 2026-11-25)

Source: TD-3 box 2 (ep9-x86-m12-td3-2, tree 2c5002c8), rec/<build>.cycles.{1,2,3}.sym.tsv: the listed symbols only (the tail under the list cutoff is not split). Median of 3 runs, % of all cycles samples (all threads). One class per symbol: the first class whose pattern matches, in the row order below. (M) for the listed symbols; the patterns are mine, read on the mangled names. The e-jitless default form has GC helpers on (H1).

| class | e-jitless | e-jit | node-jitless |
|---|---|---|---|
| kernel [k] | 1.05 | 1.02 | 0.56 |
| interp handlers (ep9h__) | 71.67 | 70.62 | 0.00 |
| GC (ep9_heap trace/parallel/sweep/collect/remembered) | 6.15 | 7.18 | 0.08 |
| alloc entry + slots grow | 1.34 | 1.34 | 0.00 |
| shape / props add, for-in | 0.93 | 0.95 | 0.00 |
| lookup_with | 1.01 | 1.04 | 0.00 |
| string + atoms | 1.82 | 2.16 | 0.00 |
| memcpy / memset / memmove | 0.00 | 0.06 | 0.15 |
| sha256 / hasher | 0.37 | 0.35 | 0.00 |
| front end / parser | 0.19 | 0.26 | 0.00 |
| interp slow paths + rest of ep9_vm | 8.23 | 8.17 | 0.00 |
| node / V8 symbols (node only) | 0.10 | 0.05 | 92.04 |
| listed total | 92.81 | 93.16 | 92.83 |
