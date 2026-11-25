// node --test .github/scripts/gc-count.test.mjs
// The GC count run of the bench mode (gc-count.mjs) with fake engines (written here), the fake
// valgrind of bench-set.test.mjs (tools/bench/cross/testdata/fake-valgrind.mjs) and a fake time
// tool: no ep9 build, no valgrind and no bench checkout needed (the rows are T0 files). ci.yml
// runs it in the bench-set job, in the step "gc count", on every leg but linux-arm64.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { BENCH_SET } from "./bench-set.mjs";
import {
  compareReads,
  decisionSig,
  fullMedian,
  median,
  O17_LIMIT,
  parseArgs,
  parseTrace,
  patchHeap,
  peakOld,
  renderMarkdown,
  SCHEMA,
  summarizeCell,
  TRACE_ANCHOR,
  tsLines,
} from "./gc-count.mjs";
import { TS_WORKLOAD } from "../../tools/bench/cross/workloads.mjs";

const DIR = path.dirname(fileURLToPath(import.meta.url));
const SCRIPT = path.join(DIR, "gc-count.mjs");
const ROOT = path.resolve(DIR, "../..");
const FAKE_VALGRIND = path.resolve(DIR, "../../tools/bench/cross/testdata/fake-valgrind.mjs");
const HEAD = "2".repeat(40);
const MIB = 1 << 20;

const gcb = (n, o = {}) =>
  `gcb n=${n} kind=${o.kind ?? "minor"} since=100 budget=${o.budget ?? 8 * MIB} old_before=${o.ob ?? MIB} old_after=${o.oa ?? 2 * MIB} marked=10 promoted=20 next_budget=${o.nb ?? 8 * MIB} next_major=false major_limit=${16 * MIB} hs=false`;

// ---- the trace print ---------------------------------------------------------------------------

test("the trace print goes around the after_gc line of the repo heap.rs, once", () => {
  const heap = readFileSync(path.join(ROOT, "crates/ep9_heap/src/heap.rs"), "utf8");
  assert.equal(heap.split(TRACE_ANCHOR).length - 1, 1, "the anchor line is in heap.rs exactly once");
  const p = patchHeap(heap);
  assert.equal(p.split(TRACE_ANCHOR).length - 1, 1, "the anchor line is kept");
  assert.ok(p.indexOf("let gcb_old_before = inner.old_bytes;") < p.indexOf(TRACE_ANCHOR), "old bytes are read before after_gc");
  assert.ok(p.indexOf(TRACE_ANCHOR) < p.indexOf('"gcb n={} kind={}'), "the print comes after after_gc");
  assert.equal(p.length - heap.length > 0, true);
  // Only lines are added: the base text is the patched text minus the added lines.
  const added = p.split("\n").filter((l) => !heap.split("\n").includes(l));
  assert.ok(added.every((l) => /gcb|eprintln|inner\.|minor|marked_bytes|promoted_bytes|^\s*\);$/.test(l)), added.join("\n"));
  assert.throws(() => patchHeap(p), /already has the trace print/);
  assert.throws(() => patchHeap("fn x() {}\n"), /0 times, not once/);
  assert.throws(() => patchHeap(TRACE_ANCHOR + TRACE_ANCHOR), /2 times, not once/);
});

test("the gcb lines of the print parse; a bad line is an error", () => {
  const t = parseTrace(`${gcb(1)}\nhello\n${gcb(2, { kind: "major", oa: 12 * MIB })}\nep9: gc collections=2 freed_cells=5 allocated_bytes=9\n`);
  assert.equal(t.collections, 2);
  assert.equal(t.gcs.length, 2);
  assert.deepEqual(t.gcs[1], { n: 2, kind: "major", since: 100, budget: 8 * MIB, old_before: MIB, old_after: 12 * MIB, marked: 10, promoted: 20, next_budget: 8 * MIB, next_major: false, major_limit: 16 * MIB, hs: false });
  assert.equal(peakOld(t.gcs), 12 * MIB);
  assert.equal(peakOld([]), 0);
  assert.equal(parseTrace("no gc here\n").collections, null);
  assert.throws(() => parseTrace(gcb(1).replace("budget=", "budget=x")), /bad gcb line \(budget\)/);
  assert.throws(() => parseTrace(gcb(1).replace("kind=minor", "kind=odd")), /bad gcb line \(kind\)/);
  assert.throws(() => parseTrace(gcb(1).replace("hs=false", "hs=1")), /bad gcb line \(hs\)/);
});

