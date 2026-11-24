//! T-HOLES (H01 layer 1): every class of the pre-scan hole hunt (297) and of the pre-scan
//! design check (21) gives its expected result kind, each in its own child process.
//!
//! Before layer 1, 117 of these classes aborted the process: the pre-scan (layer 0) counted
//! them low, and oxc recursed past the end of the worker stack. Layer 1 (the stack check of
//! the oxc_parser patch, l1.rs) must turn each one into a RangeError. The other 201 classes are
//! controls: the caps decide them, so their result is the hunt result on every host, and a
//! StackLimit there is a Claim D failure (the check fired within the budget).
//!
//! The table is `tests/stack_holes.tsv` (its header has the column rules). The test sets:
//!
//! | Set | Rows | Sizes | Profiles |
//! |---|---|---|---|
//! | `every_abort_class_stops` | 117 | `n_abort` and 2,000,000 | debug, release |
//! | `every_control_class_is_clean` | 198 | 2,000,000; a cap-size row at its hunt sizes | debug, release |
//! | `slow_classes_print_their_time` | 3 | the hunt sizes up to 5,000 (`SLOW_MAX_N`) | release (it prints the time; it asserts the kind) |
//!
//! Debug runs the controls at 2,000,000 too: on the Mac the debug set took less than 4 min
//! (the switch rule of the plan; the measured times are in the role log).
//!
//! A child is this test binary again (`--exact <set> --nocapture --test-threads=1`) with the
//! env `EP9_L1_CHILD=holes:<name>:<n>`. It prints `HOLES <name> n <n> -> <kind>`. The parent
//! fails on a signal, a non-zero status, a time-out, no line, a line for another row or size,
//! or a kind that the row does not allow.
//!
//! Memory: a child that parses touches up to one worker stack, and a flat program of 2,000,000
//! levels builds a large AST (the largest child, `E17-binary-mixed-prec`, has a peak RSS of
//! 3.7 GiB in debug on the Mac). So the sets run one at a time, at most 4 children run at a
//! time, and a child with a large text takes more of the 4 slots (`weight`), so that 2 large
//! children never run together (the macOS CI runner has 7 GB).
//!
//! `cargo test -p ep9_frontend --test stack_holes`
//! `cargo test -p ep9_frontend --release --test stack_holes`

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use ep9_bytecode::{EvalContext, FunctionCtorKind, VerifiedUnit};
use ep9_frontend::{
    CodegenError, CompileError, CompileOptions, DepthLayer, RangeErrorKind, SourceOpts, Strictness,
    compile, compile_eval, compile_function_ctor,
};

const TABLE: &str = include_str!("stack_holes.tsv");
const CHILD: &str = "EP9_L1_CHILD";
/// The hunt size of every class that ran at full size, and the size of a control row with no
/// `@n` case.
const BIG: usize = 2_000_000;
/// One child may not take longer than this (the slowest child on the Mac took 20 s in debug).
const TIME_OUT: Duration = Duration::from_secs(600);
/// A child takes one slot per this many bytes of text (at least 1, at most all slots).
const BYTES_PER_SLOT: usize = 8 << 20;
/// The slow set runs a hunt size only up to this level count. The slow classes also use
/// memory that grows as the square of n (oxc parses the arrow heads again, and its arena
/// keeps each try): on the Mac in release, X9 needs 2.3 GiB at 5,000 levels, 5.8 GiB at 8,000
/// and 13.8 GiB at 12,400, and X19 9.6 GiB at 8,000. The larger hunt sizes do not fit the
/// macOS CI runner (7 GB); the table keeps them.
const SLOW_MAX_N: usize = 5_000;
/// The allowed kinds of an abort row (the stop, or a cap after a parse that fitted).
const STOP: &str = "Range(StackLimit)|Range(Ast)|Range(Pass)";

/// The hunt rule counts of hunt.md (rules 1-18; 103 abort classes).
const RULE_COUNTS: [usize; 18] = [11, 5, 8, 8, 4, 4, 8, 2, 12, 4, 2, 2, 1, 6, 3, 4, 1, 18];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Set {
    Abort,
    Control,
    Slow,
}

