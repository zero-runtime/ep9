#!/usr/bin/env node
// The GC count run of the bench mode (ci.yml job bench-set, step "gc count"; route GC-X86-COUNT,
// plan v18 section 4.2, G-4 and G-6). A COUNT RUN: it reads counts only and is never a timed
// run. On x86-64 Linux it gives, per build, workload row and arm of the GC-BUDGET probe switch
// (EP9_GC_BUDGET = 0, 1 = B1, 2 = B2; crates/ep9_heap/src/heap.rs gc_budget_arm):
//
//   - the GC count and the per-GC trace of a trace binary (the head tree with one print per GC,
//     --patch below), per helper form, several runs;
//   - the peak old bytes per run (O17: a GC lever's PROOF (c) needs at most 11 MiB in every
//     count run of an IDENTITY row) and whether the GC decisions of the arm equal arm 0;
//   - the peak RSS of the --rss-rows (default earley-boyer: the G7 line) with the plain binary;
//   - the cachegrind Ir of the plain binary per row and arm (--ir-arms), in the bench-set.mjs
//     form (EP9_HASH_SEED, EP9_GC_HELPERS=0, --cache-sim=no), so that a later read can check
//     "the merge head equals the read code" (--compare);
//   - the TS line: count(B1) / count(arm 0) of octane-typescript, against 0.661 (r24-offloop.md
//     section 3.5 the E line: E = 0.0443 x (1 - ratio) >= 0.015).
//
//   node .github/scripts/gc-count.mjs --patch <tree>/crates/ep9_heap/src/heap.rs
//   node .github/scripts/gc-count.mjs --trace-ep9 T --trace-ep9-jitless TL --ep9 P --ep9-jitless PL \
//     [--head SHA] [--target linux-x64] [--workloads LIST] [--arms 0,1,2] [--ir-arms 0,1,2|none] \
//     [--forms H1,H0] [--runs 3] [--ts-runs 5] [--rss-rows earley-boyer] [--rss-runs 5] \
//     [--hash-seed 1] [--jobs N] [--timeout-s 900] [--valgrind PATH] [--keep DIR] [--work DIR] \
//     --out gc-count.json [--md FILE]
//   node .github/scripts/gc-count.mjs --compare READ.json MERGE.json [--read-arm 1] [--merge-arm 0]
//
//   --patch FILE      writes the trace print into heap.rs (one `gcb ...` line on stderr per GC,
//                     after Inner::after_gc). Only for a copy of the tree in its own build dir:
//                     a print never goes into a timed binary (AB form PROOF (c)). Exit 2 when the
//                     anchor line is not there exactly once (the step stops; no silent count)
//   --trace-ep9 / --trace-ep9-jitless   the trace binaries (jit build, jitless build). A build
//                     runs when its trace binary is given
//   --ep9 / --ep9-jitless   the plain binaries of the same commit (the bench step's head
//                     binaries): Ir and the RSS rows. Not given: no Ir and no RSS for that build
//   --head SHA        each binary's build line must name it (the trace build says dirty, the
//                     plain build must say clean)
//   --workloads       default: octane-typescript and the 19 BENCH-SET rows (bench-set.mjs)
//   --forms           H0 = EP9_GC_HELPERS=0 (sequential mark: a fixed count), H1 = 1 helper
//                     (the box form), HD = the helper default of the host
//   --runs, --ts-runs runs per (build, row, arm, form); --ts-runs for octane-typescript
//   --ir-arms         arms with an Ir run (default: --arms); "none" = no Ir run (a host with no
//                     valgrind)
//   --jobs            parallel cachegrind runs (default: the CPU count). Count runs and RSS runs
//                     go one at a time (the helper threads must not share the CPUs)
//
// A run counts only when its exit status is 0, its stdout is the workload's expected output and
// the count of `gcb` lines equals the --gc-stats collections line (else the run is an error).
// Exit status: 0 every run counted; 1 a run failed (the JSON and the table are still written);
// 2 usage error. With --compare: 0 PASS, 1 FAIL or INCOMPLETE, 2 usage error.
// Tests: node --test .github/scripts/gc-count.test.mjs (fake engines; no ep9 build).

import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { BENCH_SET, checkBuildLine, parseCgLog, UsageError } from "./bench-set.mjs";
import { splitVersion } from "../../tools/bench/cross/kernels.mjs";
import { rssTool } from "../../tools/bench/cross/mem.mjs";
import { parseBuild } from "../../tools/bench/cross/run.mjs";
import { expandWorkloads, materialize, TS_WORKLOAD, WORKLOADS } from "../../tools/bench/cross/workloads.mjs";
import { hostInfo } from "../../tools/bench/lib/host.mjs";