test("the decision sha reads the decision fields only", () => {
  const a = parseTrace(`${gcb(1)}\n${gcb(2)}`).gcs;
  const b = parseTrace(`${gcb(1, { ob: 3 * MIB, oa: 4 * MIB })}\n${gcb(2)}`).gcs;
  const c = parseTrace(`${gcb(1, { nb: 9 * MIB })}\n${gcb(2)}`).gcs;
  assert.equal(decisionSig(a), decisionSig(b), "old bytes are not a decision field");
  assert.notEqual(decisionSig(a), decisionSig(c), "the next budget is");
  assert.notEqual(decisionSig(a), decisionSig(a.slice(0, 1)), "the GC count is");
});

test("a cell: median, constant count, O17 at 11 MiB, decisions against arm 0", () => {
  assert.equal(median([12, 10, 13]), 12);
  assert.equal(median([12, 10]), 11);
  assert.equal(median([]), null);
  const run = (gcs, peak, sig = "s") => ({ status: "ok", gcs, majors: 1, peak_old: peak, sig });
  const arm0 = [run(12, 40 * MIB, "a"), run(11, 40 * MIB, "b")];
  const c = summarizeCell([run(6, 40 * MIB, "c"), run(6, 41 * MIB, "c")], arm0);
  assert.equal(c.median, 6);
  assert.equal(c.constant, true);
  assert.equal(c.o17, "FAIL");
  assert.equal(c.decisions_equal_arm0, false);
  const id = summarizeCell([run(5, O17_LIMIT, "x"), run(5, 0, "x")], [run(5, 1, "x")]);
  assert.equal(id.o17, "pass", "exactly 11 MiB passes");
  assert.equal(id.decisions_equal_arm0, true);
  assert.equal(summarizeCell([run(5, O17_LIMIT + 1, "x")], []).o17, "FAIL");
  const bad = summarizeCell([run(5, 0, "x"), { status: "error", error: "e" }], [run(5, 0, "x")]);
  assert.equal(bad.o17, "no reading", "a failed run gives no O17 reading");
  assert.equal(bad.decisions_equal_arm0, null);
  assert.equal(bad.counted, 1);
  // A failed arm-0 run: no compare with a part set of arm 0.
  assert.equal(summarizeCell([run(5, 0, "x")], [run(5, 0, "x"), { status: "error", error: "e" }]).decisions_equal_arm0, null);
  assert.equal(fullMedian(id), 5);
  assert.equal(fullMedian(bad), null, "a cell with a failed run has no full median");
  assert.equal(fullMedian(undefined), null);
});

test("the TS line: B1 at most 0.661 of arm 0", () => {
  const cell = (m, counted = 5) => ({ runs: 5, counted, median: m });
  const res = (c0, c1) => ({ forms: ["H1"], builds: [{ build: "jitless", rows: [{ name: TS_WORKLOAD, arms: { 0: { H1: cell(c0) }, 1: { H1: cell(c1) } } }] }] });
  assert.equal(tsLines(res(12, 7))[0].word, "pass");
  assert.equal(tsLines(res(12, 8))[0].word, "FAIL");
  assert.equal(tsLines(res(11, 7))[0].word, "pass");
  assert.deepEqual(tsLines(res(12, 6))[0], { build: "jitless", form: "H1", arm0: 12, b1: 6, ratio: 0.5, e: 0.0221, word: "pass" });
  assert.equal(tsLines(res(null, 6))[0].word, "no reading");
  // A failed run in the B1 cell: no TS reading, not the median of the 4 runs that counted.
  const part = res(12, 6);
  part.builds[0].rows[0].arms[1].H1.counted = 4;
  assert.equal(tsLines(part)[0].word, "no reading");
});

// ---- options -----------------------------------------------------------------------------------