impl Set {
    fn test(self) -> &'static str {
        match self {
            Set::Abort => "every_abort_class_stops",
            Set::Control => "every_control_class_is_clean",
            Set::Slow => "slow_classes_print_their_time",
        }
    }
}

/// One case of the `expect` column: the allowed kinds, at the set sizes or at one size.
struct Case {
    kinds: Vec<&'static str>,
    n: Option<usize>,
}

struct Row {
    name: &'static str,
    family: &'static str,
    goal: &'static str,
    path: &'static str,
    /// prefix, open, mid, close, suffix (unescaped).
    parts: [String; 5],
    n_abort: Option<usize>,
    hunt: &'static str,
    valid: bool,
    expect: &'static str,
    cases: Vec<Case>,
}

impl Row {
    fn set(&self) -> Set {
        match self.hunt {
            "ABORT" => Set::Abort,
            "timing" => Set::Slow,
            _ => Set::Control,
        }
    }

    /// `prefix + open x n + mid + close x n + suffix`; `{i}` is the level index.
    fn text(&self, n: usize) -> String {
        let [prefix, open, mid, close, suffix] = &self.parts;
        let mut s = String::with_capacity(prefix.len() + (open.len() + close.len() + 7) * n + 64);
        s.push_str(prefix);
        repeat_into(&mut s, open, n);
        s.push_str(mid);
        repeat_into(&mut s, close, n);
        s.push_str(suffix);
        s
    }

    /// The byte length of `text(n)`, near enough for `weight` (a `{i}` counts as 3 bytes).
    fn text_len(&self, n: usize) -> usize {
        let [prefix, open, mid, close, suffix] = &self.parts;
        prefix.len() + mid.len() + suffix.len() + (open.len() + close.len()) * n
    }

    /// The (size, allowed kinds) jobs of this row.
    fn jobs(&self) -> Vec<(usize, &[&'static str])> {
        let mut jobs = Vec::new();
        for case in &self.cases {
            match (self.set(), case.n) {
                (Set::Abort, _) => {
                    if let Some(n) = self.n_abort.filter(|&n| n != BIG) {
                        jobs.push((n, case.kinds.as_slice()));
                    }
                    jobs.push((BIG, case.kinds.as_slice()));
                }
                (Set::Control, None) => jobs.push((BIG, case.kinds.as_slice())),
                (_, Some(n)) => jobs.push((n, case.kinds.as_slice())),
                (Set::Slow, None) => panic!("{}: a slow row needs a size", self.name),
            }
        }
        jobs
    }
}

fn repeat_into(s: &mut String, part: &str, n: usize) {
    if part.contains("{i}") {
        let (head, tail) = part.split_once("{i}").unwrap();
        for i in 0..n {
            s.push_str(head);
            s.push_str(&i.to_string());
            s.push_str(tail);
        }
    } else {
        for _ in 0..n {
            s.push_str(part);
        }
    }
}

/// The escapes of the table: `\n \r \t \\ \u{HEX}`.
fn unescape(field: &str, name: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some('u') => {
                assert_eq!(chars.next(), Some('{'), "{name}: bad \\u escape");
                let hex: String = chars.by_ref().take_while(|&c| c != '}').collect();
                let code = u32::from_str_radix(&hex, 16).expect("hex digits");
                out.push(char::from_u32(code).expect("a scalar value"));
            }
            other => panic!("{name}: bad escape \\{other:?}"),
        }
    }
    out
}