export const SCHEMA = "ep9-gc-count/v1";
/** The arms of EP9_GC_BUDGET (heap.rs budget_arm_from): "0" = arm 0, "1" = B1, "2" = B2. */
export const ARMS = ["0", "1", "2"];
/** Helper forms: the EP9_GC_HELPERS value, or null = not set (the host default). */
export const FORMS = { H0: "0", H1: "1", HD: null };
/** O17: the peak old bytes of an IDENTITY row in every count run. */
export const O17_LIMIT = 11 << 20;
/** The TS line: count(B1) / count(arm 0) <= 1 - 0.015 / 0.0443 (r24-offloop.md section 3.5). */
export const TS_RATIO_LIMIT = 0.661;
/** The Ir band of "the merge head equals the read code" (plan v18 section 4.2). */
export const IR_BAND = 1e-5;
/** The decision fields of a GC: equal fields in every GC = the same GC decisions. */
export const DECISION_FIELDS = ["kind", "budget", "next_budget", "next_major", "major_limit", "hs"];

// One error class with bench-set.mjs: its checkBuildLine throws it.
export { UsageError };

// ---- the trace print ---------------------------------------------------------------------------

/** The one line of Heap::collect_scanning that the print goes around. */
export const TRACE_ANCHOR = "            inner.after_gc(minor, marked_bytes, promoted_bytes);\n";
const TRACE_MARK = "// GC-X86-COUNT trace build only";
const TRACE_TEXT = `            let gcb_since = inner.allocated_since_gc; ${TRACE_MARK}
            let gcb_budget = inner.budget;
            let gcb_old_before = inner.old_bytes;
${TRACE_ANCHOR}            eprintln!(
                "gcb n={} kind={} since={} budget={} old_before={} old_after={} marked={} promoted={} next_budget={} next_major={} major_limit={} hs={}",
                inner.stats.collections + 1,
                if minor { "minor" } else { "major" },
                gcb_since,
                gcb_budget,
                gcb_old_before,
                inner.old_bytes,
                marked_bytes,
                promoted_bytes,
                inner.budget,
                inner.next_major,
                inner.major_limit,
                inner.high_survival
            );
`;

/** heap.rs with the trace print; a UsageError unless the anchor is there exactly once. */
export function patchHeap(text) {
  if (text.includes(TRACE_MARK)) throw new UsageError("heap.rs already has the trace print");
  const n = text.split(TRACE_ANCHOR).length - 1;
  if (n !== 1) throw new UsageError(`heap.rs: the anchor line "${TRACE_ANCHOR.trim()}" is there ${n} times, not once`);
  return text.replace(TRACE_ANCHOR, TRACE_TEXT);
}

const INT_FIELDS = ["n", "since", "budget", "old_before", "old_after", "marked", "promoted", "next_budget", "major_limit"];

/**
 * The stderr of a trace run: { gcs: [{ n, kind, since, ..., hs }], collections } (collections:
 * the --gc-stats line, or null). A `gcb` line with a missing or bad field is an error.
 */
export function parseTrace(stderr) {
  const gcs = [];
  let collections = null;
  for (const line of stderr.split("\n")) {
    if (line.startsWith("gcb ")) {
      const d = {};
      for (const kv of line.slice(4).trim().split(/\s+/)) {
        const i = kv.indexOf("=");
        if (i > 0) d[kv.slice(0, i)] = kv.slice(i + 1);
      }
      for (const k of INT_FIELDS) {
        if (!/^\d+$/.test(d[k] ?? "")) throw new Error(`bad gcb line (${k}): ${line.slice(0, 200)}`);
        d[k] = Number(d[k]);
      }
      if (!["minor", "major"].includes(d.kind)) throw new Error(`bad gcb line (kind): ${line.slice(0, 200)}`);
      for (const k of ["next_major", "hs"]) {
        if (!["true", "false"].includes(d[k])) throw new Error(`bad gcb line (${k}): ${line.slice(0, 200)}`);
        d[k] = d[k] === "true";
      }
      gcs.push(d);
    }
    const m = /^ep9: gc collections=(\d+) /.exec(line);
    if (m) collections = Number(m[1]);
  }
  return { gcs, collections };
}

const sha256 = (s) => createHash("sha256").update(s).digest("hex");

/** The sha256 of the GC decisions of one run (DECISION_FIELDS of every GC, in order). */
export function decisionSig(gcs) {
  return sha256(gcs.map((g) => DECISION_FIELDS.map((k) => g[k]).join(" ")).join("\n"));
}

/** The peak old bytes of one run (before or after each GC; 0 with no GC). */
export function peakOld(gcs) {
  return gcs.reduce((m, g) => Math.max(m, g.old_before, g.old_after), 0);
}