test("options: the default rows are octane-typescript and the BENCH-SET; usage errors", () => {
  const o = parseArgs(["--trace-ep9", "t", "--out", "x.json"]);
  assert.deepEqual(o.workloads, [TS_WORKLOAD, ...BENCH_SET]);
  assert.equal(o.workloads.length, 20);
  assert.deepEqual(o.arms, ["0", "1", "2"]);
  assert.deepEqual(o.irArms, ["0", "1", "2"]);
  assert.deepEqual(o.forms, ["H1", "H0"]);
  assert.deepEqual(parseArgs(["--trace-ep9", "t", "--out", "x", "--ir-arms", "none"]).irArms, []);
  assert.deepEqual(parseArgs(["--patch", "h.rs"]).patch, path.resolve("h.rs"));
  assert.deepEqual(parseArgs(["--trace-ep9", "t", "--out", "x", "--workloads", `${TS_WORKLOAD},octane-splay,${TS_WORKLOAD}`, "--rss-rows", "none"]).workloads, [TS_WORKLOAD, "octane-splay"], "a name twice is one row");
  assert.deepEqual(parseArgs(["--trace-ep9", "t", "--out", "x", "--workloads", "v8v7-7"]).workloads.length, 7, "a set name expands");
  for (const [argv, re] of [
    [["--out", "x"], /--trace-ep9 or --trace-ep9-jitless/],
    [["--trace-ep9", "t"], /--out is required/],
    [["--trace-ep9", "t", "--out", "x", "--arms", "1,2"], /must hold arm 0/],
    [["--trace-ep9", "t", "--out", "x", "--arms", "0,3"], /bad list/],
    [["--trace-ep9", "t", "--out", "x", "--arms", "0,1", "--ir-arms", "2"], /a part of --arms/],
    [["--trace-ep9", "t", "--out", "x", "--forms", "H2"], /bad list/],
    [["--trace-ep9", "t", "--out", "x", "--head", "abc"], /full commit id/],
    [["--trace-ep9", "t", "--out", "x", "--workloads", "nope"], /unknown workload nope/],
    [["--trace-ep9", "t", "--out", "x", "--workloads", "a,,b"], /bad list/],
    [["--trace-ep9", "t", "--out", "x", "--workloads", "t0-loop"], /--rss-rows must be a part/],
    [["--patch", "a", "--compare", "b", "c"], /two modes/],
    [["--runs", "0"], /integer >= 1/],
    [["--bogus"], /unknown option/],
  ]) {
    assert.throws(() => parseArgs(argv), re, argv.join(" "));
  }
});

// ---- the whole run with fake engines -----------------------------------------------------------

// The fake engine: one file, one symlink per binary name. A name with "trace" prints one gcb line
// per GC; the GC count is FAKE_GCS[<EP9_GC_BUDGET>] (JSON; default 3), the old bytes after each
// GC FAKE_OLD_MIB MiB. --version prints the build line of FAKE_PRESET and FAKE_COMMIT (dirty for
// a trace name). FAKE_LOG gets "<name> <workload> arm=<..> helpers=<..> seed=<..>".
const FAKE = `#!/usr/bin/env node
const fs = require("node:fs");
const path = require("node:path");
const name = path.basename(process.argv[1]);
const trace = name.includes("trace");
const args = process.argv.slice(2);
if (args[0] === "--version") {
  console.log("fake " + name + " 0.0");
  console.log(process.env.FAKE_PRESET === "jit" ? "features jit; preset jit" : "features none; preset jitless");
  console.error("ep9 build: commit " + process.env.FAKE_COMMIT + (trace ? " dirty" : " clean"));
  process.exit(0);
}
const file = args[args.length - 1];
const w = path.basename(file, ".js");
const arm = process.env.EP9_GC_BUDGET ?? "unset";
if (process.env.FAKE_LOG) fs.appendFileSync(process.env.FAKE_LOG, name + " " + w + " arm=" + arm + " helpers=" + (process.env.EP9_GC_HELPERS ?? "unset") + " seed=" + process.env.EP9_HASH_SEED + "\\n");
if (process.env.FAKE_FAIL === name + ":" + w) { console.error("boom"); process.exit(3); }
const m = /^const want = "?([^";]*)"?;/m.exec(fs.readFileSync(file, "utf8"));
if (m) console.log(m[1]);
const gcs = (JSON.parse(process.env.FAKE_GCS ?? "{}"))[arm] ?? 3;
const old = Number(process.env.FAKE_OLD_MIB ?? "2") * 1048576;
if (trace) for (let n = 1; n <= gcs; n++) console.error("gcb n=" + n + " kind=minor since=1 budget=8388608 old_before=" + old + " old_after=" + old + " marked=1 promoted=1 next_budget=" + (arm === "0" ? 8388608 : 8388608 + Number(arm)) + " next_major=false major_limit=16777216 hs=false");
if (args.includes("--gc-stats")) console.error("ep9: gc collections=" + (process.env.FAKE_STATS ?? gcs) + " freed_cells=1 allocated_bytes=2");
`;
const FAKE_TIME = `#!/usr/bin/env node
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const a = process.argv.slice(2);
let out = null;
let i = 0;
for (; i < a.length && a[i].startsWith("-"); i++) {
  if (a[i] === "-o") out = a[++i];
  else if (a[i] === "-f") i++;
}
const r = spawnSync(a[i], a.slice(i + 1), { stdio: "inherit" });
fs.writeFileSync(out, "4096\\n     4194304  maximum resident set size\\n");
process.exit(r.status ?? 1);
`;