fn table() -> Vec<Row> {
    TABLE
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|line| {
            let cols: Vec<&'static str> = line.split('\t').collect();
            assert_eq!(cols.len(), 13, "a row has 13 columns: {line}");
            let name = cols[0];
            let parts = std::array::from_fn(|i| unescape(cols[4 + i], name));
            let cases = cols[12]
                .split(';')
                .map(|case| {
                    let (kinds, n) = match case.split_once('@') {
                        Some((k, n)) => (k, Some(n.parse().expect("a size"))),
                        None => (case, None),
                    };
                    Case {
                        kinds: kinds.split('|').collect(),
                        n,
                    }
                })
                .collect();
            Row {
                name,
                family: cols[1],
                goal: cols[2],
                path: cols[3],
                parts,
                n_abort: (!cols[9].is_empty()).then(|| cols[9].parse().expect("n_abort")),
                hunt: cols[10],
                valid: match cols[11] {
                    "yes" => true,
                    "no" => false,
                    v => panic!("{name}: valid {v}"),
                },
                expect: cols[12],
                cases,
            }
        })
        .collect()
}

/// The result kind as a word.
fn word(result: &Result<VerifiedUnit, CodegenError>) -> String {
    match result {
        Ok(_) => "Ok".to_owned(),
        Err(CodegenError::Compile(CompileError::Syntax(_))) => "Syntax".to_owned(),
        Err(CodegenError::Compile(CompileError::Range(e))) => match e.kind {
            RangeErrorKind::TooDeep(DepthLayer::PreScan) => "Range(PreScan)".to_owned(),
            RangeErrorKind::TooDeep(DepthLayer::Ast) => "Range(Ast)".to_owned(),
            RangeErrorKind::TooDeep(DepthLayer::Pass) => "Range(Pass)".to_owned(),
            RangeErrorKind::StackLimit => "Range(StackLimit)".to_owned(),
            RangeErrorKind::StackUnavailable => "Range(StackUnavailable)".to_owned(),
        },
        Err(CodegenError::Unsupported(_)) => "Unsupported".to_owned(),
        Err(CodegenError::Limit(_)) => "Limit".to_owned(),
        Err(CodegenError::Internal(_)) => "Internal".to_owned(),
    }
}

/// Compile the row's text at `n` by its path (public API only).
fn run_row(row: &Row, n: usize) -> Result<VerifiedUnit, CodegenError> {
    let text = row.text(n);
    let options = CompileOptions::default();
    match (row.path, row.goal) {
        ("compile", "script") => compile(&text, SourceOpts::script(Strictness::Sloppy), options),
        ("compile", "strict") => compile(&text, SourceOpts::script(Strictness::Strict), options),
        ("compile", "module") => compile(&text, SourceOpts::module(), options),
        // A direct eval at the top level of a sloppy script (the hunt's outer script).
        ("eval", "script") => compile_eval(
            &text,
            &EvalContext {
                direct: true,
                ..EvalContext::default()
            },
            options,
        ),
        ("function", "script") => {
            compile_function_ctor(FunctionCtorKind::Normal, "", &text, options)
        }
        ("function-params", "script") => {
            compile_function_ctor(FunctionCtorKind::Normal, &text, "", options)
        }
        (path, goal) => panic!("{}: path {path}, goal {goal}", row.name),
    }
}

/// The child side: if this process is a child of `set`, run its row and return true.
fn child(set: Set) -> bool {
    let Ok(value) = std::env::var(CHILD) else {
        return false;
    };
    let spec = value
        .strip_prefix("holes:")
        .unwrap_or_else(|| panic!("{CHILD}={value}: not a T-HOLES child"));
    let (name, n) = spec.rsplit_once(':').expect("holes:<name>:<n>");
    let n: usize = n.parse().expect("a size");
    let rows = table();
    let row = rows.iter().find(|r| r.name == name).expect("a row");
    assert_eq!(row.set(), set, "{name}: not a row of {}", set.test());
    let start = Instant::now();
    let kind = word(&run_row(row, n));
    let secs = start.elapsed().as_secs_f64();
    println!("HOLES {name} n {n} -> {kind}");
    println!("HOLES-TIME {name} n {n} {secs:.3} s");
    true
}

/// One set at a time (the memory rule of the module doc).
static ONE_SET: Mutex<()> = Mutex::new(());
/// The child slots in use.
static SLOTS: Mutex<usize> = Mutex::new(0);
static SLOT_FREE: Condvar = Condvar::new();

