//! Stack probe: measures stack bytes per depth unit for every nesting form and every phase.
//! The constants in `stack.rs` come from its output. Run it on demand:
//!
//! ```text
//! cargo test -p ep9_frontend           probe::measure -- --ignored --nocapture   (debug)
//! cargo test -p ep9_frontend --release probe::measure -- --ignored --nocapture   (release)
//! ```
//!
//! Method: stack painting. A thread with a 2 GiB stack fills the region below its stack pointer
//! with a pattern, runs one phase, and finds the lowest word that changed. Two nesting counts
//! give the slope (bytes per nesting level), which removes the fixed base cost.

use std::fmt::Write as _;
use std::hint::black_box;

use oxc_allocator::Allocator;

use crate::forms::{FORMS, Form};
use crate::{Goal, SourceOpts, Strictness, depth, prescan};

const PATTERN: u64 = 0x5A5A_C3C3_A5A5_3C3C;
/// Bytes between the measuring frame and the painted region (the probe's own frame, red zone).
const GAP: usize = 16 << 10;

/// Paint `bytes` of stack below the caller, run `f`, return the deepest stack use of `f` in bytes.
///
/// Panics if `f` used the whole painted region (the measurement would be too low).
#[inline(never)]
fn measure<R>(bytes: usize, f: impl FnOnce() -> R) -> (usize, R) {
    let (used, result, full) = measure_or_full(bytes, f);
    assert!(!full, "the painted region ({bytes} B) is too small");
    (used, result)
}

/// As [`measure`], but reports a full region instead of panicking.
///
/// The stack is touched from the anchor down, 8 bytes at a time (reads in the gap, then the
/// paint from `high` down to `low`), because Windows commits a thread stack one guard page at
/// a time from the top and a first touch below the guard page is an access violation.
#[inline(never)]
fn measure_or_full<R>(bytes: usize, f: impl FnOnce() -> R) -> (usize, R, bool) {
    let anchor = 0u8;
    let sp = black_box(&raw const anchor) as usize;
    let low = (sp - bytes) & !7;
    let high = (sp - GAP) & !7;
    let mut addr = sp & !7;
    while addr > low {
        addr -= 8;
        if addr >= high {
            // SAFETY: `[high, sp)` (the gap) lies inside this thread's stack just below the
            // anchor; a volatile read changes nothing. On Windows it commits the gap pages in
            // order.
            unsafe { std::ptr::read_volatile(addr as *const u64) };
        } else {
            // SAFETY: `[low, high)` lies inside this thread's stack (the probe thread has a
            // 2 GiB stack and `bytes` is at most 1.5 GiB), below the current stack pointer
            // minus the 16 KiB `GAP`, so no live frame and no red zone uses it. Volatile writes
            // keep the compiler from eliding or reordering them.
            unsafe { std::ptr::write_volatile(addr as *mut u64, PATTERN) };
        }
    }
    let result = black_box(f());
    let mut addr = low;
    // SAFETY: as above: reads of this thread's own stack below every live frame.
    while addr < high && unsafe { std::ptr::read_volatile(addr as *const u64) } == PATTERN {
        addr += 8;
    }
    (sp.saturating_sub(addr), result, addr == low)
}

/// T-RED (l1.rs): paint `bytes` of stack below the caller, run `f`, and give the lowest address
/// that `f` touched. The paint and touch order of [`measure_or_full`] (top down, for Windows).
///
/// Panics if `f` used the whole painted region.
#[inline(never)]
pub(crate) fn deepest_point<R>(bytes: usize, f: impl FnOnce() -> R) -> (usize, R) {
    let anchor = 0u8;
    let sp = black_box(&raw const anchor) as usize;
    let low = (sp - bytes) & !7;
    let high = (sp - GAP) & !7;
    let mut addr = sp & !7;
    while addr > low {
        addr -= 8;
        if addr >= high {
            // SAFETY: as in `measure_or_full`: the gap below the anchor, a read.
            unsafe { std::ptr::read_volatile(addr as *const u64) };
        } else {
            // SAFETY: as in `measure_or_full`: `[low, high)` lies in this thread's stack (the
            // caller keeps `bytes` above the guard pages), below every live frame.
            unsafe { std::ptr::write_volatile(addr as *mut u64, PATTERN) };
        }
    }
    let result = black_box(f());
    let mut addr = low;
    // SAFETY: as above: reads of this thread's own stack below every live frame.
    while addr < high && unsafe { std::ptr::read_volatile(addr as *const u64) } == PATTERN {
        addr += 8;
    }
    assert!(addr > low, "the painted region ({bytes} B) is too small");
    (addr, result)
}

#[derive(Clone, Copy, Debug, Default)]
struct Sample {
    estimate: u32,
    ast_depth: u32,
    prescan: usize,
    parse: usize,
    depth_pass: usize,
    semantic: usize,
    scope: usize,
}

/// The parse call of the worker (`crate::parse`, lib.rs): `regex_check::parse` with the
/// source type of the goal. The whole call runs on the worker stack, so the probe measures the
/// whole call: in debug builds pass 1 (tokens off, own arena) and pass 2 (tokens on), in
/// release pass 2 only, then the regex literal check over the tokens (regex parsing is off in
/// oxc, so a regex pattern is parsed in that check, not in the parser). The parser config is
/// the production one, `L1Config` (layer 1), so the probe measures its frames; the probe thread
/// has the limit of its own stack (`l1::LimitScope::of`).
fn worker_parse<'a>(
    allocator: &'a Allocator,
    src: &'a str,
    module: bool,
) -> Result<oxc_parser::ParserReturn<'a>, crate::regex_check::Stopped> {
    let source_type = if module {
        oxc_span::SourceType::mjs()
    } else {
        oxc_span::SourceType::script()
    };
    crate::regex_check::parse(allocator, src, source_type)
}