export function median(xs) {
  const v = xs.filter((x) => typeof x === "number").sort((a, b) => a - b);
  if (!v.length) return null;
  const h = v.length >> 1;
  return v.length % 2 ? v[h] : (v[h - 1] + v[h]) / 2;
}

/**
 * One cell (build, row, arm, form) from its runs and the arm-0 runs of the same form:
 * GC counts, median, peak old bytes, O17, and whether the set of decision traces equals arm 0's.
 */
export function summarizeCell(runs, arm0Runs) {
  const ok = runs.filter((r) => r.status === "ok");
  const ok0 = (arm0Runs ?? []).filter((r) => r.status === "ok");
  const counts = ok.map((r) => r.gcs);
  const peak = ok.length ? Math.max(...ok.map((r) => r.peak_old)) : null;
  const sigs = [...new Set(ok.map((r) => r.sig))].sort();
  const sigs0 = [...new Set(ok0.map((r) => r.sig))].sort();
  return {
    runs: runs.length,
    counted: ok.length,
    counts,
    median: median(counts),
    constant: ok.length > 0 && counts.every((c) => c === counts[0]),
    majors: ok.map((r) => r.majors),
    peak_old: peak,
    o17: ok.length === runs.length && ok.length > 0 ? (peak <= O17_LIMIT ? "pass" : "FAIL") : "no reading",
    trace_sigs: sigs,
    decisions_equal_arm0: ok.length && ok0.length && ok.length === runs.length ? sigs.join() === sigs0.join() : null,
  };
}

/**
 * The TS line per build and form: count(B1) / count(arm 0) of octane-typescript (medians), the
 * B1 cut, the E value (0.0443 x (1 - ratio)) and "pass" when ratio <= 0.661.
 */
export function tsLines(res) {
  const out = [];
  for (const b of res.builds) {
    const row = b.rows.find((r) => r.name === TS_WORKLOAD);
    if (!row) continue;
    for (const form of res.forms) {
      const c0 = row.arms["0"]?.[form]?.median ?? null;
      const c1 = row.arms["1"]?.[form]?.median ?? null;
      const ratio = c0 && c1 !== null ? c1 / c0 : null;
      out.push({
        build: b.build,
        form,
        arm0: c0,
        b1: c1,
        ratio: ratio === null ? null : Number(ratio.toFixed(4)),
        e: ratio === null ? null : Number((0.0443 * (1 - ratio)).toFixed(4)),
        word: ratio === null ? "no reading" : ratio <= TS_RATIO_LIMIT ? "pass" : "FAIL",
      });
    }
  }
  return out;
}

// ---- options -----------------------------------------------------------------------------------