function setup(t) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "ep9-gc-count-test-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const fake = path.join(dir, "fake-engine.cjs");
  writeFileSync(fake, FAKE);
  chmodSync(fake, 0o755);
  for (const n of ["ep9-trace", "ep9-jitless-trace", "ep9-head", "ep9-jitless-head"]) symlinkSync(fake, path.join(dir, n));
  const time = path.join(dir, "fake-time.cjs");
  writeFileSync(time, FAKE_TIME);
  chmodSync(time, 0o755);
  return { dir, time };
}

function runScript(args, env) {
  return spawnSync(process.execPath, [SCRIPT, ...args], { cwd: ROOT, encoding: "utf8", env: { ...process.env, ...env } });
}

test("a run with fake engines: counts per arm and form, O17, RSS, Ir, the TS-free table", { skip: process.platform === "win32" && "POSIX only" }, (t) => {
  const { dir, time } = setup(t);
  const log = path.join(dir, "log.txt");
  const out = path.join(dir, "gc.json");
  const md = path.join(dir, "gc.md");
  const r = runScript(
    ["--trace-ep9-jitless", path.join(dir, "ep9-jitless-trace"), "--ep9-jitless", path.join(dir, "ep9-jitless-head"), "--head", HEAD,
      "--workloads", "t0-loop,t0-call", "--rss-rows", "t0-call", "--rss-runs", "2", "--runs", "2", "--forms", "H1,H0,HD",
      "--valgrind", FAKE_VALGRIND, "--jobs", "2", "--out", out, "--md", md],
    { FAKE_COMMIT: HEAD, FAKE_GCS: JSON.stringify({ 0: 4, 1: 2, 2: 2 }), FAKE_LOG: log, EP9_MEM_TIME: time, EP9_GC_HELPERS: "7", EP9_GC_BUDGET: "2" },
  );
  assert.equal(r.status, 0, r.stderr);
  const res = JSON.parse(readFileSync(out, "utf8"));
  assert.equal(res.schema, SCHEMA);
  assert.equal(res.errors, 0);
  assert.equal(res.head, HEAD);
  assert.equal(res.builds.length, 1);
  const b = res.builds[0];
  assert.equal(b.build, "jitless");
  assert.deepEqual(b.rows.map((x) => x.name), ["t0-loop", "t0-call"]);
  const loop = b.rows[0];
  assert.equal(loop.arms["0"].H1.median, 4);
  assert.equal(loop.arms["1"].H0.median, 2);
  assert.deepEqual(loop.arms["2"].HD.counts, [2, 2]);
  assert.equal(loop.arms["1"].H1.o17, "pass");
  assert.equal(loop.arms["1"].H1.decisions_equal_arm0, false);
  assert.equal(loop.arms["0"].H1.decisions_equal_arm0, true);
  assert.deepEqual(loop.arms["0"].H1.rss, [4 * MIB, 4 * MIB]);
  assert.equal(loop.rss, null);
  assert.deepEqual(b.rows[1].rss["1"], { bytes: [4 * MIB, 4 * MIB], median: 4 * MIB });
  // fake valgrind: Ir by the workload name, the same in each arm.
  assert.deepEqual(loop.ir, { 0: 1_000_007_000, 1: 1_000_007_000, 2: 1_000_007_000 });
  assert.deepEqual(loop.ir_rel_arm0, { 0: 0, 1: 0, 2: 0 });
  assert.deepEqual(res.ts, [], "no TS row, no TS line");
  // The env of each run: the arm, the form's helpers (HD: not set, over the caller's 7), seed 1.
  const lines = readFileSync(log, "utf8").trim().split("\n");
  assert.ok(lines.includes("ep9-jitless-trace t0-loop arm=1 helpers=1 seed=1"));
  assert.ok(lines.includes("ep9-jitless-trace t0-loop arm=2 helpers=0 seed=1"));
  assert.ok(lines.includes("ep9-jitless-trace t0-loop arm=0 helpers=unset seed=1"));
  assert.ok(lines.includes("ep9-jitless-head t0-call arm=1 helpers=unset seed=1"), "RSS runs: the host default");
  assert.ok(lines.includes("ep9-jitless-head t0-loop arm=2 helpers=0 seed=1"), "Ir runs: helpers 0, as bench-set.mjs");
  // count: 2 rows x 3 arms x 3 forms x 2 runs; RSS: 1 row x 3 arms x 2; Ir: 2 rows x 3 arms.
  assert.equal(lines.filter((l) => l.startsWith("ep9-jitless-trace ")).length, 36);
  assert.equal(lines.filter((l) => l.startsWith("ep9-jitless-head ")).length, 6 + 6);
  const text = readFileSync(md, "utf8");
  assert.equal(text, renderMarkdown(res));
  assert.match(text, /\| t0-loop \| 4\/4\/4 \| 2\/2\/2 \| 2\/2\/2 \| 2\.0 \| pass \| 1:NO\/NO\/NO 2:NO\/NO\/NO \| \+0\.0000% \| \+0\.0000% \|/);
  assert.match(text, /RSS \(G7 line\) t0-call, jitless, plain binary, median peak RSS: arm 0 4\.0 MiB; arm 1 4\.0 MiB \(x1\.000\)/);
});