/// The parse of `form` at `n` must have no error (a fatal one or a diagnostic).
fn assert_parsed<'a>(
    form: &Form,
    n: usize,
    parsed: Result<oxc_parser::ParserReturn<'a>, crate::regex_check::Stopped>,
) -> oxc_parser::ParserReturn<'a> {
    let parsed = parsed.unwrap_or_else(|stopped| panic!("form {} n {n}: {stopped:?}", form.name));
    assert!(
        !parsed.diagnostics.has_errors(),
        "form {} n {n}: {:?}",
        form.name,
        parsed.diagnostics.errors().next()
    );
    parsed
}

fn sample(form: &Form, n: usize, paint: usize) -> Sample {
    let src = (form.build)(n);
    let opts = if form.module {
        SourceOpts::module()
    } else {
        SourceOpts::script(Strictness::Sloppy)
    };
    let module = opts.goal() == Goal::Module;
    let (prescan_bytes, est) = measure(paint, || prescan::estimate(src.as_bytes(), module));
    let allocator = Allocator::default();
    // The parse as the worker runs it (see `worker_parse`): the T-A slope is of that call.
    let (parse_bytes, parsed) = measure(paint, || worker_parse(&allocator, &src, module));
    let program = assert_parsed(form, n, parsed).program;
    // At `AST_CAP`, as `check_depth` calls it: on this 2 GiB thread that is the copy with no
    // backstop, the one that runs on the worker.
    let (depth_bytes, ast) = measure(paint, || depth::check_ast_depth(&program, crate::AST_CAP));
    let ast_depth = ast.expect("within the cap");
    let (semantic_bytes, built) = measure(paint, || {
        oxc_semantic::SemanticBuilder::new_compiler().build(&program)
    });
    assert!(
        !built.diagnostics.has_errors(),
        "form {} n {n}: {:?}",
        form.name,
        built.diagnostics.errors().next()
    );
    let semantic = built.semantic;
    let fe = crate::FrontEnd {
        source: &src,
        opts,
        program: &program,
        semantic: &semantic,
    };
    let (scope_bytes, scopes) = measure(paint, || crate::scope::analyze(&fe));
    assert!(scopes.is_ok(), "form {} n {n}: {scopes:?}", form.name);
    Sample {
        estimate: est.depth,
        ast_depth,
        prescan: prescan_bytes,
        parse: parse_bytes,
        depth_pass: depth_bytes,
        semantic: semantic_bytes,
        scope: scope_bytes,
    }
}

fn per(delta_bytes: usize, delta_units: u32) -> String {
    if delta_units == 0 {
        if delta_bytes > 4096 {
            format!("{delta_bytes}B/0!")
        } else {
            "-".into()
        }
    } else {
        format!("{:.0}", delta_bytes as f64 / f64::from(delta_units))
    }
}

/// Stack growth from n1 to n2 that is noise, not a slope (one page).
const FLAT: usize = 4 << 10;

/// Parse stack one pre-scan unit may use: its share of the worker stack budget (`BUDGET_BYTES`,
/// the red zone of layer 1 is not budget), keeping the 1.5x margin of `stack.rs` (7,559 B
/// debug, 2,615 B release).
const PARSE_UNIT_BUDGET: usize = crate::stack::BUDGET_BYTES / 3 * 2 / crate::PRESCAN_CAP as usize;

/// Pass stack one AST level may use: its share of the worker stack budget, keeping the 1.5x
/// margin of `stack.rs`, as `bytecodegen::stack_tests` does for codegen (9,449 B debug, 3,269 B
/// release).
const PASS_LEVEL_BUDGET: usize = crate::stack::BUDGET_BYTES / 3 * 2 / crate::AST_CAP as usize;

/// Pass stack one AST level may use in a walk after layer 2: the unit of the run-time budget
/// check of layer 1 (`stack::PASS_PER_LEVEL`, the worst pass row of the 4 targets). It is below
/// the budget share, so a walk that fits it fits the budget too.
const PASS_UNIT: usize = crate::stack::PASS_PER_LEVEL;

// T-BUDGET: each share is a share of `BUDGET_BYTES`, never of the red zone.
const _: () = {
    assert!(PARSE_UNIT_BUDGET * crate::PRESCAN_CAP as usize <= crate::stack::BUDGET_BYTES);
    assert!(PASS_LEVEL_BUDGET * crate::AST_CAP as usize <= crate::stack::BUDGET_BYTES);
    assert!(PASS_UNIT <= PASS_LEVEL_BUDGET);
};

/// `None` if `delta_bytes` over `delta_units` fits `budget` per unit; else what is wrong. No
/// units means the stack must stay flat (the cap counts nothing for this form).
fn over_budget(delta_bytes: usize, delta_units: u32, budget: usize) -> Option<String> {
    if delta_units == 0 {
        (delta_bytes > FLAT).then(|| format!("{delta_bytes} B with no units"))
    } else {
        let per = delta_bytes / delta_units as usize;
        (per > budget).then(|| format!("{per} B/unit > budget {budget}"))
    }
}

/// The 4 walks of the eval path after layer 2 (`eval::measure_walks`): the T-PASS columns.
const WALKS: [&str; 4] = [
    "Allowed::collect",
    "Shim::patch",
    "rewrite_completion",
    "post_checks",
];

/// The text of a form at n levels.
type Build = fn(usize) -> String;

/// T-COVER: the class-B walks of oxc (the cover grammar of arrow heads and assignment targets),
/// at n levels.
const COVER: [(&str, Build); 4] = [
    ("([[[a]]])=>0", |n| {
        format!("({}a{})=>0", "[".repeat(n), "]".repeat(n))
    }),
    ("({a:{a:b}})=>0", |n| {
        format!("({}b{})=>0", "{a:".repeat(n), "}".repeat(n))
    }),
    ("[[[a]]]=0", |n| {
        format!("{}a{}=0", "[".repeat(n), "]".repeat(n))
    }),
    ("((((a))))=0", |n| {
        format!("{}a{}=0", "(".repeat(n), ")".repeat(n))
    }),
];