fn width() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(4))
}

/// The slots that a child with a text of `bytes` takes.
fn weight(bytes: usize) -> usize {
    (bytes / BYTES_PER_SLOT + 1).min(width())
}

struct Slot(usize);

impl Slot {
    fn take(weight: usize) -> Self {
        let mut used = SLOTS.lock().unwrap();
        while *used + weight > width() {
            used = SLOT_FREE.wait(used).unwrap();
        }
        *used += weight;
        Slot(weight)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        *SLOTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) -= self.0;
        SLOT_FREE.notify_all();
    }
}

struct Outcome {
    /// `None`: the child ran past `TIME_OUT` and was killed.
    status: Option<std::process::ExitStatus>,
    stdout: String,
    stderr: String,
    wall: Duration,
}

fn spawn_child(set: Set, name: &str, n: usize, weight: usize) -> Outcome {
    let _slot = Slot::take(weight);
    let start = Instant::now();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", set.test(), "--nocapture", "--test-threads=1"])
        .env(CHILD, format!("holes:{name}:{n}"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the child");
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut s = Vec::new();
        out.read_to_end(&mut s).ok();
        s
    });
    let err = std::thread::spawn(move || {
        let mut s = Vec::new();
        err.read_to_end(&mut s).ok();
        s
    });
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait for the child") {
            break Some(status);
        }
        if start.elapsed() > TIME_OUT {
            child.kill().ok();
            child.wait().ok();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let wall = start.elapsed();
    Outcome {
        status,
        stdout: String::from_utf8_lossy(&out.join().unwrap()).into_owned(),
        stderr: String::from_utf8_lossy(&err.join().unwrap()).into_owned(),
        wall,
    }
}

fn status_text(status: Option<std::process::ExitStatus>) -> String {
    let Some(status) = status else {
        return format!("killed after the time-out of {} s", TIME_OUT.as_secs());
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("{status:?} (signal {signal})");
        }
    }
    format!("{status:?}")
}

/// The kind that the child printed for (`name`, `n`), or why there is none.
fn child_kind(stdout: &str, name: &str, n: usize) -> Result<String, String> {
    // The harness prints "test ... " on the same line before the child's line.
    let lines: Vec<&str> = stdout
        .lines()
        .filter_map(|l| l.find("HOLES ").map(|at| &l[at..]))
        .collect();
    let mark = format!("HOLES {name} n {n} -> ");
    match lines.as_slice() {
        [] => Err("no HOLES line".to_owned()),
        [line] => line
            .strip_prefix(&mark)
            .map(|k| k.trim().to_owned())
            .ok_or_else(|| format!("a line for another row or size: {line}")),
        more => Err(format!("{} HOLES lines", more.len())),
    }
}

fn tail(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

/// The judge of every set: the child printed a kind, and the row allows it. A slow row too:
/// its cases are valid programs (Ok) or a cap, so a StackLimit there is a lost program.
fn allowed(kind: &Result<String, String>, kinds: &[&str]) -> bool {
    match kind {
        Ok(k) => kinds.contains(&k.as_str()),
        Err(_) => false,
    }
}

/// One child run: the row, the level count and the allowed kinds.
type Job<'a> = (&'a Row, usize, &'a [&'static str]);