test("O17 FAIL, a trace count that differs from collections, a failed run: exit 1", { skip: process.platform === "win32" && "POSIX only" }, (t) => {
  const { dir, time } = setup(t);
  const out = path.join(dir, "gc.json");
  const base = ["--trace-ep9-jitless", path.join(dir, "ep9-jitless-trace"), "--workloads", "t0-loop", "--rss-rows", "none", "--ir-arms", "none", "--runs", "1", "--forms", "H0", "--out", out];
  let r = runScript(base, { FAKE_COMMIT: HEAD, FAKE_OLD_MIB: "12", EP9_MEM_TIME: time });
  assert.equal(r.status, 0, r.stderr);
  let res = JSON.parse(readFileSync(out, "utf8"));
  assert.equal(res.builds[0].rows[0].arms["0"].H0.o17, "FAIL");
  assert.equal(res.builds[0].rows[0].arms["0"].H0.peak_old, 12 * MIB);
  assert.equal(res.builds[0].rows[0].ir, null);
  r = runScript(base, { FAKE_COMMIT: HEAD, FAKE_STATS: "9", EP9_MEM_TIME: time });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /3 gcb lines, but collections=9 \(not a trace binary\?\)/);
  res = JSON.parse(readFileSync(out, "utf8"));
  assert.equal(res.errors, 3);
  assert.equal(res.builds[0].rows[0].arms["0"].H0.o17, "no reading");
  r = runScript(base, { FAKE_COMMIT: HEAD, FAKE_FAIL: "ep9-jitless-trace:t0-loop", EP9_MEM_TIME: time });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /exit 3: boom/);
});

test("the build lines: preset, --head commit, a dirty plain binary: exit 2", { skip: process.platform === "win32" && "POSIX only" }, (t) => {
  const { dir, time } = setup(t);
  const out = path.join(dir, "gc.json");
  const args = (extra) => ["--trace-ep9-jitless", path.join(dir, "ep9-jitless-trace"), "--workloads", "t0-loop", "--rss-rows", "none", "--runs", "1", "--forms", "H0", "--head", HEAD, "--out", out, ...extra];
  let r = runScript(args(["--ir-arms", "none"]), { FAKE_COMMIT: "3".repeat(40), EP9_MEM_TIME: time });
  assert.equal(r.status, 2);
  assert.match(r.stderr, /names commit 3{40}, not --head/);
  r = runScript(args(["--ir-arms", "none"]), { FAKE_COMMIT: HEAD, FAKE_PRESET: "jit", EP9_MEM_TIME: time });
  assert.equal(r.status, 2);
  assert.match(r.stderr, /is not the jitless build/);
  // A plain binary that says dirty (here: a trace binary given as the plain one).
  r = runScript(args(["--ep9-jitless", path.join(dir, "ep9-jitless-trace"), "--valgrind", FAKE_VALGRIND]), { FAKE_COMMIT: HEAD, EP9_MEM_TIME: time });
  assert.equal(r.status, 2);
  assert.match(r.stderr, /plain binary was built with uncommitted engine changes/);
});