export function parseArgs(argv) {
  const o = {
    patch: null,
    compare: null,
    readArm: "1",
    mergeArm: "0",
    traceEp9: null,
    traceEp9Jitless: null,
    ep9: null,
    ep9Jitless: null,
    head: null,
    target: `${process.platform}-${process.arch}`,
    workloads: null,
    arms: ARMS,
    irArms: null,
    forms: ["H1", "H0"],
    runs: 3,
    tsRuns: 5,
    rssRows: ["earley-boyer"],
    rssRuns: 5,
    hashSeed: "1",
    jobs: os.availableParallelism?.() ?? os.cpus().length,
    timeoutS: 900,
    valgrind: "valgrind",
    keep: null,
    work: null,
    out: null,
    md: null,
  };
  const int = (a, v, min) => {
    const n = Number(v);
    if (!Number.isInteger(n) || n < min) throw new UsageError(`${a} needs an integer >= ${min}, got ${v}`);
    return n;
  };
  const list = (a, v, ok) => {
    const l = v.split(",");
    if (!v || l.some((x) => !ok(x)) || new Set(l).size !== l.length) throw new UsageError(`${a}: bad list ${JSON.stringify(v)}`);
    return l;
  };
  const arm = (a, v) => {
    if (!ARMS.includes(v)) throw new UsageError(`${a} needs one of ${ARMS.join(", ")}, got ${v}`);
    return v;
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const val = () => {
      if (i + 1 >= argv.length) throw new UsageError(`${a} needs a value`);
      return argv[++i];
    };
    switch (a) {
      case "--patch": o.patch = path.resolve(val()); break;
      case "--compare": o.compare = [path.resolve(val()), path.resolve(val())]; break;
      case "--read-arm": o.readArm = arm(a, val()); break;
      case "--merge-arm": o.mergeArm = arm(a, val()); break;
      case "--trace-ep9": o.traceEp9 = path.resolve(val()); break;
      case "--trace-ep9-jitless": o.traceEp9Jitless = path.resolve(val()); break;
      case "--ep9": o.ep9 = path.resolve(val()); break;
      case "--ep9-jitless": o.ep9Jitless = path.resolve(val()); break;
      case "--head": o.head = val(); break;
      case "--target": o.target = val(); break;
      case "--workloads": {
        // A name twice is one row (the CI step puts octane-typescript first; bench_workloads may hold it).
        const v = val();
        if (!/^[a-z0-9-]+(,[a-z0-9-]+)*$/.test(v)) throw new UsageError(`${a}: bad list ${JSON.stringify(v)}`);
        o.workloads = v.split(",");
        break;
      }
      case "--arms": o.arms = list(a, val(), (x) => ARMS.includes(x)); break;
      case "--ir-arms": {
        const v = val();
        o.irArms = v === "none" ? [] : list(a, v, (x) => ARMS.includes(x));
        break;
      }
      case "--forms": o.forms = list(a, val(), (x) => Object.hasOwn(FORMS, x)); break;
      case "--runs": o.runs = int(a, val(), 1); break;
      case "--ts-runs": o.tsRuns = int(a, val(), 1); break;
      case "--rss-rows": {
        const v = val();
        o.rssRows = v === "none" ? [] : list(a, v, (x) => /^[a-z0-9-]+$/.test(x));
        break;
      }
      case "--rss-runs": o.rssRuns = int(a, val(), 1); break;
      case "--hash-seed": o.hashSeed = val(); break;
      case "--jobs": o.jobs = int(a, val(), 1); break;
      case "--timeout-s": o.timeoutS = int(a, val(), 1); break;
      case "--valgrind": o.valgrind = val(); break;
      case "--keep": o.keep = path.resolve(val()); break;
      case "--work": o.work = path.resolve(val()); break;
      case "--out": o.out = path.resolve(val()); break;
      case "--md": o.md = path.resolve(val()); break;
      default: throw new UsageError(`unknown option ${a}`);
    }
  }
  if (o.patch || o.compare) {
    if (o.patch && o.compare) throw new UsageError("--patch and --compare are two modes");
    return o;
  }
  if (!o.traceEp9 && !o.traceEp9Jitless) throw new UsageError("--trace-ep9 or --trace-ep9-jitless is required");
  if (!o.out) throw new UsageError("--out is required");
  if (o.head !== null && !/^[0-9a-f]{40}$/.test(o.head)) throw new UsageError(`--head needs a full commit id (40 hex digits), got ${o.head}`);
  if (!o.arms.includes("0")) throw new UsageError("--arms must hold arm 0 (every cell compares with it)");
  o.irArms ??= o.arms;
  if (o.irArms.some((x) => !o.arms.includes(x))) throw new UsageError("--ir-arms must be a part of --arms");
  o.workloads = expandWorkloads(o.workloads ?? [TS_WORKLOAD, ...BENCH_SET]);
  for (const w of [...o.workloads, ...o.rssRows]) {
    if (!WORKLOADS[w]) throw new UsageError(`unknown workload ${w}`);
  }
  if (o.rssRows.some((w) => !o.workloads.includes(w))) throw new UsageError("--rss-rows must be a part of --workloads");
  return o;
}

// ---- runs --------------------------------------------------------------------------------------

function versionOf(bin) {
  const v = spawnSync(bin, ["--version"], { encoding: "utf8" });
  if (v.error) throw new UsageError(`${bin}: ${v.error.message}`);
  return { path: bin, ...splitVersion(v.stdout || ""), build: parseBuild(v.stderr) };
}

/** A binary's build line: the --build preset; with --head, that commit; plain = clean. */
export function checkBinary(what, ep9, build, head, { trace }) {
  checkBuildLine(what, ep9, build);
  const b = ep9.build;
  if (head && b?.commit !== head) throw new UsageError(`${what} names commit ${b?.commit ?? "none"}, not --head ${head}`);
  if (head && !trace && b.dirty !== false) throw new UsageError(`${what} was built with uncommitted engine changes`);
}

function envFor(o, arm, helpers) {
  const env = { ...process.env, EP9_HASH_SEED: o.hashSeed, EP9_GC_BUDGET: arm };
  delete env.EP9_GC_HELPERS;
  if (helpers !== null) env.EP9_GC_HELPERS = helpers;
  return env;
}

/** Spawns argv; resolves to { code, signal, error, stdout, stderr }. */
function spawnRun(argv, env, timeoutS) {
  return new Promise((resolve) => {
    const child = spawn(argv[0], argv.slice(1), { env, stdio: ["ignore", "pipe", "pipe"], timeout: timeoutS * 1000 });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => {
      if (stdout.length < 1 << 20) stdout += d;
    });
    child.stderr.on("data", (d) => {
      if (stderr.length < 64 << 20) stderr += d;
    });
    child.on("error", (e) => resolve({ code: null, signal: null, error: e.message, stdout, stderr }));
    child.on("close", (code, signal) => resolve({ code, signal, error: null, stdout, stderr }));
  });
}