/// The parent side: run every job of `set` in a child; fail with every wrong row.
fn run_set(set: Set) {
    assert!(
        std::env::var_os(CHILD).is_none(),
        "{CHILD} is set in the parent: a child must not start children"
    );
    let _one_set = ONE_SET
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let rows = table();
    let (jobs, skipped): (Vec<Job>, Vec<Job>) = rows
        .iter()
        .filter(|r| r.set() == set)
        .flat_map(|r| r.jobs().into_iter().map(move |(n, k)| (r, n, k)))
        .partition(|&(_, n, _)| set != Set::Slow || n <= SLOW_MAX_N);
    for (row, n, _) in &skipped {
        println!(
            "T-HOLES {}: {} n {n}: not run (above {SLOW_MAX_N} levels; see SLOW_MAX_N)",
            set.test(),
            row.name
        );
    }
    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let times = Mutex::new(Vec::new());
    let kinds_seen = Mutex::new(Vec::new());
    let start = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..width() {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(row, n, kinds)) = jobs.get(i) else {
                        break;
                    };
                    // A slow class also needs memory that grows as the square of n: alone.
                    let weight = if set == Set::Slow {
                        width()
                    } else {
                        weight(row.text_len(n))
                    };
                    let out = spawn_child(set, row.name, n, weight);
                    let ok = out.status.is_some_and(|s| s.success());
                    let kind = child_kind(&out.stdout, row.name, n);
                    let time = out
                        .stdout
                        .lines()
                        .find_map(|l| l.find("HOLES-TIME ").map(|at| l[at..].to_owned()));
                    times.lock().unwrap().push((row.name, n, out.wall, time));
                    if let Ok(k) = &kind {
                        kinds_seen.lock().unwrap().push(k.clone());
                        println!("HOLES {} n {n} -> {k}", row.name);
                    }
                    if !ok || !allowed(&kind, kinds) {
                        failures.lock().unwrap().push(format!(
                            "{} n {n}: status {}, kind {}, allowed {}\nstderr tail:\n{}",
                            row.name,
                            status_text(out.status),
                            kind.unwrap_or_else(|e| e),
                            kinds.join("|"),
                            tail(&out.stderr, 20)
                        ));
                    }
                }
            });
        }
    });
    let elapsed = start.elapsed();
    let mut times = times.into_inner().unwrap();
    times.sort_by_key(|t| std::cmp::Reverse(t.2));
    for (name, n, wall, time) in times.iter().take(if set == Set::Slow { 64 } else { 5 }) {
        println!(
            "T-HOLES {}: {name} n {n}: child {:.2} s; {}",
            set.test(),
            wall.as_secs_f64(),
            time.as_deref().unwrap_or("no time line")
        );
    }
    let mut seen = kinds_seen.into_inner().unwrap();
    seen.sort_unstable();
    let mut counts: Vec<(String, usize)> = Vec::new();
    for k in seen {
        match counts.last_mut() {
            Some((last, c)) if *last == k => *c += 1,
            _ => counts.push((k, 1)),
        }
    }
    let counts: Vec<String> = counts.iter().map(|(k, c)| format!("{k} {c}")).collect();
    println!(
        "T-HOLES {}: {} children ({} rows) in {:.1} s, {} at a time; kinds: {}",
        set.test(),
        jobs.len(),
        rows.iter().filter(|r| r.set() == set).count(),
        elapsed.as_secs_f64(),
        width(),
        counts.join(", ")
    );
    let failures = failures.into_inner().unwrap();
    assert!(
        failures.is_empty(),
        "{} of {} children failed:\n{}",
        failures.len(),
        jobs.len(),
        failures.join("\n\n")
    );
}

/// The 117 classes that aborted before layer 1 (103 hunt + 14 design check): a RangeError
/// (StackLimit where layer 0 misses the depth, or a cap after a parse that fitted), or a
/// SyntaxError only for a text that is not valid. Never Ok, never the pre-scan, never a signal.
#[test]
fn every_abort_class_stops() {
    if child(Set::Abort) {
        return;
    }
    run_set(Set::Abort);
}

/// The 198 control classes (191 hunt + 7 design check): exactly the hunt kind of each row.
/// A StackLimit here is a Claim D failure.
#[test]
fn every_control_class_is_clean() {
    if child(Set::Control) {
        return;
    }
    run_set(Set::Control);
}

/// The 3 slow classes of the hunt (X9, X18, X19: clean, but quadratic in oxc), at their hunt
/// sizes up to `SLOW_MAX_N`, one child at a time. Release only; it prints the time, and fails
/// when a child dies or hangs or gives a kind that the row does not allow.
#[test]
#[cfg_attr(debug_assertions, ignore = "release only: slow in debug")]
fn slow_classes_print_their_time() {
    if child(Set::Slow) {
        return;
    }
    run_set(Set::Slow);
}