/// T-A: the final assert of `measure_forms` runs only when this is true.
/// On since the debug unit of `stack.rs` comes from T-A on the 4 CI targets (H01 B2).
const T_A_ASSERT: bool = true;

/// Measures every form, and fails when a slope does not fit its share of the worker stack or
/// its unit: parse per pre-scan unit (the whole worker parse call, [`worker_parse`]); a regex
/// row per group, against the unit of the regex budget check (`stack::REGEX_PER_GROUP`); the
/// depth pass, oxc_semantic, the scope pass and the 4 walks of the eval path per AST level,
/// against the unit of the run-time budget check (`stack::PASS_PER_LEVEL`).
#[test]
#[ignore = "measurement; run on demand with --ignored --nocapture"]
fn measure_forms() {
    let (n1, n2) = (500usize, 1500usize);
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let (out, over, worst, regex_worst, eval_out, eval_over, eval_worst, cover_out) =
        std::thread::Builder::new()
        .stack_size(2 << 30)
        .spawn(move || {
            // Layer 1: this thread's limit, as a worker has one (l1.rs).
            let _limit = crate::l1::LimitScope::of(2 << 30);
            let mut over = Vec::new();
            // T-A: the worst parse slope per pre-scan unit, rounded up to a whole byte.
            let mut worst: Option<(usize, &'static str)> = None;
            // The worst regex row, per group.
            let mut regex_worst: Option<(usize, &'static str)> = None;
            let mut out = String::new();
            writeln!(
                out,
                "profile {profile}, target {}, n = {n1} -> {n2}",
                std::env::consts::ARCH
            )
            .unwrap();
            writeln!(
                out,
                "| form | dE/dn | dAST/dn | parse B/E | parse B/n | depth B/AST | semantic B/AST | scope B/AST | E@n2 | AST@n2 | parse base | semantic base | scope base | prescan B@n2 |"
            )
            .unwrap();
            writeln!(out, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|").unwrap();
            let mut eval_out = String::new();
            writeln!(eval_out, "| form | {} B/AST |\n|---|---|---|---|---|", WALKS.join(" B/AST | "))
                .unwrap();
            let mut eval_over = Vec::new();
            let mut eval_worst: [Option<(usize, &'static str)>; 4] = [None; 4];
            for form in FORMS {
                let paint = 384 << 20;
                let a = sample(form, n1, paint);
                let b = sample(form, n2, paint);
                let de = b.estimate - a.estimate;
                let dast = b.ast_depth - a.ast_depth;
                let dn = u32::try_from(n2 - n1).unwrap();
                if de > 0 {
                    let slope = b.parse.saturating_sub(a.parse).div_ceil(de as usize);
                    if worst.is_none_or(|(w, _)| slope > w) {
                        worst = Some((slope, form.name));
                    }
                }
                // A regex row: n is the group or class level, the unit of the regex budget
                // check of layer 1 (`stack::REGEX_PER_GROUP`, the worst of the 4 targets). Whole
                // bytes (the floor), as `over_budget`: never above the "parse B/n" column.
                if form.name.starts_with("regex-") {
                    let group = b.parse.saturating_sub(a.parse) / dn as usize;
                    if regex_worst.is_none_or(|(w, _)| group > w) {
                        regex_worst = Some((group, form.name));
                    }
                    if group > crate::stack::REGEX_PER_GROUP {
                        over.push(format!(
                            "{} parse: {group} B/group > REGEX_PER_GROUP {}",
                            form.name,
                            crate::stack::REGEX_PER_GROUP
                        ));
                    }
                }
                // Base = the stack at n2 minus the slope times the units at n2.
                let base = |at_b: usize, at_a: usize, units_b: u32, units_a: u32| -> i64 {
                    let du = i64::from(units_b) - i64::from(units_a);
                    if du == 0 {
                        return i64::try_from(at_b).unwrap();
                    }
                    let slope = (i64::try_from(at_b).unwrap() - i64::try_from(at_a).unwrap()) / du;
                    i64::try_from(at_b).unwrap() - slope * i64::from(units_b)
                };
                writeln!(
                    out,
                    "| {} | {:.2} | {:.2} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                    form.name,
                    f64::from(de) / f64::from(dn),
                    f64::from(dast) / f64::from(dn),
                    per(b.parse.saturating_sub(a.parse), de),
                    per(b.parse.saturating_sub(a.parse), dn),
                    per(b.depth_pass.saturating_sub(a.depth_pass), dast),
                    per(b.semantic.saturating_sub(a.semantic), dast),
                    per(b.scope.saturating_sub(a.scope), dast),
                    b.estimate,
                    b.ast_depth,
                    base(b.parse, a.parse, b.estimate, a.estimate),
                    base(b.semantic, a.semantic, b.ast_depth, a.ast_depth),
                    base(b.scope, a.scope, b.ast_depth, a.ast_depth),
                    // The pre-scan is a loop: its stack does not grow with the input.
                    b.prescan,
                )
                .unwrap();
                let pass = |at_b: usize, at_a: usize| {
                    over_budget(at_b.saturating_sub(at_a), dast, PASS_UNIT)
                };
                for (phase, problem) in [
                    (
                        "parse",
                        over_budget(b.parse.saturating_sub(a.parse), de, PARSE_UNIT_BUDGET),
                    ),
                    ("depth pass", pass(b.depth_pass, a.depth_pass)),
                    ("semantic", pass(b.semantic, a.semantic)),
                    ("scope pass", pass(b.scope, a.scope)),
                    ("pre-scan", over_budget(b.prescan.saturating_sub(a.prescan), 0, 0)),
                ] {
                    if let Some(problem) = problem {
                        over.push(format!("{} {phase}: {problem}", form.name));
                    }
                }
                // T-PASS columns: the 4 walks of the eval path per AST level.
                if !form.module {
                    let walks = |n: usize| {
                        // A lone surrogate (shim text), so that `Shim::patch` walks.
                        let text = format!("\"\u{FDD0}d800\";\n{}", (form.build)(n));
                        crate::eval::measure_walks(&text, &mut |f: &mut dyn FnMut()| {
                            measure(paint, f).0
                        })
                    };
                    match (walks(n1), walks(n2)) {
                        (Some(wa), Some(wb)) => {
                            let cols: Vec<String> =
                                (0..4).map(|k| per(wb[k].saturating_sub(wa[k]), dast)).collect();
                            writeln!(eval_out, "| {} | {} |", form.name, cols.join(" | "))
                                .unwrap();
                            for k in 0..4 {
                                if let Some(problem) =
                                    over_budget(wb[k].saturating_sub(wa[k]), dast, PASS_UNIT)
                                {
                                    eval_over.push(format!("{} {}: {problem}", form.name, WALKS[k]));
                                }
                                if dast > 0 {
                                    let slope = wb[k].saturating_sub(wa[k]) / dast as usize;
                                    if eval_worst[k].is_none_or(|(w, _)| slope > w) {
                                        eval_worst[k] = Some((slope, form.name));
                                    }
                                }
                            }
                        }
                        _ => writeln!(eval_out, "| {} | no eval code | | | |", form.name).unwrap(),
                    }
                }
            }
            // T-COVER: the cover walks of oxc (class B sites) per level, in the whole parse.
            let mut cover_out = String::new();
            for (name, build) in COVER {
                let bytes = |n: usize| {
                    let src = build(n);
                    let allocator = Allocator::default();
                    let (used, parsed) = measure(384 << 20, || worker_parse(&allocator, &src, false));
                    assert!(parsed.is_ok_and(|p| !p.diagnostics.has_errors()), "T-COVER {name}");
                    used
                };
                let level = (bytes(n2).saturating_sub(bytes(n1))) / (n2 - n1);
                writeln!(cover_out, "T-COVER {profile} {name}: {level} B per level (parse)").unwrap();
            }
            (
                out,
                over,
                worst,
                regex_worst,
                eval_out,
                eval_over,
                eval_worst,
                cover_out,
            )
        })
        .unwrap()
        .join()
        .unwrap();
    println!("{out}");
    let (w, name) = worst.expect("a form with a growing estimate");
    println!(
        "T-A worst {}-{} {profile}: {w} B per pre-scan unit (form {name}); budget {PARSE_UNIT_BUDGET} B",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let (w, name) = regex_worst.expect("a regex form");
    println!(
        "T-A regex worst {}-{} {profile}: {w} B per group (form {name}); REGEX_PER_GROUP {} B",
        std::env::consts::OS,
        std::env::consts::ARCH,
        crate::stack::REGEX_PER_GROUP
    );
    for line in &over {
        println!("T-A over: {line}");
    }
    // T-PASS: the run-time budget check of layer 1 takes PASS_PER_LEVEL as the worst of these
    // columns and the pass columns above.
    println!("T-PASS eval walks per AST level:\n{eval_out}");
    for (k, worst) in eval_worst.iter().enumerate() {
        if let Some((w, name)) = worst {
            println!(
                "T-PASS worst {}-{} {profile}: {} {w} B per AST level (form {name}); unit {PASS_UNIT} B",
                std::env::consts::OS,
                std::env::consts::ARCH,
                WALKS[k]
            );
        }
    }
    for line in &eval_over {
        println!("T-PASS over: {line}");
    }
    print!("{cover_out}");
    if T_A_ASSERT {
        assert!(
            over.is_empty() && eval_over.is_empty(),
            "stack slopes above the worker stack share or the unit:\n{}",
            over.iter()
                .chain(&eval_over)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// Literal starts (regex, string, template) in oxc's AST.
#[derive(Default)]
struct OxcLiterals(Vec<(prescan::LitKind, u32)>);

impl<'a> oxc_ast_visit::Visit<'a> for OxcLiterals {
    fn visit_reg_exp_literal(&mut self, it: &oxc_ast::ast::RegExpLiteral<'a>) {
        self.0.push((prescan::LitKind::Regex, it.span.start));
    }
    fn visit_string_literal(&mut self, it: &oxc_ast::ast::StringLiteral<'a>) {
        self.0.push((prescan::LitKind::String, it.span.start));
    }
    fn visit_template_literal(&mut self, it: &oxc_ast::ast::TemplateLiteral<'a>) {
        self.0.push((prescan::LitKind::Template, it.span.start));
        oxc_ast_visit::walk::walk_template_literal(self, it);
    }
}

/// A corpus check over no files proves nothing: fail when a checkout under `third_party/` is
/// missing or empty, and name the script that syncs it.
pub(crate) fn assert_checkout(dir: &str, files: usize) {
    let script = if dir.starts_with("test262") {
        "scripts/sync-test262.sh"
    } else {
        "scripts/sync-bench.sh"
    };
    assert!(
        files > 0,
        "third_party/{dir} has no .js or .mjs file: run `bash {script}` first"
    );
}

fn corpus_files() -> Vec<(std::path::PathBuf, bool)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party");
    let mut out = Vec::new();
    for top in [
        "test262/test",
        "test262/harness",
        "bench-octane",
        "bench-v8v7",
        "bench-jetstream",
    ] {
        let before = out.len();
        let mut stack = vec![root.join(top)];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    if path
                        .file_name()
                        .is_some_and(|n| n == ".git" || n == "node_modules")
                    {
                        continue;
                    }
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "js" || e == "mjs") {
                    let module = path.extension().is_some_and(|e| e == "mjs");
                    out.push((path, module));
                }
            }
        }
        assert_checkout(top, out.len() - before);
    }
    out
}

/// Parse stack that does not grow with the estimate: the 16 KiB `GAP` that every measurement
/// includes, oxc_parser's base and the regex parser's base. Corpus worst against
/// `PARSE_UNIT_BUDGET` (aarch64, 2026-10-21): 13,776 B release (an empty file), 35,568 B debug
/// (a `\p{...}` regex).
const PARSE_BASE: usize = 64 << 10;

/// Every valid file of test262 and the benchmark pins: the scanner finds exactly oxc's regex,
/// string and template literals, and the real parse stack stays within the model
/// `PARSE_BASE + estimate x PARSE_UNIT_BUDGET`. Run on demand (needs the third_party
/// checkouts; fails without them):
/// `cargo test -p ep9_frontend --release probe::scanner -- --ignored --nocapture`
#[test]
#[ignore = "corpus check; needs third_party checkouts; run on demand"]
fn scanner_matches_oxc_on_corpus() {
    let files = corpus_files();
    let (report, checked, mismatches, over_model) = std::thread::Builder::new()
        .stack_size(2 << 30)
        .spawn(move || {
            let mut checked = 0usize;
            let mut literals = 0usize;
            let mut mismatches: Vec<String> = Vec::new();
            let mut over_model: Vec<String> = Vec::new();
            let mut worst_residual: (i64, String, u32, usize) = (i64::MIN, String::new(), 0, 0);
            let mut worst_per_unit: (f64, String, u32, usize) = (0.0, String::new(), 0, 0);
            let unit = i64::try_from(PARSE_UNIT_BUDGET).unwrap();
            for (path, ext_module) in &files {
                let Ok(src) = std::fs::read_to_string(path) else {
                    continue;
                };
                let module = *ext_module
                    || src.contains("flags: [module]")
                    || (src.contains("flags:")
                        && src.contains("module")
                        && src.contains("---*/")
                        && src
                            .split("---*/")
                            .next()
                            .is_some_and(|fm| fm.contains("module")));
                let negative = src.contains("negative:");
                if negative {
                    continue;
                }
                let source_type = if module {
                    oxc_span::SourceType::mjs()
                } else {
                    oxc_span::SourceType::script()
                };
                let est = prescan::estimate(src.as_bytes(), module);
                let paint = (512usize << 10).max((usize::try_from(est.depth).unwrap() * 8) << 10);
                let allocator = Allocator::default();
                let options = oxc_parser::ParseOptions {
                    parse_regular_expression: true,
                    ..oxc_parser::ParseOptions::default()
                };
                let (used, parsed, full) = measure_or_full(paint, || {
                    oxc_parser::Parser::new(&allocator, &src, source_type)
                        .with_options(options)
                        .parse()
                });
                assert!(!full, "{}: painted region too small", path.display());
                if parsed.diagnostics.has_errors() {
                    continue;
                }
                checked += 1;
                let name = path.display().to_string();
                let residual = i64::try_from(used).unwrap() - unit * i64::from(est.depth);
                if residual > worst_residual.0 {
                    worst_residual = (residual, name.clone(), est.depth, used);
                }
                if residual > i64::try_from(PARSE_BASE).unwrap() {
                    over_model.push(format!(
                        "{name}: estimate {}, used {used} B, residual {residual} B",
                        est.depth
                    ));
                }
                if est.depth > 20 {
                    let per_unit = used as f64 / f64::from(est.depth);
                    if per_unit > worst_per_unit.0 {
                        worst_per_unit = (per_unit, name.clone(), est.depth, used);
                    }
                }
                let mut oxc = OxcLiterals::default();
                oxc_ast_visit::Visit::visit_program(&mut oxc, &parsed.program);
                let mut theirs = oxc.0;
                theirs.sort_unstable();
                theirs.dedup();
                let mut ours = prescan::literal_tokens(src.as_bytes(), module);
                ours.sort_unstable();
                literals += theirs.len();
                if ours != theirs {
                    let missing: Vec<_> = theirs
                        .iter()
                        .filter(|t| !ours.contains(t))
                        .take(3)
                        .collect();
                    let extra: Vec<_> = ours
                        .iter()
                        .filter(|t| !theirs.contains(t))
                        .take(3)
                        .collect();
                    let show = |at: u32| {
                        let at = at as usize;
                        let from = src.floor_char_boundary(at.saturating_sub(30));
                        let to = src.ceil_char_boundary((at + 30).min(src.len()));
                        src[from..to].replace('\n', "\\n")
                    };
                    let detail = missing
                        .iter()
                        .map(|(k, at)| format!("missing {k:?}@{at} `{}`", show(*at)))
                        .chain(
                            extra
                                .iter()
                                .map(|(k, at)| format!("extra {k:?}@{at} `{}`", show(*at))),
                        )
                        .collect::<Vec<_>>()
                        .join("; ");
                    mismatches.push(format!("{name}: {detail}"));
                }
            }
            let mut out = String::new();
            writeln!(
                out,
                "files {} (valid, checked {checked}), oxc literals {literals}",
                files.len()
            )
            .unwrap();
            writeln!(out, "token mismatches: {}", mismatches.len()).unwrap();
            for m in mismatches.iter().take(40) {
                writeln!(out, "  {m}").unwrap();
            }
            writeln!(
                out,
                "worst residual (used - {unit} x estimate): {} B in {} (estimate {}, used {})",
                worst_residual.0, worst_residual.1, worst_residual.2, worst_residual.3
            )
            .unwrap();
            writeln!(
                out,
                "worst bytes per unit (estimate > 20): {:.0} in {} (estimate {}, used {})",
                worst_per_unit.0, worst_per_unit.1, worst_per_unit.2, worst_per_unit.3
            )
            .unwrap();
            (out, checked, mismatches, over_model)
        })
        .unwrap()
        .join()
        .unwrap();
    println!("{report}");
    // Coverage: files oxc rejects (negative tests, module code run as a script) are skipped.
    assert_eq!(
        checked, 49_522,
        "files checked changed: review the skipped files, then update this count"
    );
    assert!(
        mismatches.is_empty(),
        "{} files where the scanner's literal tokens differ from oxc:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
    assert!(
        over_model.is_empty(),
        "{} files whose parse stack is above {PARSE_BASE} + estimate x {PARSE_UNIT_BUDGET} B:\n{}",
        over_model.len(),
        over_model.join("\n")
    );
}

/// test262 front matter: (module goal, strict only).
fn test262_flags(src: &str, ext_module: bool) -> (bool, bool) {
    let front = src
        .split("/*---")
        .nth(1)
        .and_then(|rest| rest.split("---*/").next())
        .unwrap_or("");
    let flags = front
        .lines()
        .find(|l| l.trim_start().starts_with("flags:"))
        .unwrap_or("");
    (
        ext_module || flags.contains("module"),
        flags.contains("onlyStrict"),
    )
}

/// Every valid file of test262 and the benchmark pins through the scope pass: no panic, no
/// abort, every `IdentifierReference` visited, the same declaration as oxc's resolution (or
/// one of the reviewed differences), and the same result twice (determinism). Run on demand
/// (needs the third_party checkouts; fails without them):
/// `cargo test -p ep9_frontend --release probe::scope_pass -- --ignored --nocapture`
#[test]
#[ignore = "corpus check; needs third_party checkouts; run on demand"]
fn scope_pass_matches_oxc_on_corpus() {
    use std::time::{Duration, Instant};

    let files = corpus_files();
    let (mut ok, mut rejected, mut references, mut unvisited, mut not_equal) = (0, 0, 0, 0, 0);
    let mut time = Duration::ZERO;
    let mut mismatches: Vec<String> = Vec::new();
    let mut by_why = std::collections::BTreeMap::new();
    let mut rejected_by = std::collections::BTreeMap::new();
    for (path, ext_module) in &files {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        if src.contains("negative:") {
            continue;
        }
        let (module, only_strict) = test262_flags(&src, *ext_module);
        let opts = if module {
            SourceOpts::module()
        } else if only_strict {
            SourceOpts::script(Strictness::Strict)
        } else {
            SourceOpts::script(Strictness::Sloppy)
        };
        let owned = src.clone();
        let result = crate::with_front_end(&src, opts, move |fe| {
            let start = Instant::now();
            let first = crate::scope::analyze(fe)?;
            let elapsed = start.elapsed();
            let second = crate::scope::analyze(fe)?;
            let (found, missed) = crate::scope::tests::oxc_mismatches(fe, &first);
            let detail: Vec<(crate::scope::tests::Why, String)> = found
                .iter()
                .map(|m| {
                    let at = m.at as usize;
                    let from = owned.floor_char_boundary(at.saturating_sub(40));
                    let to = owned.ceil_char_boundary((at + 30).min(owned.len()));
                    (
                        m.why,
                        format!(
                            "{:?} {} @{} ep9 {:?} oxc {:?} `{}`",
                            m.why,
                            m.name,
                            m.at,
                            m.ep9,
                            m.oxc,
                            owned[from..to].replace('\n', "\\n")
                        ),
                    )
                })
                .collect();
            Ok((
                elapsed,
                first.references().len(),
                missed,
                first == second,
                detail,
            ))
        });
        match result {
            Ok((elapsed, refs, missed, equal, detail)) => {
                ok += 1;
                time += elapsed;
                references += refs;
                unvisited += missed;
                not_equal += usize::from(!equal);
                for (why, d) in detail {
                    *by_why.entry(why).or_insert(0usize) += 1;
                    mismatches.push(format!("{}: {d}", path.display()));
                }
            }
            Err(e) => {
                rejected += 1;
                let key = match &e {
                    crate::CompileError::Syntax(s) => format!("syntax: {}", s.first().message),
                    crate::CompileError::Range(r) => format!("range: {:?}", r.kind),
                };
                let short = path.display().to_string();
                let short = short
                    .split("third_party/")
                    .nth(1)
                    .unwrap_or(&short)
                    .to_owned();
                rejected_by.entry(key).or_insert_with(Vec::new).push(short);
            }
        }
    }
    println!("files analyzed {ok}, rejected by the front end {rejected}");
    for (key, files) in &rejected_by {
        println!("  rejected {} x {key}, e.g. {}", files.len(), files[0]);
    }
    println!(
        "references {references}; oxc references or scopes with no ep9 counterpart {unvisited}; different on a second run {not_equal}"
    );
    println!("scope pass time {:.2?} total", time);
    println!(
        "references resolved unlike oxc: {} {by_why:?}",
        mismatches.len()
    );
    // One example per file, with the count.
    let mut per_file: Vec<(String, usize, String)> = Vec::new();
    for m in &mismatches {
        let (file, detail) = m.split_once(": ").unwrap();
        match per_file.last_mut() {
            Some(last) if last.0 == file => last.1 += 1,
            _ => per_file.push((file.to_owned(), 1, detail.to_owned())),
        }
    }
    println!("files with differences: {}", per_file.len());
    for (file, count, example) in per_file
        .iter()
        .filter(|(_, _, e)| e.starts_with("Other"))
        .chain(per_file.iter().take(3))
    {
        let short = file.split("third_party/").nth(1).unwrap_or(file);
        println!("  {short} x{count}: {example}");
    }
    // Coverage: a rejected file drops out of every check below. At the pinned corpus the
    // rejections are module code run as a script (`_FIXTURE.js` files and bench modules carry
    // no module flag) and the annexB call-expression assignment targets oxc rejects.
    assert_eq!(
        (ok, rejected),
        (49_517, 267),
        "files analyzed / rejected changed: review the rejections above, then update these \
         counts and H01"
    );
    assert_eq!(
        unvisited, 0,
        "oxc references or scopes with no ep9 counterpart (BytecodeGen looks them up)"
    );
    assert_eq!(
        not_equal, 0,
        "files with a different ScopeAnalysis on the second run"
    );
    // Only the reviewed classes, at their reviewed counts (H01 "F1 corpus"). `Why::Other` is a
    // bug in ep9 or in oxc.
    let reviewed = std::collections::BTreeMap::from([
        (crate::scope::tests::Why::SplitBody, 54),
        (crate::scope::tests::Why::AnnexB, 488),
        (crate::scope::tests::Why::CatchVar, 4),
    ]);
    assert_eq!(
        by_why, reviewed,
        "references resolved unlike oxc changed: review the examples above, then update this \
         table and H01"
    );
}

/// The argument text of every `eval(`, `Function(` and `evalScript(` call in `src`.
fn eval_arguments(src: &str) -> Vec<&str> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    for needle in ["eval(", "Function(", "evalScript("] {
        let mut from = 0;
        while let Some(i) = src[from..].find(needle) {
            let at = from + i;
            from = at + needle.len();
            // `eval(` must not be the tail of a longer identifier (`myeval(`).
            if needle == "eval("
                && at > 0
                && (bytes[at - 1].is_ascii_alphanumeric()
                    || bytes[at - 1] == b'_'
                    || bytes[at - 1] == b'$')
            {
                continue;
            }
            let open = at + needle.len() - 1;
            if let Some(close) = matching_paren(bytes, open) {
                out.push(&src[open + 1..close]);
            }
        }
    }
    out
}

/// Index of the `)` that closes the `(` at `open`, skipping strings, templates and comments.
fn matching_paren(b: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            q @ (b'"' | b'\'' | b'`') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Code units of `\uXXXX` / `\u{X}` escapes with an odd number of backslashes (real escapes in
/// the literal, not escaped backslashes), as (start, end, value).
fn unicode_escapes(text: &str) -> Vec<(usize, usize, u32)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && b[i] == b'\\' {
            i += 1;
        }
        if (i - start) % 2 == 0 || b.get(i) != Some(&b'u') {
            continue;
        }
        let esc_start = i - 1;
        i += 1;
        let (digits, end) = if b.get(i) == Some(&b'{') {
            let close = text[i..].find('}').map_or(i, |c| i + c);
            (&text[i + 1..close], close + 1)
        } else {
            (&text[i..(i + 4).min(text.len())], i + 4)
        };
        if let Ok(v) = u32::from_str_radix(digits, 16) {
            out.push((esc_start, end.min(text.len()), v));
        }
        i = end.min(text.len());
    }
    out
}

/// The literal text contains a surrogate escape that is not half of a lead+trail pair.
fn has_lone_surrogate_escape(text: &str) -> bool {
    let units = unicode_escapes(text);
    let lead = |v: u32| (0xD800..=0xDBFF).contains(&v);
    let trail = |v: u32| (0xDC00..=0xDFFF).contains(&v);
    units.iter().enumerate().any(|(k, &(start, end, v))| {
        if lead(v) {
            !units
                .get(k + 1)
                .is_some_and(|&(s, _, w)| s == end && trail(w))
        } else if trail(v) {
            !(k > 0 && units[k - 1].1 == start && lead(units[k - 1].2))
        } else {
            false
        }
    })
}

/// P4 baseline (01-front-end §7): test262 tests that pass source text with a lone surrogate to
/// `eval`, `Function` or `$262.evalScript`. Such source cannot be UTF-8; it needs the shim.
/// Fails without the test262 checkout, and when the set differs from the reviewed baseline.
/// `cargo test -p ep9_frontend --release probe::p4 -- --ignored --nocapture`
#[test]
#[ignore = "corpus count; needs third_party/test262; run on demand"]
fn p4_lone_surrogate_eval_count() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/test262/test");
    let mut files = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for path in entries.flatten().map(|e| e.path()) {
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "js") {
                files.push(path);
            }
        }
    }
    assert_checkout("test262/test", files.len());
    files.sort();
    let (mut literal, mut generated, mut possible) = (Vec::new(), Vec::new(), Vec::new());
    // A `for` header that runs up to 0xFFFF, or code units built from 4 hex digits (S7.4_A5).
    let loops_all_code_units = |src: &str| {
        let bounded_loop = src.lines().any(|line| {
            line.trim_start().starts_with("for")
                && ["0xffff", "0xFFFF", "65535", "0x10000", "65536"]
                    .iter()
                    .any(|n| line.contains(n))
        });
        (src.contains("fromCharCode") || src.contains("fromCodePoint"))
            && (bounded_loop || src.contains("fromCharCode(\"0x\""))
    };
    for path in &files {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let args = eval_arguments(&src);
        if args.is_empty() {
            continue;
        }
        let name = path
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let computed = |a: &&str| {
            a.contains('+') || a.contains("${") || !a.trim_start().starts_with(['"', '\'', '`'])
        };
        if args.iter().any(|a| has_lone_surrogate_escape(a)) {
            literal.push(name);
        } else if args.iter().any(computed) && loops_all_code_units(&src) {
            generated.push(name);
        } else if has_lone_surrogate_escape(&src) {
            possible.push(name);
        }
    }
    println!("test262 files scanned: {}", files.len());
    println!(
        "A. lone surrogate in a literal eval/Function argument: {}",
        literal.len()
    );
    for f in &literal {
        println!("  {f}");
    }
    println!(
        "B. eval of String.fromCharCode over 0..0xFFFF: {}",
        generated.len()
    );
    for f in &generated {
        println!("  {f}");
    }
    println!(
        "P4 baseline count (A + B): {}",
        literal.len() + generated.len()
    );
    println!(
        "not counted, review by hand (eval call + lone surrogate elsewhere): {}",
        possible.len()
    );
    for f in &possible {
        println!("  {f}");
    }
    // The reviewed baseline (H05 "P4 baseline"): a change needs a new review by hand.
    let review =
        "the P4 set changed: review the files above by hand, then update this list and H05";
    assert_eq!(
        literal,
        [
            "language/literals/regexp/named-groups/invalid-lone-surrogate-groupname.js",
            "staging/sm/RegExp/unicode-class-raw.js",
            "staging/sm/RegExp/unicode-raw.js",
        ],
        "{review}"
    );
    assert_eq!(
        generated,
        [
            "annexB/built-ins/RegExp/RegExp-leading-escape-BMP.js",
            "annexB/built-ins/RegExp/RegExp-trailing-escape-BMP.js",
            "language/comments/S7.4_A5.js",
            "language/comments/S7.4_A6.js",
            "language/literals/regexp/S7.8.5_A1.1_T2.js",
            "language/literals/regexp/S7.8.5_A1.4_T2.js",
            "language/literals/regexp/S7.8.5_A2.1_T2.js",
            "language/literals/regexp/S7.8.5_A2.4_T2.js",
        ],
        "{review}"
    );
    assert_eq!(
        possible,
        [
            "staging/sm/RegExp/unicode-braced.js",
            "staging/sm/RegExp/unicode-class-braced.js",
            "staging/sm/RegExp/unicode-class-lead-trail.js",
            "staging/sm/RegExp/unicode-lead-trail.js",
            "staging/sm/String/unicode-braced.js",
        ],
        "{review}"
    );
}

/// T-D: the worker parse call of every form at the pre-scan cap (the largest n with estimate <=
/// `PRESCAN_CAP`, the fit of `depth_tests::check_prescan_cap_form`) fits the worker stack
/// budget (`BUDGET_BYTES`) with the 1.5x margin of `stack.rs`. The pre-scan lets such an input reach oxc, so this is the
/// deepest parse the worker can run for the form. The measure is of the whole call of
/// [`worker_parse`] (`regex_check::parse`: in debug both passes, then the regex literal check),
/// as on the worker stack. Forms whose estimate does not grow have no pre-scan cap and are only
/// listed.
#[test]
#[ignore = "measurement; run on demand with --ignored --nocapture"]
fn parse_stack_at_the_cap() {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let paint = crate::stack::WORKER_STACK_BYTES;
    // T-BUDGET: the share of the budget, not of the reservation (RED is not budget).
    let limit = crate::stack::BUDGET_BYTES / 3 * 2;
    let (out, rows, flat) = std::thread::Builder::new()
        .stack_size(2 << 30)
        .spawn(move || {
            // Layer 1: this thread's limit, as a worker has one (l1.rs).
            let _limit = crate::l1::LimitScope::of(2 << 30);
            let mut out = String::new();
            let mut rows = Vec::new();
            let mut flat = Vec::new();
            writeln!(
                out,
                "| form | n | estimate | parse B | full |\n|---|---|---|---|---|"
            )
            .unwrap();
            for form in FORMS {
                let estimate =
                    |n: usize| prescan::estimate((form.build)(n).as_bytes(), form.module).depth;
                let (e10, e11, e17) = (estimate(10), estimate(11), estimate(17));
                let ea = e11 - e10;
                let eb = e10 - 10 * ea;
                assert_eq!(
                    e17,
                    17 * ea + eb,
                    "{}: the estimate is not linear",
                    form.name
                );
                if ea == 0 {
                    flat.push(form.name);
                    continue;
                }
                let n = usize::try_from((crate::PRESCAN_CAP - eb) / ea).unwrap();
                let src = (form.build)(n);
                let e = prescan::estimate(src.as_bytes(), form.module).depth;
                assert!(e <= crate::PRESCAN_CAP, "{} n {n}: estimate {e}", form.name);
                let allocator = Allocator::default();
                // The whole worker parse call (`worker_parse`: both debug passes and the
                // regex literal check).
                let (bytes, parsed, full) =
                    measure_or_full(paint, || worker_parse(&allocator, &src, form.module));
                assert_parsed(form, n, parsed);
                writeln!(out, "| {} | {n} | {e} | {bytes} | {full} |", form.name).unwrap();
                rows.push((form.name, n, e, bytes, full));
            }
            (out, rows, flat)
        })
        .unwrap()
        .join()
        .unwrap();
    println!("{out}");
    println!("no pre-scan cap (estimate does not grow): {flat:?}");
    let &(name, _, _, worst, _) = rows
        .iter()
        .max_by_key(|row| row.3)
        .expect("a form with a growing estimate");
    println!(
        "T-D worst {}-{} {profile}: {worst} B at the cap (form {name}); limit {limit} B",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let over: Vec<String> = rows
        .iter()
        .filter(|row| row.4 || row.3 > limit)
        .map(|&(name, n, e, bytes, full)| {
            format!("{name} n {n} estimate {e}: {bytes} B (full {full}) > limit {limit} B")
        })
        .collect();
    for line in &over {
        println!("T-D over: {line}");
    }
    assert!(
        over.is_empty(),
        "parse stack at the pre-scan cap above BUDGET_BYTES / 1.5:\n{}",
        over.join("\n")
    );
}