test("--patch writes the print into a heap.rs copy", (t) => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "ep9-gc-count-patch-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const f = path.join(dir, "heap.rs");
  writeFileSync(f, `fn a() {\n${TRACE_ANCHOR}}\n`);
  let r = runScript(["--patch", f]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(readFileSync(f, "utf8"), /eprintln!\(\n\s+"gcb n=/);
  r = runScript(["--patch", f]);
  assert.equal(r.status, 2);
  assert.match(r.stderr, /already has the trace print/);
});

// ---- the merge check ---------------------------------------------------------------------------

// The merge-check fixture: both builds, the 20 rows (octane-typescript and the 19 BENCH-SET rows),
// forms H1 and H0 (or `forms`), 3 counted runs per cell, one Ir per row, a sha256 per row, and the
// build lines of the head (trace binary dirty, plain binary clean).
const MC_ROWS = [TS_WORKLOAD, ...BENCH_SET];
const RH = "a".repeat(40);
const MH = "b".repeat(40);
const HEADS = { readHead: RH, mergeHead: MH };
function mcDoc(head, arm, { forms = ["H1", "H0"], builds = ["jit", "jitless"], rows = MC_ROWS, gcs = {}, ir = {}, sha = {} } = {}) {
  const cell = (med) => ({ runs: 3, counted: 3, median: med });
  return {
    schema: SCHEMA,
    head,
    forms,
    builds: builds.map((build) => ({
      build,
      trace_ep9: { path: "ep9-trace", build: { commit: head, dirty: true } },
      plain_ep9: { path: "ep9-head", build: { commit: head, dirty: false } },
      rows: rows.map((name, i) => ({
        name,
        sha256: name in sha ? sha[name] : `sha-${name}`,
        arms: { [arm]: Object.fromEntries(forms.map((f) => [f, cell(gcs[name] ?? (name === TS_WORKLOAD ? 6 : 5))])) },
        ir: { [arm]: ir[name] ?? 1_000_000_000 + i * 1_000 },
      })),
    })),
  };
}
const cmp = (read, merge, o = {}) => compareReads(read, merge, { ...HEADS, ...o });
const headArgs = (r = RH, m = MH) => ["--read-head", r, "--merge-head", m];

test("the merge check: GC counts equal to the read's B1, Ir within 1e-5", (t) => {
  assert.equal(MC_ROWS.length, 20);
  const read = mcDoc(RH, "1");
  let merge = mcDoc(MH, "0", { ir: { [TS_WORKLOAD]: 1_000_010_000 } });
  let c = cmp(read, merge);
  assert.equal(c.verdict, "PASS", JSON.stringify(c));
  assert.equal(c.rows.length, 40);
  assert.deepEqual(c.missing, []);
  assert.deepEqual(c.heads, []);
  merge = mcDoc(MH, "0", { ir: { [TS_WORKLOAD]: 1_000_010_001 } });
  c = cmp(read, merge);
  assert.equal(c.verdict, "FAIL");
  assert.equal(c.rows[0].word, "FAIL");
  merge = mcDoc(MH, "0", { gcs: { [TS_WORKLOAD]: 7 } });
  assert.equal(cmp(read, merge).rows[0].word, "FAIL", "a GC count that differs");
  merge = mcDoc(MH, "0", { rows: [TS_WORKLOAD] });
  c = cmp(read, merge);
  assert.equal(c.verdict, "INCOMPLETE", "a row with no merge reading");
  assert.ok(c.missing.includes("merge: jit splay"), c.missing.join("; "));
  // A merge cell with a failed run is no reading, even when its median equals the read.
  merge = mcDoc(MH, "0");
  merge.builds[0].rows[1].arms["0"].H0.counted = 2;
  c = cmp(read, merge);
  assert.equal(c.rows[1].word, "no reading");
  assert.equal(c.verdict, "INCOMPLETE");
  // The CLI form: a correct pair passes.
  const dir = mkdtempSync(path.join(os.tmpdir(), "ep9-gc-count-cmp-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  writeFileSync(path.join(dir, "r.json"), JSON.stringify(read));
  writeFileSync(path.join(dir, "m.json"), JSON.stringify(mcDoc(MH, "0")));
  const r = runScript(["--compare", path.join(dir, "r.json"), path.join(dir, "m.json"), ...headArgs()]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, new RegExp(`Merge head = read code: PASS \\(read ${RH} arm 1; merge ${MH} arm 0\\)`));
  writeFileSync(path.join(dir, "m1.json"), JSON.stringify(mcDoc(MH, "0", { forms: ["H0"] })));
  const r1 = runScript(["--compare", path.join(dir, "r.json"), path.join(dir, "m1.json"), ...headArgs()]);
  assert.equal(r1.status, 1, r1.stdout);
  assert.match(r1.stdout, /INCOMPLETE/);
  assert.match(r1.stdout, /- missing \(INCOMPLETE\): merge: form H1/);
  writeFileSync(path.join(dir, "x.json"), "{}");
  assert.equal(runScript(["--compare", path.join(dir, "x.json"), path.join(dir, "m.json"), ...headArgs()]).status, 2);
  // The 2 heads are required, as full commit ids.
  for (const [extra, re] of [
    [[], /--read-head and --merge-head are required/],
    [["--read-head", RH], /--read-head and --merge-head are required/],
    [headArgs(RH, "abc"), /--merge-head needs a full commit id/],
  ]) {
    const x = runScript(["--compare", path.join(dir, "r.json"), path.join(dir, "m.json"), ...extra]);
    assert.equal(x.status, 2, x.stdout);
    assert.match(x.stderr, re);
  }
});

test("the merge check: a merge run without a form of the read is INCOMPLETE (Codex r1, 1)", () => {
  // The read has H1 and H0; the merge run has H0 only, with H0 counts equal and Ir in the band.
  const c = cmp(mcDoc(RH, "1"), mcDoc(MH, "0", { forms: ["H0"] }));
  assert.equal(c.verdict, "INCOMPLETE", JSON.stringify(c.rows[0]));
  assert.deepEqual(c.rows[0].counts.map((x) => x.form), ["H1", "H0"]);
  assert.equal(c.rows[0].word, "no reading");
});

test("the merge check: an H0-only read against a full merge run is not PASS (Codex r2, 1)", () => {
  // The read was made with --forms H0; H0 counts, sha256 and Ir agree with the merge run.
  let c = cmp(mcDoc(RH, "1", { forms: ["H0"] }), mcDoc(MH, "0"));
  assert.notEqual(c.verdict, "PASS", JSON.stringify(c.missing));
  assert.equal(c.verdict, "INCOMPLETE");
  assert.ok(c.missing.includes("read: form H1"), c.missing.join("; "));
  assert.deepEqual(c.rows[0].counts.map((x) => x.form), ["H1", "H0"]);
  // H0 and H1 on both sides, and every other form of the read (here HD) on the merge side.
  c = cmp(mcDoc(RH, "1", { forms: ["H1", "H0", "HD"] }), mcDoc(MH, "0"));
  assert.equal(c.verdict, "INCOMPLETE");
  assert.ok(c.missing.includes("merge: form HD"), c.missing.join("; "));
  c = cmp(mcDoc(RH, "1", { forms: ["H1"] }), mcDoc(MH, "0", { forms: ["H1"] }));
  assert.equal(c.verdict, "INCOMPLETE");
  assert.deepEqual(c.missing.filter((m) => m.includes("form")), ["read: form H0", "merge: form H0"]);
});

test("the merge check: a report of another head is FAIL (Codex r2, 2)", () => {
  const read = mcDoc(RH, "1");
  // A stale merge artifact: its measurements agree, but it names another commit.
  const stale = "c".repeat(40);
  let c = cmp(read, mcDoc(stale, "0"));
  assert.equal(c.verdict, "FAIL", JSON.stringify(c.heads));
  assert.ok(c.heads.includes(`merge: report head ${stale}, not --merge-head ${MH}`), c.heads.join("; "));
  c = cmp(mcDoc(stale, "1"), mcDoc(MH, "0"));
  assert.equal(c.verdict, "FAIL");
  assert.ok(c.heads.includes(`read: report head ${stale}, not --read-head ${RH}`), c.heads.join("; "));
  // The report head is right, but a binary build line names another commit, or is not there.
  let merge = mcDoc(MH, "0");
  merge.builds[1].plain_ep9.build.commit = stale;
  c = cmp(read, merge);
  assert.equal(c.verdict, "FAIL");
  assert.ok(c.heads.includes(`merge: jitless plain binary names commit ${stale}, not ${MH}`), c.heads.join("; "));
  merge = mcDoc(MH, "0");
  merge.builds[0].trace_ep9 = null;
  c = cmp(read, merge);
  assert.equal(c.verdict, "FAIL");
  assert.ok(c.heads.includes(`merge: jit trace binary names commit none, not ${MH}`), c.heads.join("; "));
  // A plain binary with uncommitted engine changes.
  merge = mcDoc(MH, "0");
  merge.builds[0].plain_ep9.build.dirty = true;
  c = cmp(read, merge);
  assert.equal(c.verdict, "FAIL");
  assert.ok(c.heads.includes("merge: jit plain binary is not clean"), c.heads.join("; "));
  // No head given: no PASS.
  c = compareReads(read, mcDoc(MH, "0"));
  assert.equal(c.verdict, "FAIL");
  assert.ok(c.heads.includes("--read-head and --merge-head are required"), c.heads.join("; "));
});

test("the merge check: the same report in both slots is refused (Codex r2, 2)", (t) => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "ep9-gc-count-same-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  // A full read report with the B1 arm and the arm-0 counts equal: in both slots with --merge-arm 1
  // every row compares with itself.
  const read = mcDoc(RH, "1");
  const f = path.join(dir, "r.json");
  const copy = path.join(dir, "r-copy.json");
  writeFileSync(f, JSON.stringify(read));
  writeFileSync(copy, JSON.stringify(read));
  for (const [args, re] of [
    [[f, f, ...headArgs(), "--merge-arm", "1"], /the same report in both slots \(one file\)/],
    [[f, copy, ...headArgs(), "--merge-arm", "1"], /the same report in both slots \(equal bytes\)/],
    [[f, copy, ...headArgs(RH, RH), "--merge-arm", "1"], /the same report in both slots \(one head/],
  ]) {
    const r = runScript(["--compare", ...args]);
    assert.equal(r.status, 2, r.stdout);
    assert.match(r.stderr, re);
    assert.doesNotMatch(r.stdout, /PASS/);
  }
  // The same document (other bytes) in both slots: the function says FAIL, not PASS.
  const c = compareReads(read, structuredClone(read), { readHead: RH, mergeHead: RH, mergeArm: "1" });
  assert.equal(c.verdict, "FAIL");
  assert.ok(c.heads.some((h) => h.startsWith("the same report in both slots")), c.heads.join("; "));
});

// The real r1 read of the read head (x86-64 count run): set GC_COUNT_R1_READ to its JSON file.
const R1 = process.env.GC_COUNT_R1_READ;
test("the merge check: the real r1 read against itself is refused as the same report (Codex r2, 2)", { skip: !R1 && "GC_COUNT_R1_READ not set" }, (t) => {
  const doc = JSON.parse(readFileSync(R1, "utf8"));
  assert.equal(doc.schema, SCHEMA);
  const head = doc.head;
  assert.match(head, /^[0-9a-f]{40}$/);
  const dir = mkdtempSync(path.join(os.tmpdir(), "ep9-gc-count-r1-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const copy = path.join(dir, "copy.json");
  writeFileSync(copy, readFileSync(R1));
  // The right read head; as merge head the report head (one head) or another commit.
  for (const [b, heads, re] of [
    [R1, headArgs(head, MH), /the same report in both slots \(one file\)/],
    [copy, headArgs(head, MH), /the same report in both slots \(equal bytes\)/],
    [copy, headArgs(head, head), /the same report in both slots \(one head/],
  ]) {
    const r = runScript(["--compare", R1, b, ...heads, "--merge-arm", "1"]);
    assert.equal(r.status, 2, r.stdout);
    assert.match(r.stderr, re);
  }
  // As a document: the function says FAIL (one head) and never PASS.
  const c = compareReads(doc, structuredClone(doc), { readHead: head, mergeHead: head, mergeArm: "1" });
  assert.equal(c.verdict, "FAIL");
});

test("the merge check: a read with fewer rows or one build is INCOMPLETE (Codex r1, 1)", () => {
  // A read of octane-typescript only (a bench_workloads run): it does not define the check set.
  let c = cmp(mcDoc(RH, "1", { rows: [TS_WORKLOAD] }), mcDoc(MH, "0"));
  assert.equal(c.verdict, "INCOMPLETE", JSON.stringify(c.rows));
  // A read of the jitless build only.
  c = cmp(mcDoc(RH, "1", { builds: ["jitless"] }), mcDoc(MH, "0"));
  assert.equal(c.verdict, "INCOMPLETE", JSON.stringify(c.rows.slice(0, 2)));
  // A merge run of the jit build only.
  c = cmp(mcDoc(RH, "1"), mcDoc(MH, "0", { builds: ["jit"] }));
  assert.equal(c.verdict, "INCOMPLETE");
});

test("the merge check: a workload sha256 that differs is FAIL; no sha256 is no reading (Codex r1, 1)", () => {
  let c = cmp(mcDoc(RH, "1", { sha: { splay: "x" } }), mcDoc(MH, "0", { sha: { splay: "y" } }));
  assert.equal(c.verdict, "FAIL", JSON.stringify(c.rows.filter((r) => r.name === "splay")));
  assert.deepEqual(
    c.rows.filter((r) => r.word !== "pass").map((r) => `${r.build} ${r.name} ${r.word}`),
    ["jit splay FAIL", "jitless splay FAIL"],
  );
  c = cmp(mcDoc(RH, "1", { sha: { splay: null } }), mcDoc(MH, "0"));
  assert.equal(c.verdict, "INCOMPLETE");
  assert.equal(c.rows.find((r) => r.name === "splay").word, "no reading");
});