function runError(r, w) {
  if (r.error) return r.error;
  if (r.code !== 0) {
    const tail = r.stderr.trim().split("\n").filter((l) => !l.startsWith("gcb ")).slice(-3).join(" / ");
    return `${r.signal ? `signal ${r.signal}` : `exit ${r.code}`}${tail ? `: ${tail}` : ""}`;
  }
  const got = r.stdout.replace(/\r\n/g, "\n").trim();
  if (got !== w.expect) return `printed ${JSON.stringify(got.slice(0, 200))}, want ${JSON.stringify(w.expect)}`;
  return null;
}

/** One count run of a trace binary: GC count, majors, peak old bytes, decision sha, peak RSS. */
async function countRun(o, tool, bin, w, arm, helpers, tmp) {
  const rssFile = path.join(tmp, `rss-${process.pid}-${Math.random().toString(36).slice(2)}`);
  const argv = tool ? [tool.program, ...tool.args(rssFile), bin, "--gc-stats", w.file] : [bin, "--gc-stats", w.file];
  const r = await spawnRun(argv, envFor(o, arm, helpers), o.timeoutS);
  let rss = null;
  try {
    if (tool) rss = tool.parse(readFileSync(rssFile, "utf8"));
  } catch {}
  rmSync(rssFile, { force: true });
  const err = runError(r, w);
  if (err) return { status: "error", error: err };
  let t;
  try {
    t = parseTrace(r.stderr);
  } catch (e) {
    return { status: "error", error: e.message };
  }
  if (t.collections === null) return { status: "error", error: "no --gc-stats line" };
  if (t.collections !== t.gcs.length) return { status: "error", error: `${t.gcs.length} gcb lines, but collections=${t.collections} (not a trace binary?)` };
  return {
    status: "ok",
    gcs: t.gcs.length,
    majors: t.gcs.filter((g) => g.kind === "major").length,
    peak_old: peakOld(t.gcs),
    sig: decisionSig(t.gcs),
    rss,
  };
}

/** One RSS run of a plain binary (the host helper default, as mem.mjs runs). */
async function rssRun(o, tool, bin, w, arm, tmp) {
  const rssFile = path.join(tmp, `rss-${process.pid}-${Math.random().toString(36).slice(2)}`);
  const r = await spawnRun([tool.program, ...tool.args(rssFile), bin, "--gc-stats", w.file], envFor(o, arm, null), o.timeoutS);
  let rss = null;
  try {
    rss = tool.parse(readFileSync(rssFile, "utf8"));
  } catch {}
  rmSync(rssFile, { force: true });
  const err = runError(r, w);
  if (err) return { status: "error", error: err, rss: null, gcs: null };
  if (rss === null) return { status: "error", error: "no RSS reading", rss: null, gcs: null };
  return { status: "ok", error: null, rss, gcs: parseTrace(r.stderr).collections };
}

/** One cachegrind run of a plain binary in the bench-set.mjs form; resolves to { ir } or an error. */
async function irRun(o, bin, w, arm, keep, tag) {
  const stem = path.join(keep, tag);
  const argv = [o.valgrind, "--tool=cachegrind", "--cache-sim=no", "--branch-sim=no", `--cachegrind-out-file=${stem}.out`, `--log-file=${stem}.log`, bin, w.file];
  const r = await spawnRun(argv, envFor(o, arm, "0"), o.timeoutS * 4);
  const err = runError(r, w);
  if (err) return { status: "error", error: err, ir: null };
  try {
    const { ir } = parseCgLog(readFileSync(`${stem}.log`, "utf8"));
    if (ir === null) return { status: "error", error: "the cachegrind log has no I refs line", ir: null };
    return { status: "ok", error: null, ir };
  } catch (e) {
    return { status: "error", error: `no cachegrind log: ${e.message}`, ir: null };
  }
}

async function pool(items, jobs, fn) {
  let next = 0;
  const worker = async () => {
    while (next < items.length) await fn(items[next++]);
  };
  await Promise.all(Array.from({ length: Math.min(jobs, items.length) }, worker));
}