/// The table itself: 318 rows, unique names, the hunt.md family counts, and an `expect` that
/// follows the row rules, so that no row can be made weaker by an edit.
#[test]
fn the_table_is_well_formed() {
    assert!(
        TABLE.len() < 64 * 1024,
        "the table is {} bytes",
        TABLE.len()
    );
    assert!(
        TABLE
            .bytes()
            .all(|b| (0x20..0x7f).contains(&b) || b == b'\t' || b == b'\n'),
        "a raw control, format or non-ASCII byte in the table"
    );
    let rows = table();
    assert_eq!(rows.len(), 318);
    let mut names: Vec<&str> = rows.iter().map(|r| r.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 318, "row names are unique");
    let count = |set| rows.iter().filter(|r| r.set() == set).count();
    assert_eq!(
        (count(Set::Abort), count(Set::Control), count(Set::Slow)),
        (117, 198, 3)
    );
    let family = |f: &str| {
        rows.iter()
            .filter(|r| r.set() == Set::Abort && r.family == f)
            .count()
    };
    for (i, &want) in RULE_COUNTS.iter().enumerate() {
        assert_eq!(family(&(i + 1).to_string()), want, "hunt rule {}", i + 1);
    }
    for k in 1..=6 {
        assert!(family(&format!("K{k}")) >= 1, "design-check class K{k}");
    }
    let stop_syntax = format!("{STOP}|Syntax");
    for row in &rows {
        assert!(
            matches!(row.goal, "script" | "strict" | "module"),
            "{}",
            row.name
        );
        assert!(
            matches!(
                row.path,
                "compile" | "eval" | "function" | "function-params"
            ),
            "{}",
            row.name
        );
        assert!(
            row.path == "compile" || row.goal == "script",
            "{}",
            row.name
        );
        assert!(!row.text(3).is_empty(), "{}", row.name);
        if row.set() == Set::Abort {
            let want = if row.valid {
                STOP
            } else {
                stop_syntax.as_str()
            };
            assert_eq!(row.expect, want, "{}: the abort rule", row.name);
            assert!(
                row.family != "control" && row.n_abort.is_none_or(|n| n < BIG),
                "{}",
                row.name
            );
        } else {
            assert_eq!(row.family, "control", "{}", row.name);
            assert!(row.n_abort.is_none(), "{}", row.name);
            assert!(!row.expect.contains("StackLimit"), "{}: Claim D", row.name);
            assert!(
                matches!(row.hunt, "clean" | "runs" | "rc1" | "timing"),
                "{}",
                row.name
            );
        }
        for case in &row.cases {
            for kind in &case.kinds {
                assert!(
                    matches!(
                        *kind,
                        "Ok" | "Syntax"
                            | "Unsupported"
                            | "Range(PreScan)"
                            | "Range(Ast)"
                            | "Range(Pass)"
                            | "Range(StackLimit)"
                    ),
                    "{}: kind {kind}",
                    row.name
                );
            }
        }
    }
}

/// Review V5: the slow set judges the kind as the other sets do. Each case of a slow row at a
/// size that the set runs allows its own kinds only: a StackLimit is not allowed (each one is a
/// valid program or a cap), and a missing line never passes.
#[test]
fn every_set_judges_the_kind() {
    let rows = table();
    let mut cases = 0;
    for row in rows.iter().filter(|r| r.set() == Set::Slow) {
        for (n, kinds) in row.jobs().into_iter().filter(|&(n, _)| n <= SLOW_MAX_N) {
            cases += 1;
            assert!(
                !allowed(&Ok("Range(StackLimit)".to_owned()), kinds),
                "{} n {n}",
                row.name
            );
            assert!(
                allowed(&Ok(kinds[0].to_owned()), kinds),
                "{} n {n}",
                row.name
            );
            assert!(!allowed(&Err("no HOLES line".to_owned()), kinds));
        }
    }
    assert_eq!(cases, 7, "the slow cases up to SLOW_MAX_N");
}