/** The whole count run: returns the ep9-gc-count/v1 object. */
export async function run(o, { log = (s) => process.stderr.write(`${s}\n`) } = {}) {
  const builds = [
    { build: "jit", trace: o.traceEp9, plain: o.ep9 },
    { build: "jitless", trace: o.traceEp9Jitless, plain: o.ep9Jitless },
  ].filter((b) => b.trace);
  for (const b of builds) {
    b.traceVersion = versionOf(b.trace);
    checkBinary(`the ${b.build} trace binary`, b.traceVersion, b.build, o.head, { trace: true });
    if (b.plain) {
      b.plainVersion = versionOf(b.plain);
      checkBinary(`the ${b.build} plain binary`, b.plainVersion, b.build, o.head, { trace: false });
    }
  }
  const tool = rssTool();
  const tmp = mkdtempSync(path.join(os.tmpdir(), "ep9-gc-count-"));
  const work = o.work ?? path.join(tmp, "work");
  const keep = o.keep ?? path.join(tmp, "cg");
  mkdirSync(work, { recursive: true });
  mkdirSync(keep, { recursive: true });
  const notes = [];
  if (!tool) notes.push(`no RSS tool on ${process.platform}`);
  const vg = o.irArms.length ? spawnSync(o.valgrind, ["--version"], { encoding: "utf8" }) : null;
  let errors = 0;
  const out = [];
  try {
    const files = materialize(o.workloads, work);
    const ws = o.workloads.map((name) => ({ name, file: files[name].file, expect: files[name].expect, sha256: files[name].sha256 }));
    for (const b of builds) {
      const rows = [];
      for (const w of ws) {
        const runsPer = w.name === TS_WORKLOAD ? o.tsRuns : o.runs;
        const raw = {};
        // Count runs one at a time, arms interleaved in each round (a slow host phase hits every arm).
        for (const form of o.forms) {
          for (const arm of o.arms) (raw[arm] ??= {})[form] = [];
          for (let i = 0; i < runsPer; i++) {
            for (const arm of o.arms) {
              const r = await countRun(o, tool, b.trace, w, arm, FORMS[form], tmp);
              raw[arm][form].push(r);
              if (r.status !== "ok") {
                errors++;
                log(`gc-count: ${b.build} ${w.name} arm ${arm} ${form} run ${i + 1}: ${r.error}`);
              }
            }
          }
        }
        const arms = {};
        for (const arm of o.arms) {
          arms[arm] = {};
          for (const form of o.forms) {
            const cell = summarizeCell(raw[arm][form], raw["0"][form]);
            cell.rss = raw[arm][form].map((r) => r.rss ?? null);
            arms[arm][form] = cell;
          }
        }
        const line = o.forms.map((f) => `${f} ${o.arms.map((a) => `${a}:${arms[a][f].median ?? "-"}`).join(" ")}`).join("; ");
        log(`gc-count: ${b.build} ${w.name}: GCs ${line}`);
        const row = { name: w.name, sha256: w.sha256, arms, rss: null, ir: null };
        if (o.rssRows.includes(w.name) && b.plain && tool) {
          row.rss = {};
          for (const arm of o.arms) row.rss[arm] = [];
          for (let i = 0; i < o.rssRuns; i++) {
            for (const arm of o.arms) {
              const r = await rssRun(o, tool, b.plain, w, arm, tmp);
              row.rss[arm].push(r);
              if (r.status !== "ok") {
                errors++;
                log(`gc-count: ${b.build} ${w.name} RSS arm ${arm} run ${i + 1}: ${r.error}`);
              }
            }
          }
          for (const arm of o.arms) {
            const v = row.rss[arm].map((r) => r.rss);
            row.rss[arm] = { bytes: v, median: median(v) };
          }
        }
        rows.push(row);
      }
      if (b.plain && o.irArms.length) {
        const items = rows.flatMap((row) => o.irArms.map((arm) => ({ row, arm })));
        await pool(items, o.jobs, async ({ row, arm }) => {
          const w = ws.find((x) => x.name === row.name);
          const r = await irRun(o, b.plain, w, arm, keep, `${b.build}.${row.name}.arm${arm}`);
          (row.ir ??= {})[arm] = r.ir;
          if (r.status !== "ok") {
            errors++;
            log(`gc-count: ${b.build} ${row.name} Ir arm ${arm}: ${r.error}`);
          } else {
            log(`gc-count: ${b.build} ${row.name} Ir arm ${arm}: ${r.ir}`);
          }
        });
        for (const row of rows) {
          row.ir_rel_arm0 = {};
          for (const arm of o.irArms) {
            const a = row.ir?.[arm];
            const z = row.ir?.["0"];
            row.ir_rel_arm0[arm] = typeof a === "number" && typeof z === "number" && z > 0 ? a / z - 1 : null;
          }
        }
      } else if (o.irArms.length) {
        notes.push(`${b.build}: no plain binary, so no Ir and no RSS rows`);
      }
      out.push({
        build: b.build,
        trace_ep9: { ...b.traceVersion, path: path.basename(b.traceVersion.path) },
        plain_ep9: b.plainVersion ? { ...b.plainVersion, path: path.basename(b.plainVersion.path) } : null,
        rows,
      });
    }
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
  const res = {
    schema: SCHEMA,
    kind: "count run (never a timed run)",
    target: o.target,
    head: o.head,
    switch: "EP9_GC_BUDGET",
    arms: o.arms,
    ir_arms: o.irArms,
    forms: o.forms,
    form_helpers: Object.fromEntries(o.forms.map((f) => [f, FORMS[f]])),
    runs: o.runs,
    ts_runs: o.tsRuns,
    hash_seed: o.hashSeed,
    o17_limit_bytes: O17_LIMIT,
    ts_ratio_limit: TS_RATIO_LIMIT,
    rss_tool: tool?.name ?? null,
    valgrind: vg && vg.status === 0 ? vg.stdout.trim() : null,
    host: hostInfo(),
    notes,
    errors,
    builds: out,
  };
  res.ts = tsLines(res);
  return res;
}

// ---- the merge check ---------------------------------------------------------------------------

/**
 * "The merge head equals the read code" (plan v18 section 4.2): for every build and row of the
 * read, the merge run's GC count (median, each form) equals the read's arm --read-arm, and its
 * Ir (arm --merge-arm) is within 1e-5 of the read's Ir at --read-arm.
 */
export function compareReads(read, merge, { readArm = "1", mergeArm = "0" } = {}) {
  const rows = [];
  for (const rb of read.builds) {
    const mb = merge.builds.find((b) => b.build === rb.build);
    for (const rr of rb.rows) {
      const mr = mb?.rows.find((r) => r.name === rr.name);
      const forms = read.forms.filter((f) => merge.forms.includes(f));
      const counts = forms.map((f) => ({ form: f, read: rr.arms[readArm]?.[f]?.median ?? null, merge: mr?.arms[mergeArm]?.[f]?.median ?? null }));
      const ir0 = rr.ir?.[readArm] ?? null;
      const ir1 = mr?.ir?.[mergeArm] ?? null;
      const have = Number.isInteger(ir0) && Number.isInteger(ir1) && ir0 > 0;
      const rel = have ? ir1 / ir0 - 1 : null;
      // Integer compare (as bench-set.mjs compare): |merge - read| x 100,000 <= read.
      const irOk = have && (BigInt(ir1) - BigInt(ir0)) * (ir1 >= ir0 ? 1n : -1n) * BigInt(Math.round(1 / IR_BAND)) <= BigInt(ir0);
      const countOk = counts.length > 0 && counts.every((c) => c.read !== null && c.read === c.merge);
      const countMissing = !counts.length || counts.some((c) => c.read === null || c.merge === null);
      const word = countMissing || !have ? "no reading" : countOk && irOk ? "pass" : "FAIL";
      rows.push({ build: rb.build, name: rr.name, counts, ir_read: ir0, ir_merge: ir1, ir_rel: rel, word });
    }
  }
  const verdict = rows.some((r) => r.word === "FAIL") ? "FAIL" : rows.length && rows.every((r) => r.word === "pass") ? "PASS" : "INCOMPLETE";
  return { read_head: read.head, merge_head: merge.head, read_arm: readArm, merge_arm: mergeArm, verdict, rows };
}

// ---- tables ------------------------------------------------------------------------------------

const mib = (b) => (b === null || b === undefined ? "-" : (b / (1 << 20)).toFixed(1));
const num = (n) => (n === null || n === undefined ? "-" : n.toLocaleString("en-US"));

export function renderMarkdown(res) {
  const L = [];
  L.push(`### GC count run (${res.target}; head ${res.head ?? "not checked"}; a count run, never timed)`);
  L.push("");
  L.push(`Switch ${res.switch}: arms ${res.arms.join(", ")} (1 = B1, 2 = B2); forms ${res.forms.join(", ")}; ${res.runs} runs per cell (octane-typescript ${res.ts_runs}); errors ${res.errors}.`);
  L.push("");
  for (const t of res.ts) {
    L.push(`- TS line ${t.build} ${t.form}: arm 0 ${t.arm0 ?? "-"} GCs, B1 ${t.b1 ?? "-"}; ratio ${t.ratio ?? "-"} (limit ${res.ts_ratio_limit}); E ${t.e ?? "-"}: **${t.word}**`);
  }
  for (const b of res.builds) {
    L.push("");
    L.push(`#### ${b.build} build`);
    L.push("");
    const f = res.forms;
    const head = ["row", ...res.arms.map((a) => `GCs arm ${a} (${f.join("/")})`), "peak old MiB (all arms)", "O17", "decisions = arm 0", ...res.ir_arms.filter((a) => a !== "0").map((a) => `Ir arm ${a} vs 0`)];
    L.push(`| ${head.join(" | ")} |`);
    L.push(`|${head.map(() => "---").join("|")}|`);
    for (const r of b.rows) {
      const cells = res.arms.map((a) => f.map((x) => r.arms[a][x].median ?? "-").join("/"));
      const peak = Math.max(...res.arms.flatMap((a) => f.map((x) => r.arms[a][x].peak_old ?? 0)));
      const o17 = res.arms.flatMap((a) => f.map((x) => r.arms[a][x].o17));
      const o17w = o17.includes("FAIL") ? "FAIL" : o17.includes("no reading") ? "no reading" : "pass";
      const eq = res.arms.filter((a) => a !== "0").map((a) => `${a}:${f.map((x) => (r.arms[a][x].decisions_equal_arm0 === null ? "-" : r.arms[a][x].decisions_equal_arm0 ? "yes" : "NO")).join("/")}`);
      const ir = res.ir_arms.filter((a) => a !== "0").map((a) => {
        const v = r.ir_rel_arm0?.[a];
        return v === null || v === undefined ? "-" : `${v >= 0 ? "+" : ""}${(v * 100).toFixed(4)}%`;
      });
      L.push(`| ${[r.name, ...cells, mib(peak), o17w, eq.join(" "), ...ir].join(" | ")} |`);
    }
    for (const r of b.rows.filter((x) => x.rss)) {
      const arm0 = r.rss["0"].median;
      const parts = res.arms.map((a) => `arm ${a} ${mib(r.rss[a].median)} MiB${a !== "0" && arm0 ? ` (x${(r.rss[a].median / arm0).toFixed(3)})` : ""}`);
      L.push("");
      L.push(`- RSS (G7 line) ${r.name}, ${b.build}, plain binary, median peak RSS: ${parts.join("; ")}`);
    }
  }
  if (res.notes.length) {
    L.push("");
    for (const n of res.notes) L.push(`- note: ${n}`);
  }
  L.push("");
  return `${L.join("\n")}\n`;
}

export function renderCompare(c) {
  const L = [`### Merge head = read code: ${c.verdict} (read ${c.read_head ?? "-"} arm ${c.read_arm}; merge ${c.merge_head ?? "-"} arm ${c.merge_arm})`, ""];
  L.push("| build | row | GCs read / merge | Ir read | Ir merge | Ir rel | word |");
  L.push("|---|---|---|---|---|---|---|");
  for (const r of c.rows) {
    const g = r.counts.map((x) => `${x.form} ${x.read ?? "-"}/${x.merge ?? "-"}`).join(", ");
    const rel = r.ir_rel === null ? "-" : r.ir_rel.toExponential(2);
    L.push(`| ${r.build} | ${r.name} | ${g} | ${num(r.ir_read)} | ${num(r.ir_merge)} | ${rel} | ${r.word} |`);
  }
  return `${L.join("\n")}\n`;
}

// ---- main --------------------------------------------------------------------------------------

async function main() {
  let o;
  try {
    o = parseArgs(process.argv.slice(2));
    if (o.patch) {
      writeFileSync(o.patch, patchHeap(readFileSync(o.patch, "utf8")));
      process.stdout.write(`gc-count: trace print written into ${o.patch}\n`);
      process.exit(0);
    }
    if (o.compare) {
      const [read, merge] = o.compare.map((f) => JSON.parse(readFileSync(f, "utf8")));
      for (const x of [read, merge]) if (x.schema !== SCHEMA) throw new UsageError(`not an ${SCHEMA} file`);
      const c = compareReads(read, merge, { readArm: o.readArm, mergeArm: o.mergeArm });
      process.stdout.write(renderCompare(c));
      process.exit(c.verdict === "PASS" ? 0 : 1);
    }
  } catch (e) {
    if (!(e instanceof UsageError)) throw e;
    console.error(`gc-count: ${e.message}`);
    process.exit(2);
  }
  let res;
  try {
    res = await run(o);
  } catch (e) {
    if (!(e instanceof UsageError)) throw e;
    console.error(`gc-count: ${e.message}`);
    process.exit(2);
  }
  writeFileSync(o.out, `${JSON.stringify(res, null, 1)}\n`);
  const md = renderMarkdown(res);
  if (o.md) appendFileSync(o.md, md);
  process.stdout.write(md);
  process.exit(res.errors ? 1 : 0);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main().catch((e) => {
    console.error(e.stack ?? String(e));
    process.exit(1);
  });
}
