//! Depth guard gate tests (M0 exit item 7, M1 exit item 8): for every nesting form, the deepest
//! input at the cap compiles and one level more is a RangeError, through the public API, in the
//! build profile under test. The test process staying alive is the "no abort" check.

use crate::forms::{FORMS, Form, Gate};
use crate::prescan::{self, REGEX_WEIGHT};
use crate::{
    AST_CAP, CompileError, DepthLayer, PRESCAN_CAP, RangeErrorKind, SourceOpts, Strictness,
    check_early_errors, depth, parse,
};
use oxc_allocator::Allocator;

fn form(name: &str) -> &'static Form {
    FORMS
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("no form {name}"))
}

fn opts(form: &Form) -> SourceOpts {
    if form.module {
        SourceOpts::module()
    } else {
        SourceOpts::script(Strictness::Sloppy)
    }
}

/// AST depth of a small instance (on the test thread; small n only). The parse needs a layer-1
/// limit on this thread (l1.rs); a test thread has at least 2 MiB.
fn ast_depth(form: &Form, n: usize) -> u32 {
    let src = (form.build)(n);
    let allocator = Allocator::default();
    let program = crate::l1::with_limit(2 << 20, || parse(&allocator, &src, opts(form)))
        .expect("small instances parse");
    depth::check_ast_depth(&program, u32::MAX).expect("no cap")
}

fn estimate(form: &Form, n: usize) -> u32 {
    prescan::estimate((form.build)(n).as_bytes(), form.module).depth
}

/// `(a, b)` with `depth(n) = a * n + b`, checked on three points.
fn linear(f: impl Fn(usize) -> u32) -> (u32, u32) {
    let (d10, d11, d17) = (f(10), f(11), f(17));
    let a = d11 - d10;
    let b = d10 - 10 * a;
    assert_eq!(d17, 17 * a + b, "not linear: {d10} {d11} {d17}");
    (a, b)
}

fn layer_of(result: &Result<(), CompileError>) -> Option<DepthLayer> {
    match result {
        Err(CompileError::Range(e)) => match e.kind {
            RangeErrorKind::TooDeep(layer) => Some(layer),
            RangeErrorKind::StackUnavailable => panic!("stack unavailable: {e}"),
            // On the full worker within the budget (Claim D of layer 1).
            RangeErrorKind::StackLimit => panic!("stack limit on the full worker: {e}"),
        },
        _ => None,
    }
}

fn run(form: &Form, n: usize) -> Result<(), CompileError> {
    check_early_errors(&(form.build)(n), opts(form))
}

/// The gate check for one form whose depth grows with the AST.
fn check_ast_form(name: &str) {
    let form = form(name);
    let (a, b) = linear(|n| ast_depth(form, n));
    assert!(a >= 1, "{name}: depth does not grow");
    // The largest n with depth(n) <= AST_CAP.
    let n_cap = usize::try_from((AST_CAP - b) / a).unwrap();
    let depth_at = |n: usize| a * u32::try_from(n).unwrap() + b;
    assert!(depth_at(n_cap) <= AST_CAP && depth_at(n_cap + 1) > AST_CAP);

    // The pre-scan must never reject what layer 2 accepts.
    let (ea, eb) = linear(|n| estimate(form, n));
    let est_cap = ea * u32::try_from(n_cap).unwrap() + eb;
    assert!(
        est_cap <= PRESCAN_CAP,
        "{name}: estimate {est_cap} > PRESCAN_CAP at AST depth {}",
        depth_at(n_cap)
    );

    // cap - 1 (or the instance just below the cap) compiles.
    let below = run(form, n_cap - 1);
    assert!(
        below.is_ok(),
        "{name} n={} depth {}: {below:?}",
        n_cap - 1,
        depth_at(n_cap - 1)
    );
    // The deepest instance at or below the cap compiles.
    let at = run(form, n_cap);
    assert!(
        at.is_ok(),
        "{name} n={n_cap} depth {}: {at:?}",
        depth_at(n_cap)
    );
    // The scope pass (layer 3) also handles the deepest accepted tree on the worker stack.
    let scopes = crate::analyze(&(form.build)(n_cap), opts(form));
    assert!(
        scopes.is_ok(),
        "{name} n={n_cap}: scope pass {:?}",
        scopes.err()
    );
    // One more level: RangeError from layer 2 (the exact cut).
    let above = run(form, n_cap + 1);
    assert_eq!(
        layer_of(&above),
        Some(DepthLayer::Ast),
        "{name} n={} depth {}: {above:?}",
        n_cap + 1,
        depth_at(n_cap + 1)
    );
    // Far beyond: RangeError from layer 0 (or layer 2 for loops the parser builds iteratively).
    let far = run(form, 10 * n_cap);
    assert!(layer_of(&far).is_some(), "{name} n={}: {far:?}", 10 * n_cap);
    if ea > 0 {
        assert_eq!(layer_of(&far), Some(DepthLayer::PreScan), "{name}");
    }
}

/// Regex nesting has no AST depth: the pre-scan cap is the limit.
fn check_regex_form(name: &str) {
    let form = form(name);
    let (ea, eb) = linear(|n| estimate(form, n));
    assert_eq!(ea, REGEX_WEIGHT, "{name}: estimate per level");
    let n_cap = usize::try_from((PRESCAN_CAP - eb) / ea).unwrap();
    assert!(run(form, n_cap).is_ok(), "{name} n={n_cap}");
    let above = run(form, n_cap + 1);
    assert_eq!(
        layer_of(&above),
        Some(DepthLayer::PreScan),
        "{name}: {above:?}"
    );
    // Far beyond: more levels than the worker stack holds in either profile without layer 0.
    let far = run(form, 1_000_000);
    assert_eq!(
        layer_of(&far),
        Some(DepthLayer::PreScan),
        "{name} n=1000000: {far:?}"
    );
}

macro_rules! gate {
    ($($test:ident => $form:literal,)*) => {
        $(
            #[test]
            fn $test() {
                check_ast_form($form);
            }
        )*
        /// Every form in `forms.rs` (except regex forms) has a test here.
        const GATED: &[&str] = &[$($form),*];
    };
}

gate! {
    // M0 exit item 7
    m0_bracket_array => "bracket-array",
    m0_bracket_paren => "bracket-paren",
    m0_bracket_object => "bracket-object",
    m0_bracket_block => "bracket-block",
    m0_bracket_function => "bracket-function",
    m0_binary => "binary",
    m0_member => "member",
    m0_call => "call",
    m0_comma_nested => "comma-nested",
    m0_template => "template",
    m0_tagged_template => "tagged-template",
    m0_class_body => "class-body",
    m0_class_heritage => "class-heritage",
    // M1 exit item 8
    m1_conditional => "conditional",
    m1_conditional_consequent => "conditional-consequent",
    m1_assignment => "assignment",
    m1_unary => "unary",
    m1_unary_typeof => "unary-typeof",
    m1_arrow => "arrow",
    m1_exponent => "exponent",
    // other recursive productions
    x_optional_chain => "optional-chain",
    x_new => "new",
    x_spread => "spread",
    x_if => "if",
    x_else_if => "else-if",
    x_label => "label",
    x_do_while => "do-while",
    x_for => "for",
    x_with => "with",
    x_try => "try",
    x_switch => "switch",
    x_arrow_block => "arrow-block",
    x_async_arrow => "async-arrow",
    x_array_pattern => "array-pattern",
    x_object_pattern => "object-pattern",
    x_param_pattern => "param-pattern",
    x_await => "await",
    x_yield => "yield",
    x_module_block => "module-block",
}

macro_rules! regex_gate {
    ($($test:ident => $form:literal,)*) => {
        $(
            #[test]
            fn $test() {
                check_regex_form($form);
            }
        )*
        /// Every regex form in `forms.rs` has a test here.
        const REGEX_GATED: &[&str] = &[$($form),*];
    };
}

regex_gate! {
    x_regex_group => "regex-group",
    x_regex_class_v => "regex-class-v",
    // With `v`, a class stays open after a nested class closes.
    x_regex_class_v_intersection => "regex-class-v-intersection",
    x_regex_class_v_subtraction => "regex-class-v-subtraction",
    x_regex_class_v_union => "regex-class-v-union",
    x_regex_group_u => "regex-group-u",
    x_regex_named_group => "regex-named-group",
    x_regex_ignore_group => "regex-ignore-group",
    x_regex_lookahead => "regex-lookahead",
    x_regex_lookbehind => "regex-lookbehind",
}

#[test]
fn every_form_is_gated() {
    for form in FORMS {
        let gated = if form.name.starts_with("regex-") {
            REGEX_GATED.contains(&form.name)
        } else {
            GATED.contains(&form.name) || STACK_GATED.contains(&form.name)
        };
        assert!(gated, "{} has no gate test", form.name);
    }
    let m0 = FORMS.iter().filter(|f| f.gate == Gate::M0).count();
    let m1 = FORMS.iter().filter(|f| f.gate == Gate::M1).count();
    assert_eq!((m0, m1), (13, 7));
}

#[test]
fn a_flat_comma_list_has_no_depth() {
    // oxc keeps `a, b, c` flat (one SequenceExpression), so a long list is not a depth hazard.
    let src = format!("x = ({}0)", "0,".repeat(200_000));
    assert!(check_early_errors(&src, SourceOpts::script(Strictness::Sloppy)).is_ok());
}

#[test]
fn a_million_levels_never_abort() {
    for src in [
        "[".repeat(1_000_000),
        format!("1{}", "+1".repeat(1_000_000)),
        format!("a{}", ".b".repeat(1_000_000)),
        "(".repeat(1_000_000),
        format!("/{}/", "(".repeat(1_000_000)),
        "`${".repeat(300_000),
    ] {
        let result = check_early_errors(&src, SourceOpts::script(Strictness::Sloppy));
        assert!(result.is_err(), "{}...", &src[..20]);
    }
}

#[test]
fn range_error_reports_a_position() {
    let src = format!("let a = 1;\n{}", "[".repeat(100_000));
    let err = check_early_errors(&src, SourceOpts::script(Strictness::Sloppy)).unwrap_err();
    let range = err.as_range().expect("a range error");
    assert_eq!(range.kind, RangeErrorKind::TooDeep(DepthLayer::PreScan));
    let position = range.position.expect("a position");
    assert_eq!(position.line, 2);
    assert!(err.to_string().starts_with("2:"), "{err}");
    assert!(err.to_string().contains("RangeError"), "{err}");
}

/// Deterministic pseudo-random mixes of nesting forms near the caps: Ok or RangeError, never an
/// abort. This checks that the stack model holds for mixtures, not only for pure forms.
#[test]
fn random_mixes_near_the_caps_never_abort() {
    // Expression wrappers: (prefix, suffix). Each one yields a primary-like expression, so any
    // wrapper can hold any other (operators and arrows are parenthesized).
    const WRAP: &[(&str, &str)] = &[
        ("[", "]"),
        ("(", ")"),
        ("({a:", "})"),
        ("`${", "}`"),
        ("(!", ")"),
        ("(a=", ")"),
        ("(x=>", ")"),
        ("(a?", ":0)"),
        ("(a**", ")"),
        ("new ", ""),
        ("(0,", ")"),
        ("function(){return ", "}"),
        ("class extends ", "{}"),
        ("[...", "]"),
        ("(()=>{if(a)return ", "})"),
        ("(typeof ", ")"),
        ("(async x=>", ")"),
        ("f(", ")"),
        // The call, spread and list forms of FORMS (the stack forms): the nested item after 6
        // list items, and before them. The switch pairs are in a function with `return`, so each
        // pair is one expression.
        ("f(0,0,0,0,0,0,", ")"),
        ("f(", ",0,0,0,0,0,0)"),
        ("a.m(0,0,0,0,0,0,", ")"),
        ("a.m(", ",0,0,0,0,0,0)"),
        // Parenthesized: `new f?.()` is a SyntaxError, and `new ` is a wrapper.
        ("(f?.(0,0,0,0,0,0,", "))"),
        ("(f?.(", ",0,0,0,0,0,0))"),
        ("new C(0,0,0,0,0,0,", ")"),
        ("new C(", ",0,0,0,0,0,0)"),
        ("async(0,0,0,0,0,0,", ")"),
        ("async(", ",0,0,0,0,0,0)"),
        ("f(...a,...a,...a,...a,...a,...a,...", ")"),
        ("f(...", ",...a,...a,...a,...a,...a,...a)"),
        ("f`${0}${0}${0}${0}${0}${0}${", "}`"),
        ("f`${", "}${0}${0}${0}${0}${0}${0}`"),
        ("(0,0,0,0,0,0,", ")"),
        ("(", ",0,0,0,0,0,0)"),
        ("`${0}${0}${0}${0}${0}${0}${", "}`"),
        ("`${", "}${0}${0}${0}${0}${0}${0}`"),
        ("[0,0,0,0,0,0,", "]"),
        ("[", ",0,0,0,0,0,0]"),
        ("function(){switch(a){case 0:0;0;0;0;0;0;return ", "}}"),
        ("function(){switch(a){case 0:return ", ";0;0;0;0;0;0;}}"),
        ("function(){0;0;0;0;0;0;return ", "}"),
        ("function(){return ", ";0;0;0;0;0;0;}"),
        ("(()=>f(0,0,0,0,0,0,", "))"),
        ("(()=>f(", ",0,0,0,0,0,0))"),
    ];
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut outcomes = [0usize; 2];
    for round in 0..12 {
        let target = match round % 3 {
            0 => AST_CAP as usize / 2,
            1 => PRESCAN_CAP as usize - 50,
            _ => PRESCAN_CAP as usize + 50,
        };
        let mut prefix = String::from("x=");
        let mut suffix = String::new();
        for _ in 0..target {
            let (p, s) = WRAP[usize::try_from(next() % WRAP.len() as u64).unwrap()];
            prefix.push_str(p);
            suffix.insert_str(0, s);
        }
        let src = format!("{prefix}0{suffix}");
        let result = check_early_errors(&src, SourceOpts::script(Strictness::Sloppy));
        match &result {
            Ok(()) => outcomes[0] += 1,
            Err(CompileError::Range(_)) => outcomes[1] += 1,
            Err(CompileError::Syntax(e)) => {
                let at = e.first().span.map_or(0, |s| s.start as usize);
                let from = at.saturating_sub(60);
                let to = (at + 60).min(src.len());
                panic!(
                    "round {round}: unexpected syntax error {e}\n...{}...",
                    &src[from..to]
                );
            }
        }
    }
    // Some rounds compile, some hit a cap.
    assert!(outcomes[0] > 0 && outcomes[1] > 0, "{outcomes:?}");
}

// The pre-scan counts 2 units per new-expression level (`new` + `(`) while the AST has 1, so these forms reach PRESCAN_CAP (about 12,500 levels) before AST_CAP; route PRESCAN-NEW-COUNT.
fn check_prescan_new_form(name: &str) {
    let form = form(name);
    let (a, _) = linear(|n| ast_depth(form, n));
    assert_eq!(a, 1, "{name}: AST depth per level");
    // A fix of the count gives 1 here: then the form moves to check_ast_form.
    let (ea, eb) = linear(|n| estimate(form, n));
    assert_eq!(ea, 2, "{name}: estimate per level");
    let n_cap = usize::try_from((PRESCAN_CAP - eb) / ea).unwrap();
    let at = run(form, n_cap);
    assert!(at.is_ok(), "{name} n={n_cap}: {at:?}");
    // The scope pass (layer 3) also handles the deepest accepted tree on the worker stack.
    let scopes = crate::analyze(&(form.build)(n_cap), opts(form));
    assert!(
        scopes.is_ok(),
        "{name} n={n_cap}: scope pass {:?}",
        scopes.err()
    );
    let above = run(form, n_cap + 1);
    assert_eq!(
        layer_of(&above),
        Some(DepthLayer::PreScan),
        "{name} n={}: {above:?}",
        n_cap + 1
    );
    let far = run(form, 10 * n_cap);
    assert_eq!(
        layer_of(&far),
        Some(DepthLayer::PreScan),
        "{name} n={}: {far:?}",
        10 * n_cap
    );
}

macro_rules! stack_gate {
    ($($test:ident => $check:ident($form:literal),)*) => {
        $(
            #[test]
            fn $test() {
                $check($form);
            }
        )*
        /// The stack forms of `forms.rs` (plan v17 §5.2 step 1) have a test here.
        const STACK_GATED: &[&str] = &[$($form),*];
    };
}

stack_gate! {
    // stack forms: the nested item after 6 list items, and first
    s_call_args => check_ast_form("call-args"),
    s_call_args_first => check_ast_form("call-args-first"),
    s_method_args => check_ast_form("method-args"),
    s_method_args_first => check_ast_form("method-args-first"),
    s_optional_call_args => check_ast_form("optional-call-args"),
    s_optional_call_args_first => check_ast_form("optional-call-args-first"),
    s_new_args => check_prescan_new_form("new-args"),
    s_new_args_first => check_prescan_new_form("new-args-first"),
    s_async_call_args => check_ast_form("async-call-args"),
    s_async_call_args_first => check_ast_form("async-call-args-first"),
    s_spread_args => check_ast_form("spread-args"),
    s_spread_args_first => check_ast_form("spread-args-first"),
    s_tagged_subst => check_ast_form("tagged-subst"),
    s_tagged_subst_first => check_ast_form("tagged-subst-first"),
    s_sequence_list => check_ast_form("sequence-list"),
    s_sequence_list_first => check_ast_form("sequence-list-first"),
    s_template_list => check_ast_form("template-list"),
    s_template_list_first => check_ast_form("template-list-first"),
    s_array_list => check_ast_form("array-list"),
    s_array_list_first => check_ast_form("array-list-first"),
    s_switch_clause_list => check_ast_form("switch-clause-list"),
    s_switch_clause_list_first => check_ast_form("switch-clause-list-first"),
    s_function_body_list => check_ast_form("function-body-list"),
    s_function_body_list_first => check_ast_form("function-body-list-first"),
    s_arrow_body_call => check_ast_form("arrow-body-call"),
    s_arrow_body_call_first => check_ast_form("arrow-body-call-first"),
}

/// T-B (plan v17 §5.2 step 4): every form with a growing estimate, at the largest n with
/// estimate <= PRESCAN_CAP, compiles or is a TooDeep(Ast) RangeError, never an abort. An abort
/// kills the process, so each form runs in a child process (this test with the env var set).
/// The child asserts the fit (estimate <= PRESCAN_CAP at n, > PRESCAN_CAP at n + 1).
#[test]
fn check_prescan_cap_form() {
    const CHILD: &str = "EP9_PRESCAN_CAP_FORM";
    let n_cap = |form: &Form| -> Option<usize> {
        let (ea, eb) = linear(|n| estimate(form, n));
        (ea > 0).then(|| usize::try_from((PRESCAN_CAP - eb) / ea).unwrap())
    };
    if let Some(name) = std::env::var_os(CHILD) {
        let form = form(name.to_str().unwrap());
        let n = n_cap(form).expect("a growing estimate");
        let e = estimate(form, n);
        // The fit: n is the largest n with estimate <= PRESCAN_CAP (read at n and n + 1, not
        // from the line), so the run below is a parse at the cap, not a layer-0 reject.
        assert!(
            e <= PRESCAN_CAP,
            "{} n {n}: estimate {e} > PRESCAN_CAP",
            form.name
        );
        let e_next = estimate(form, n + 1);
        assert!(
            e_next > PRESCAN_CAP,
            "{} n {}: estimate {e_next} <= PRESCAN_CAP (n is not the largest fit)",
            form.name,
            n + 1
        );
        let result = match run(form, n) {
            Ok(()) => "Ok".to_string(),
            Err(CompileError::Range(e)) => match e.kind {
                RangeErrorKind::TooDeep(layer) => format!("TooDeep({layer:?})"),
                RangeErrorKind::StackUnavailable => panic!("{}: stack unavailable: {e}", form.name),
                // T-SAME: printed, and the parent fails on it (Claim D of layer 1).
                RangeErrorKind::StackLimit => "StackLimit".to_string(),
            },
            Err(e) => panic!("{} n={n}: {e}", form.name),
        };
        println!(
            "PRESCAN_CAP_FORM {} n {n} estimate {e} -> {result}",
            form.name
        );
        return;
    }
    let mut flat = Vec::new();
    let mut runs = Vec::new();
    for form in FORMS {
        if n_cap(form).is_some() {
            runs.push(form);
        } else {
            flat.push(form.name);
        }
    }
    // A few children at a time: each one parses at the pre-scan cap on its own worker stack.
    let width = std::thread::available_parallelism().map_or(1, |n| n.get().min(4));
    for chunk in runs.chunks(width) {
        let children: Vec<_> = chunk
            .iter()
            .map(|form| {
                let child = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "depth_tests::check_prescan_cap_form",
                        "--nocapture",
                        "--test-threads=1",
                    ])
                    .env(CHILD, form.name)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap();
                (form.name, child)
            })
            .collect();
        for (name, child) in children {
            let out = child.wait_with_output().unwrap();
            let stdout = String::from_utf8_lossy(&out.stdout);
            // The harness prints "test ... " on the same line before the child's line.
            let mark = format!("PRESCAN_CAP_FORM {name} ");
            let line = stdout
                .lines()
                .find_map(|l| l.find(&mark).map(|at| &l[at..]));
            // At the cap layer 0 passes, so only a parse result is right: Ok, or TooDeep from
            // layer 2 (the AST cap). TooDeep(PreScan) means a wrong fit.
            assert!(
                out.status.success()
                    && line.is_some_and(|l| l.ends_with("-> Ok") || l.ends_with("-> TooDeep(Ast)")),
                "{name}: {:?}\nstdout:\n{stdout}\nstderr:\n{}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            );
            println!("{}", line.unwrap());
        }
    }
    println!("PRESCAN_CAP_FORM no pre-scan cap (estimate does not grow): {flat:?}");
}

// ---------------------------------------------------------------------------------------------
// Layer 1 (l1.rs, H01): the stack check of the parse, the budget checks and the backstops. The
// small-stack tests run each case in a child process (an abort kills only the child), on a
// worker of `SMALL_WORKER` bytes, so the room above the limit is 3 MiB.

/// The child env var of the layer-1 tests: `<test name>:<case index>`.
const L1_CHILD: &str = "EP9_L1_CHILD";

/// The worker of the small-stack tests.
const SMALL_WORKER: usize = 4 << 20;

/// The small worker as a front-end stack.
const SMALL: crate::l1::Stack = crate::l1::Stack::Worker(SMALL_WORKER);

/// In a child of the test `test`: the case index that it runs.
fn l1_child(test: &str) -> Option<usize> {
    let value = std::env::var(L1_CHILD).ok()?;
    let (name, case) = value.split_once(':')?;
    (name == test).then(|| case.parse().expect("a case index"))
}

/// The child line of a case: `L1 <test> <case> -> <word>`.
fn l1_line(test: &str, case: usize, word: &str) {
    println!("L1 {test} {case} -> {word}");
}

/// Run the cases `0..count` of `test` (a test of this module), each in its own child process,
/// a few at a time. Gives the word that each child printed. Panics on a signal, a non-zero
/// status or no line, with the case name and the stderr tail.
fn run_children(test: &str, names: &[String]) -> Vec<String> {
    let width = std::thread::available_parallelism().map_or(1, |n| n.get().min(4));
    let mut words = Vec::new();
    let indices: Vec<usize> = (0..names.len()).collect();
    for chunk in indices.chunks(width) {
        let children: Vec<_> = chunk
            .iter()
            .map(|&i| {
                let child = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        &format!("depth_tests::{test}"),
                        "--nocapture",
                        "--test-threads=1",
                    ])
                    .env(L1_CHILD, format!("{test}:{i}"))
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .unwrap();
                (i, child)
            })
            .collect();
        for (i, child) in children {
            let out = child.wait_with_output().unwrap();
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            // The harness prints "test ... " on the same line before the child's line.
            let mark = format!("L1 {test} {i} -> ");
            let word = stdout
                .lines()
                .find_map(|l| l.find(&mark).map(|at| l[at + mark.len()..].to_owned()));
            let tail: Vec<&str> = stderr.lines().rev().take(20).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            assert!(
                out.status.success() && word.is_some(),
                "{test} case {i} ({}): {:?}\nstderr tail:\n{}",
                names[i],
                out.status,
                tail.join("\n")
            );
            words.push(word.unwrap());
        }
    }
    words
}

/// The result kind as a word: `Ok`, `Syntax`, `Range(<kind>)`.
fn word(result: &Result<(), CompileError>) -> String {
    match result {
        Ok(()) => "Ok".to_owned(),
        Err(CompileError::Syntax(_)) => "Syntax".to_owned(),
        Err(CompileError::Range(e)) => format!("Range({:?})", e.kind),
    }
}

/// As [`word`], for the BytecodeGen entries.
fn codegen_word<T>(result: &Result<T, crate::CodegenError>) -> String {
    match result {
        Ok(_) => "Ok".to_owned(),
        Err(crate::CodegenError::Compile(e)) => word(&Err(e.clone())),
        Err(e) => format!("Other({e})"),
    }
}

/// `check_early_errors` with the job on `stack`.
fn check_on(src: &str, opts: SourceOpts, stack: crate::l1::Stack) -> Result<(), CompileError> {
    crate::with_front_end_at(src, opts, stack, |_| Ok(()))
}

const STACK_LIMIT: &str = "Range(StackLimit)";

/// The largest n of `form` within both caps (the AST cap, and the pre-scan cap where the
/// estimate grows).
fn cap_n(form: &Form) -> usize {
    let (a, b) = linear(|n| ast_depth(form, n));
    let (ea, eb) = linear(|n| estimate(form, n));
    let ast = (AST_CAP - b).checked_div(a);
    let pre = (PRESCAN_CAP - eb).checked_div(ea);
    let n = ast.into_iter().chain(pre).min();
    usize::try_from(n.unwrap_or_else(|| panic!("{}: no cap", form.name))).unwrap()
}

/// `x = {}`, `{}` and a regex literal with a backquote before the text: the carrier of hunt
/// rule 18 (the pre-scan reads a template from the backquote and skips the text).
fn carried(text: &str) -> String {
    format!("x={{}}\n{{}}\n/`/;{text};/`/")
}

/// `prefix + open x n + mid + close x n + suffix`.
fn nest(prefix: &str, open: &str, mid: &str, close: &str, suffix: &str, n: usize) -> String {
    format!("{prefix}{}{mid}{}{suffix}", open.repeat(n), close.repeat(n))
}

/// The pre-scan holes of the design check (PCHK K1-K6, `check_exp.py`), with the TS type sites
/// that a Script parse reaches (K2, K3): (name, text at `n`). Each one nests `n` levels that the
/// pre-scan does not count.
fn pchk_rows(n: usize) -> Vec<(&'static str, String)> {
    const IF: &str = "if(a)";
    vec![
        (
            "K1 H1-deco-heritage-carried",
            carried(&nest("x=", "@a @a class extends ", "B", "{}", "", n)),
        ),
        (
            "K1 H1b-deco-paren-carried",
            carried(&nest("x=", "@(a) @(a) class extends ", "B", "{}", "", n)),
        ),
        (
            "K1 H1c-deco-stmt-carried",
            carried(&nest("", "@a @a class A extends ", "B", "{}", "", n)),
        ),
        (
            "K2 H2-typeargs-bang",
            nest("x as ", "A<B!,", "B", ">,B", "", n),
        ),
        (
            "K2 H2-typeargs-bang-carried",
            carried(&nest("x as ", "A<B!,", "B", ">,B", "", n)),
        ),
        (
            "K3 H4-gt-regex-bang",
            format!("x as A<B>/{}a/1", "!".repeat(n)),
        ),
        (
            "K3 H4b-gt-regex-array",
            nest("x as A<B>/", "[", "0", "]", "/1", n),
        ),
        (
            "K3 H4c-gt-regex-typeof",
            format!("x satisfies A<B>/{}a/1", "typeof ".repeat(n)),
        ),
        (
            "K4 H5-phantom-brace-paren",
            format!("(x=a\\u{{);{};", IF.repeat(n)),
        ),
        (
            "K4 H5b-phantom-brace-bracket",
            format!("[a\\u{{];{};", IF.repeat(n)),
        ),
        (
            "K5 H7-var-comma-pattern",
            format!("var a,{{b}}\n/\"/;{};", IF.repeat(n)),
        ),
        (
            "K5 H7b-let-comma-pattern",
            format!("let a,{{b}}\n/\"/;{};", IF.repeat(n)),
        ),
        (
            "K5 H7-var-comma-pattern-carried",
            carried(&format!("var a,{{b}}\n/\"/;{};", IF.repeat(n))),
        ),
        (
            "K6 H6-zwnj-assign-carried",
            carried(&format!("x={}0", "a\u{200C}b=".repeat(n))),
        ),
    ]
}

/// The 2 forms of the design's C-1: a `/*@__PURE__*/` member chain with no call, and one after
/// a call (oxc's `set_pure_on_call_or_new_expr`, a loop since patch 0002).
fn pure_chain(call: bool, n: usize) -> String {
    let head = if call { "a()" } else { "a" };
    format!("/*@__PURE__*/{head}{}", ".a".repeat(n))
}

/// A case of T-SMALL: its name, and the maker of its text and opts (run in the child only).
type SmallCase = (String, Box<dyn Fn() -> (String, SourceOpts)>);

/// The case list of T-SMALL.
fn small_cases() -> Vec<SmallCase> {
    let mut cases: Vec<SmallCase> = FORMS
        .iter()
        .map(|f| {
            let case: Box<dyn Fn() -> (String, SourceOpts)> =
                Box::new(move || ((f.build)(cap_n(f)), opts(f)));
            (format!("form {}", f.name), case)
        })
        .collect();
    let script = SourceOpts::script(Strictness::Sloppy);
    // AST depth n + 3 (Program, the statement, n members, the name) and n + 4 with the call:
    // at the AST cap. (In release the layer-2 walk of one more level fits the small worker and
    // gives TooDeep(Ast).)
    let n = AST_CAP as usize - 4;
    for call in [false, true] {
        cases.push((
            format!("C-1 pure chain (call {call})"),
            Box::new(move || (pure_chain(call, n), script)),
        ));
    }
    for (i, (name, _)) in pchk_rows(1).into_iter().enumerate() {
        cases.push((
            format!("PCHK {name}"),
            Box::new(move || (pchk_rows(100_000).swap_remove(i).1, script)),
        ));
    }
    cases
}

/// T-SMALL: on a 4 MiB worker, every form at its cap, the 2 C-1 forms at the AST cap and the
/// PCHK rows (K1-K6, the TS type sites from a Script parse) give RangeError(StackLimit), never
/// a signal. Each path of the front end has a stop: the parse sites, the regex budget, the
/// layer-2 backstop or the run-time budget check.
#[test]
fn every_form_stops_on_a_small_stack() {
    const TEST: &str = "every_form_stops_on_a_small_stack";
    let cases = small_cases();
    if let Some(i) = l1_child(TEST) {
        let (src, opts) = (cases[i].1)();
        l1_line(TEST, i, &word(&check_on(&src, opts, SMALL)));
        return;
    }
    let names: Vec<String> = cases.iter().map(|c| c.0.clone()).collect();
    let words = run_children(TEST, &names);
    let wrong: Vec<String> = names
        .iter()
        .zip(&words)
        .filter(|(_, w)| *w != STACK_LIMIT)
        .map(|(n, w)| format!("{n}: {w}"))
        .collect();
    assert!(wrong.is_empty(), "not a StackLimit:\n{}", wrong.join("\n"));
}

/// T-STICKY: the stop inside a part that the parser reads speculatively and then rewinds (a
/// rewind drops a fatal error): an arrow head, a default value, an `async (` head, a
/// parenthesized expression. StackLimit, never SyntaxError or Ok.
#[test]
fn a_stack_stop_survives_a_rewind() {
    const TEST: &str = "a_stack_stop_survives_a_rewind";
    // Within the pre-scan cap, far above the 3 MiB of the small worker.
    let n = 20_000;
    let cases: Vec<(&str, String)> = vec![
        ("arrow head", nest("(", "[", "a", "]", ")=>0", n)),
        ("arrow default value", nest("(a=", "[", "0", "]", ")=>a", n)),
        (
            "async arrow head",
            nest("async (", "[", "a", "]", ")=>0", n),
        ),
        ("async call", nest("async (", "[", "0", "]", ")", n)),
        ("parenthesized", nest("(", "[", "0", "]", ")", n)),
        (
            "object arrow head",
            nest("({a:", "{a:", "b", "}", "})=>0", n),
        ),
    ];
    if let Some(i) = l1_child(TEST) {
        let script = SourceOpts::script(Strictness::Sloppy);
        l1_line(TEST, i, &word(&check_on(&cases[i].1, script, SMALL)));
        return;
    }
    let names: Vec<String> = cases.iter().map(|c| c.0.to_owned()).collect();
    for (name, w) in names.iter().zip(run_children(TEST, &names)) {
        assert_eq!(w, STACK_LIMIT, "{name}");
    }
}

/// T-EXIT: a lexer error before the deep part. oxc then keeps `fatal_error` and drops the
/// message of the stop (the end-of-text branch of its parse exit), so only the flag shows the
/// stop: StackLimit, not SyntaxError.
#[test]
fn the_stop_flag_wins_over_a_lexer_error() {
    const TEST: &str = "the_stop_flag_wins_over_a_lexer_error";
    let cases = |n: usize| -> Vec<(&'static str, String)> {
        vec![
            (
                "string escape",
                format!(
                    "x = \"\\u{{110000}}\";\n{}0{}",
                    "[".repeat(n),
                    "]".repeat(n)
                ),
            ),
            (
                "identifier escape (PCHK K4)",
                format!("(x=a\\u{{);{};", "if(a)".repeat(n * 5)),
            ),
        ]
    };
    let script = SourceOpts::script(Strictness::Sloppy);
    if let Some(i) = l1_child(TEST) {
        let src = cases(20_000).swap_remove(i).1;
        l1_line(TEST, i, &word(&check_on(&src, script, SMALL)));
        return;
    }
    // The texts hold a lexer error: a shallow one is a SyntaxError.
    for (name, src) in cases(3) {
        assert_eq!(word(&check_early_errors(&src, script)), "Syntax", "{name}");
    }
    let names: Vec<String> = cases(1).iter().map(|c| c.0.to_owned()).collect();
    for (name, w) in names.iter().zip(run_children(TEST, &names)) {
        assert_eq!(w, STACK_LIMIT, "{name}");
    }
}

/// T-PURE (the design's C-1): both `/*@__PURE__*/` member chain forms with 4,000,000 members
/// on the real worker. Patch 0002 makes oxc's walk over the chain a loop, so they need no stack
/// and give TooDeep(Ast) on every host, never an abort.
#[test]
fn pure_member_chains_never_abort() {
    const TEST: &str = "pure_member_chains_never_abort";
    let script = SourceOpts::script(Strictness::Sloppy);
    if let Some(i) = l1_child(TEST) {
        let src = pure_chain(i == 1, 4_000_000);
        l1_line(TEST, i, &word(&check_early_errors(&src, script)));
        return;
    }
    let names = ["no call".to_owned(), "after a call".to_owned()];
    for (name, w) in names.iter().zip(run_children(TEST, &names)) {
        assert_eq!(w, "Range(TooDeep(Ast))", "{name}");
    }
}

/// T-PASS: the walks after the parse stop on a thread smaller than the budget.
/// (1) The backstops of the ep9 walks, with a limit 32 KiB below the walk on the real worker:
/// the layer-2 walk (`check_ast_depth`) stops by its backstop; layer 2, the scope pass and
/// BytecodeGen give StackLimit (review R2 F1: scope/mod.rs maps a backstop stop of the scope
/// pass to StackLimit, and a stop at its cap to TooDeep(Pass)). The `rewrite_completion`
/// backstop has its own test in eval.rs.
/// (2) In a child, on the 4 MiB worker, an input inside the caps whose parse is a loop (a member
/// chain): the eval path (`Allowed::collect`, `Shim::patch`, `post_checks`, oxc_semantic),
/// `compile_script_with_completion` and `compile` give StackLimit (the run-time budget check, or
/// the layer-2 backstop), never an abort. On the real worker the same input compiles.
#[test]
fn passes_stop_on_a_small_stack() {
    const TEST: &str = "passes_stop_on_a_small_stack";
    let chain = format!("a{}", ".a".repeat(5_000));
    let entries = ["compile_eval", "compile_script_with_completion", "compile"];
    if let Some(i) = l1_child(TEST) {
        let w = match i {
            0 => codegen_word(&crate::eval::compile_eval_at(
                &chain,
                &ep9_bytecode::EvalContext::default(),
                crate::CompileOptions::default(),
                SMALL,
            )),
            1 => codegen_word(&crate::eval::with_script_completion_at(
                &chain,
                Strictness::Sloppy,
                SMALL,
                |fe| {
                    let analysis = crate::scope::analyze(fe)?;
                    crate::bytecodegen::generate(fe, &analysis, crate::CompileOptions::default())
                },
            )),
            _ => {
                let opts = SourceOpts::script(Strictness::Sloppy);
                let r = crate::with_front_end_at(&chain, opts, SMALL, |fe| {
                    let analysis = crate::scope::analyze(fe)?;
                    Ok(crate::bytecodegen::generate(
                        fe,
                        &analysis,
                        crate::CompileOptions::default(),
                    ))
                });
                match r {
                    Ok(unit) => codegen_word(&unit),
                    Err(e) => word(&Err(e)),
                }
            }
        };
        l1_line(TEST, i, &w);
        return;
    }
    // (2) The same input compiles on the real worker (Claim D), and stops on the small one.
    let full = crate::compile_eval(
        &chain,
        &ep9_bytecode::EvalContext::default(),
        crate::CompileOptions::default(),
    );
    assert_eq!(codegen_word(&full), "Ok");
    let names: Vec<String> = entries.iter().map(|&e| e.to_owned()).collect();
    for (name, w) in names.iter().zip(run_children(TEST, &names)) {
        assert_eq!(w, STACK_LIMIT, "{name}");
    }
    // (1) The backstops, one walk at a time.
    let src = chain.clone();
    let kinds = crate::worker::run(crate::stack::WORKER_STACK_BYTES, move || {
        let opts = SourceOpts::script(Strictness::Sloppy);
        let allocator = Allocator::default();
        let program = parse(&allocator, &src, opts).expect("parses on the real worker");
        // The layer-2 walk itself (review V9): its backstop stops it (`stack_limit`). Through
        // `check_depth` the run-time budget check gives the same word without the backstop.
        let backstop = {
            let _tight = crate::l1::LimitScope::room(32 << 10);
            match crate::depth::check_ast_depth(&program, AST_CAP) {
                Err(deep) if deep.stack_limit => "stack_limit".to_owned(),
                other => format!("{:?}", other.map_err(|d| d.stack_limit)),
            }
        };
        let layer2 = {
            let _tight = crate::l1::LimitScope::room(32 << 10);
            word(&crate::check_depth(&program, &src).map_err(CompileError::Range))
        };
        let semantic = crate::semantic(&src, &program).expect("semantic");
        let fe = crate::FrontEnd {
            source: &src,
            opts,
            program: &program,
            semantic: &semantic,
        };
        let scope = {
            let _tight = crate::l1::LimitScope::room(32 << 10);
            word(&crate::scope::analyze(&fe).map(|_| ()))
        };
        let analysis = crate::scope::analyze(&fe).expect("scope pass");
        let codegen = {
            let _tight = crate::l1::LimitScope::room(32 << 10);
            codegen_word(&crate::bytecodegen::generate(
                &fe,
                &analysis,
                crate::CompileOptions::default(),
            ))
        };
        (layer2, scope, codegen, backstop)
    })
    .unwrap();
    assert_eq!(kinds.3, "stack_limit", "the layer-2 walk backstop");
    assert_eq!(kinds.0, STACK_LIMIT, "layer 2");
    assert_eq!(kinds.1, STACK_LIMIT, "scope pass");
    assert_eq!(kinds.2, STACK_LIMIT, "BytecodeGen");
}

// The visitor-graph proof of the layer-2 walk (review R2 A2-1). `DepthPass` (depth.rs) stops
// only at its gated visit functions. The walk is bounded only if every cycle of the oxc visitor
// call graph passes through a gated node. This test reads the visitor source of the pinned oxc
// checkout, so a new oxc pin with a new cycle fails here, before an input can abort the process.
//
// Why the graph is complete for `DepthPass`:
// - The walk of `DepthPass` calls only the `Visit` trait methods. The trait default of each
//   `visit_X` calls `walk_X` and nothing else (checked below: no alias visit function).
// - A `walk_X` body calls `visitor.visit_Y` (an edge X -> Y), `visitor.enter_node` /
//   `leave_node` / `enter_scope` / `leave_scope` / `alloc` (no recursion in `DepthPass`: its
//   `enter_node` and `leave_node` call no visit function, and the scope hooks are the empty
//   defaults), and no `walk_Y` directly (checked below). Any other `visitor.` call fails the test.
// - `Vec` fields have their own walk function (`walk_statements` loops on `visit_statement`): a
//   node of the graph, a loop and not a recursion. `Option` is an `if let` around one call and
//   `Box` is a deref: one edge each, no extra node.
// - The overrides of `DepthPass` are read from depth.rs: each `fn visit_X` there must be the gate
//   form (`if self.may_descend() { walk::walk_X(self, it) }`), else the test fails.
// - A gate stops a cycle only if the depth grows on it: every cycle of the full graph must have
//   a node whose walk calls `enter_node` (checked below).

/// The oxc visitor source: `oxc_ast_visit` 0.158.0 of the crates.io package is byte-equal to
/// the pinned checkout (the oxc_parser path dependency needs the checkout, so it is present).
fn oxc_visit_source() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../third_party/oxc/crates/oxc_ast_visit/src/generated/visit.rs"
    );
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The identifiers that follow `prefix` in `body` (for example `visitor.visit_`).
fn names_after<'s>(body: &'s str, prefix: &str) -> Vec<&'s str> {
    body.match_indices(prefix)
        .map(|(at, _)| {
            let rest = &body[at + prefix.len()..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .collect()
}

/// The functions of `text` whose head line starts with `head` (at 4 spaces), each with the
/// name after `head` and the body up to the first line `    }`.
fn fn_bodies<'s>(text: &'s str, head: &str) -> Vec<(&'s str, String)> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix(head) else {
            continue;
        };
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let mut body = String::new();
        for l in lines.by_ref() {
            if l == "    }" {
                break;
            }
            body.push_str(l);
            body.push('\n');
        }
        out.push((&rest[..end], body));
    }
    out
}

/// The visitor graph: node X -> the nodes Y whose `visit_Y` the walk of X calls; and the
/// nodes whose walk calls `enter_node` (the depth grows there).
struct VisitGraph {
    edges: std::collections::BTreeMap<String, Vec<String>>,
    counted: std::collections::BTreeSet<String>,
}

fn visit_graph(text: &str) -> VisitGraph {
    let (head, walks) = text
        .split_once("\npub mod walk {\n")
        .expect("the walk module");
    // The trait defaults: `visit_X` calls `walk_X` only.
    let visits = fn_bodies(head, "    fn visit_");
    for (name, body) in &visits {
        let calls = names_after(body, "walk_");
        assert_eq!(
            calls,
            [*name],
            "visit_{name} is not the plain walk_{name} call"
        );
        assert!(
            !body.contains("visit_"),
            "visit_{name} calls a visit function"
        );
    }
    let walk_lines = text.lines().filter(|l| l.contains("pub fn walk_")).count();
    let bodies = fn_bodies(walks, "    pub fn walk_");
    assert_eq!(
        bodies.len(),
        walk_lines,
        "a walk function that the scan missed"
    );
    assert_eq!(bodies.len(), visits.len(), "a walk with no visit function");
    assert!(bodies.len() > 200, "{} walk functions", bodies.len());
    let mut graph = VisitGraph {
        edges: std::collections::BTreeMap::new(),
        counted: std::collections::BTreeSet::new(),
    };
    for (name, body) in &bodies {
        assert!(
            names_after(body, "walk_").is_empty(),
            "walk_{name} calls a walk directly"
        );
        assert_eq!(
            body.matches("visit_").count(),
            body.matches("visitor.visit_").count(),
            "walk_{name} calls a visit function not through the visitor"
        );
        let mut to = Vec::new();
        for call in names_after(body, "visitor.") {
            match call {
                "enter_node" => {
                    graph.counted.insert((*name).to_owned());
                }
                "leave_node" | "enter_scope" | "leave_scope" | "alloc" => {}
                _ => {
                    let y = call
                        .strip_prefix("visit_")
                        .unwrap_or_else(|| panic!("walk_{name}: visitor.{call}"));
                    to.push(y.to_owned());
                }
            }
        }
        graph.edges.insert((*name).to_owned(), to);
    }
    for (x, to) in &graph.edges {
        for y in to {
            assert!(
                graph.edges.contains_key(y),
                "walk_{x} -> visit_{y}: no walk_{y}"
            );
        }
    }
    graph
}

/// The gated visit functions of `DepthPass`, read from depth.rs (not a second list).
fn depth_pass_gates() -> std::collections::BTreeSet<String> {
    let src = include_str!("depth.rs");
    let code = src.split_once("#[cfg(test)]").expect("the test module").0;
    let mut gates = std::collections::BTreeSet::new();
    for (name, body) in fn_bodies(code, "    fn visit_") {
        assert!(
            body.contains("if self.may_descend() {")
                && body.contains(&format!("walk::walk_{name}(self, it);")),
            "DepthPass::visit_{name} is not the gate form:\n{body}"
        );
        gates.insert(name.to_owned());
    }
    assert!(!gates.is_empty(), "no gate found in depth.rs");
    gates
}

/// The cycles of `edges` restricted to `keep`: one cycle per strongly connected component
/// (Tarjan), each as `a -> b -> a`.
fn open_cycles(
    edges: &std::collections::BTreeMap<String, Vec<String>>,
    keep: impl Fn(&str) -> bool,
) -> Vec<String> {
    let nodes: Vec<&String> = edges.keys().filter(|n| keep(n)).collect();
    let index_of: std::collections::BTreeMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let succ: Vec<Vec<usize>> = nodes
        .iter()
        .map(|n| {
            edges[*n]
                .iter()
                .filter_map(|y| index_of.get(y.as_str()).copied())
                .collect()
        })
        .collect();
    // Iterative Tarjan.
    let count = nodes.len();
    let (mut index, mut low, mut on) =
        (vec![usize::MAX; count], vec![0; count], vec![false; count]);
    let (mut stack, mut comps, mut next) = (Vec::new(), Vec::new(), 0);
    for root in 0..count {
        if index[root] != usize::MAX {
            continue;
        }
        let mut work = vec![(root, 0usize)];
        while let Some(&mut (v, ref mut at)) = work.last_mut() {
            if *at == 0 && index[v] == usize::MAX {
                index[v] = next;
                low[v] = next;
                next += 1;
                stack.push(v);
                on[v] = true;
            }
            if let Some(&w) = succ[v].get(*at) {
                *at += 1;
                if index[w] == usize::MAX {
                    work.push((w, 0));
                } else if on[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[v]);
            }
            if low[v] == index[v] {
                let mut comp = Vec::new();
                while let Some(w) = stack.pop() {
                    on[w] = false;
                    comp.push(w);
                    if w == v {
                        break;
                    }
                }
                if comp.len() > 1 || succ[v].contains(&v) {
                    comps.push(comp);
                }
            }
        }
    }
    // One cycle per component: a shortest path in the component from its first node back to it.
    let mut cycles = Vec::new();
    for comp in comps {
        let inside: std::collections::BTreeSet<usize> = comp.iter().copied().collect();
        let start = *inside.iter().next().expect("a node");
        let mut from = vec![usize::MAX; count];
        let mut queue = std::collections::VecDeque::from([start]);
        let mut end = None;
        'bfs: while let Some(v) = queue.pop_front() {
            for &w in &succ[v] {
                if !inside.contains(&w) {
                    continue;
                }
                if w == start {
                    end = Some(v);
                    break 'bfs;
                }
                if from[w] == usize::MAX {
                    from[w] = v;
                    queue.push_back(w);
                }
            }
        }
        let mut path = vec![start];
        let mut v = end.expect("a cycle in a component");
        while v != start {
            path.push(v);
            v = from[v];
        }
        path[1..].reverse();
        path.push(start);
        let names: Vec<&str> = path.iter().map(|&i| nodes[i].as_str()).collect();
        let mut members: Vec<&str> = comp.iter().map(|&i| nodes[i].as_str()).collect();
        members.sort_unstable();
        cycles.push(format!(
            "{} (component: {})",
            names.join(" -> "),
            members.join(", ")
        ));
    }
    cycles
}

/// A2-1: every cycle of the oxc visitor graph passes a gate of `DepthPass`, so the layer-2 walk
/// is a bounded recursion on every AST (also the TS nodes that eval's JS-mode parse keeps).
#[test]
fn the_depth_walk_gates_every_visitor_cycle() {
    let graph = visit_graph(&oxc_visit_source());
    let gates = depth_pass_gates();
    assert_eq!(gates.len(), 11, "the depth.rs texts say 11 gated points");
    for gate in &gates {
        assert!(
            graph.edges.contains_key(gate),
            "depth.rs gates visit_{gate}: no such walk"
        );
    }
    // A gate stops a cycle only if the depth grows on it.
    let flat = open_cycles(&graph.edges, |n| !graph.counted.contains(n));
    assert!(
        flat.is_empty(),
        "cycles with no enter_node:\n{}",
        flat.join("\n")
    );
    let open = open_cycles(&graph.edges, |n| !gates.contains(n));
    for cycle in &open {
        println!("OPEN CYCLE {cycle}");
    }
    assert!(
        open.is_empty(),
        "{} open cycles of the visitor graph (gates {gates:?}):\n{}",
        open.len(),
        open.join("\n")
    );
    // How far past the cap the walk can go: a gate lets the walk on at depth <= cap, and the
    // depth then grows by at most the counted nodes of a gate-free path to the next gate.
    let mut memo = std::collections::BTreeMap::new();
    let past = gates
        .iter()
        .map(|g| levels_to_a_gate(&graph, &gates, g, &mut memo))
        .max()
        .unwrap_or(0);
    println!(
        "VISIT GRAPH {} nodes, {} counted, gates {gates:?}: no open cycle; at most {past} \
         levels past the cap",
        graph.edges.len(),
        graph.counted.len()
    );
    // depth.rs says "at most 8 levels past the cap": a larger value needs that text changed.
    assert!(
        past <= 8,
        "{past} levels past the cap (the depth.rs texts say 8)"
    );
}

/// The most counted nodes on a path from `node` that enters no gate after `node` (the graph
/// without the gates is acyclic: checked before this call).
fn levels_to_a_gate(
    graph: &VisitGraph,
    gates: &std::collections::BTreeSet<String>,
    node: &str,
    memo: &mut std::collections::BTreeMap<String, u32>,
) -> u32 {
    if let Some(&v) = memo.get(node) {
        return v;
    }
    let below = graph.edges[node]
        .iter()
        .filter(|y| !gates.contains(*y))
        .map(|y| levels_to_a_gate(graph, gates, y, memo))
        .max()
        .unwrap_or(0);
    let v = u32::from(graph.counted.contains(node)) + below;
    memo.insert(node.to_owned(), v);
    v
}

// The run tests of A2-1. The parser builds the first 4 chains in a loop, so a text of n levels
// parses on a small stack and only the walk can recurse n levels. The parser builds the last 2
// (a namespace path, nested JSX elements) by recursion: no text reaches a depth where the walk
// overflows before the parse does, so the test builds their AST in a loop.

/// The chains of A2-1 and of the other open cycles of the visitor-graph proof.
const CHAINS: [&str; 6] = [
    "ts-as-type-name",
    "ts-satisfies-type-name",
    "ts-import-type-qualifier",
    "jsx-member-name",
    "ts-namespace-path (built)",
    "jsx-nested-elements (built)",
];

/// The text of the type-name chains at `n` levels (eval parses it as JS: a SyntaxError).
fn type_name_chain(case: usize, n: usize) -> String {
    let head = match case {
        0 => "x as a",
        1 => "x satisfies a",
        2 => "x as import(\"m\").a",
        _ => unreachable!("case {case}"),
    };
    let mut s = String::with_capacity(head.len() + 2 * n);
    s.push_str(head);
    for _ in 0..n {
        s.push_str(".a");
    }
    s
}

/// Build chain `case` of [`CHAINS`] at `n` levels and run the layer-2 walk with `cap`.
fn chain_depth(case: usize, n: usize, cap: u32) -> Result<u32, depth::TooDeep> {
    with_chain(case, n, |program| depth::check_ast_depth(program, cap))
}

/// Build chain `case` of [`CHAINS`] at `n` levels and give its program to `f`.
fn with_chain<R>(case: usize, n: usize, f: impl FnOnce(&oxc_ast::ast::Program<'_>) -> R) -> R {
    use oxc_allocator::{CloneIn, Vec as ArenaVec};
    use oxc_ast::ast::{
        Expression, JSXChild, JSXElement, Statement, TSNamespaceDeclaration,
        TSNamespaceDeclarationBody,
    };
    use oxc_ast::builder::AstBuilder;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    let allocator = Allocator::default();
    let (src, source_type) = match case {
        0..=2 => (type_name_chain(case, n), SourceType::ts()),
        3 => (format!("<a{} />", ".a".repeat(n)), SourceType::jsx()),
        4 => ("namespace a {}".to_owned(), SourceType::ts()),
        5 => ("<a></a>".to_owned(), SourceType::jsx()),
        _ => unreachable!("case {case}"),
    };
    let mut parsed = Parser::new(&allocator, &src, source_type).parse();
    assert!(!parsed.diagnostics.has_errors(), "{}", CHAINS[case]);
    let ast = AstBuilder::new(&allocator);
    match &mut parsed.program.body[0] {
        Statement::TSNamespaceDeclaration(decl) if case == 4 => {
            // `namespace a.a.a... {}`: each level is the body of the one above.
            let mut body = decl.body.clone_in(&allocator);
            for _ in 0..n {
                body = TSNamespaceDeclarationBody::TSNamespaceDeclaration(
                    TSNamespaceDeclaration::boxed(
                        decl.span,
                        decl.id.clone_in(&allocator),
                        body,
                        decl.kind,
                        false,
                        &ast,
                    ),
                );
            }
            decl.body = body;
        }
        Statement::ExpressionStatement(stmt) if case == 5 => {
            // `<a><a>...</a></a>`: each level is the one child of the one above.
            let Expression::JSXElement(el) = &mut stmt.expression else {
                panic!("a JSX element");
            };
            let mut inner = el.clone_in(&allocator);
            for _ in 0..n {
                inner = JSXElement::boxed(
                    el.span,
                    el.opening_element.clone_in(&allocator),
                    ArenaVec::from_iter_in([JSXChild::Element(inner)], &ast),
                    el.closing_element.clone_in(&allocator),
                    &ast,
                );
            }
            el.children = ArenaVec::from_iter_in([JSXChild::Element(inner)], &ast);
        }
        _ => assert!(case < 4, "{}: the parsed head", CHAINS[case]),
    }
    f(&parsed.program)
}

/// A2-1 (unit): on a 16 MiB thread, each chain of 1,000,000 levels stops at the cap of 1,000
/// (layer 2 gives `TooDeep`, no backstop: the thread has no limit). Before the gates the walk
/// recursed once per level and the process aborted. One child process per chain.
#[test]
fn the_depth_walk_stops_on_every_chain() {
    const TEST: &str = "the_depth_walk_stops_on_every_chain";
    if let Some(i) = l1_child(TEST) {
        let result = std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || chain_depth(i, 1_000_000, 1_000))
            .unwrap()
            .join()
            .unwrap();
        let w = match result {
            Ok(depth) => format!("Ok({depth})"),
            Err(deep) if deep.stack_limit => "StackLimit".to_owned(),
            Err(_) => "TooDeep".to_owned(),
        };
        l1_line(TEST, i, &w);
        return;
    }
    // A short chain of each form is inside the cap (the builders give the AST they say).
    for (i, name) in CHAINS.iter().enumerate() {
        let small = chain_depth(i, 10, 1_000).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let deep = chain_depth(i, 100, 1_000).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(
            deep >= small + 90,
            "{name}: depth {small} at 10, {deep} at 100"
        );
    }
    let names: Vec<String> = CHAINS.iter().map(|&n| n.to_owned()).collect();
    for (name, w) in names.iter().zip(run_children(TEST, &names)) {
        assert_eq!(w, "TooDeep", "{name}");
    }
}

/// A2-1 (public path): `eval` of the 3 type-name chains at 4,000,000 levels (8 MB of text) gives
/// a SyntaxError (oxc parses them in JS mode with a non-fatal error; layer 2 stops the walk and
/// eval prefers the parse error). Before the gates each one aborted the process in debug and
/// release. One child process per chain.
#[test]
fn eval_of_a_type_name_chain_is_a_syntax_error() {
    const TEST: &str = "eval_of_a_type_name_chain_is_a_syntax_error";
    if let Some(i) = l1_child(TEST) {
        let src = type_name_chain(i, 4_000_000);
        let w = codegen_word(&crate::compile_eval(
            &src,
            &ep9_bytecode::EvalContext::default(),
            crate::CompileOptions::default(),
        ));
        l1_line(TEST, i, &w);
        return;
    }
    let names: Vec<String> = CHAINS[..3].iter().map(|&n| n.to_owned()).collect();
    for (name, w) in names.iter().zip(run_children(TEST, &names)) {
        assert_eq!(w, "Syntax", "{name}");
    }
}

/// A2-1: a valid deep type-name chain (TS source type, no parse error) gives the layer-2 result
/// on the real worker: within the cap `Ok`, at 4,000,000 levels RangeError `TooDeep(Ast)`, not
/// `StackLimit` (the walk stops at the cap, inside the budget). No public entry parses TS: this
/// is the result that a TS caller of `check_depth` gets. In a child process.
#[test]
fn a_deep_type_name_chain_is_too_deep() {
    const TEST: &str = "a_deep_type_name_chain_is_too_deep";
    let sizes = [1_000, 4_000_000];
    if let Some(i) = l1_child(TEST) {
        let w = crate::worker::run(crate::stack::WORKER_STACK_BYTES, move || {
            let src = format!("let x: {};", &type_name_chain(0, sizes[i])[5..]);
            let allocator = Allocator::default();
            let parsed =
                oxc_parser::Parser::new(&allocator, &src, oxc_span::SourceType::ts()).parse();
            assert!(!parsed.diagnostics.has_errors());
            word(&crate::check_depth(&parsed.program, &src).map_err(CompileError::Range))
        })
        .unwrap();
        l1_line(TEST, i, &w);
        return;
    }
    let names: Vec<String> = sizes.iter().map(|n| format!("n {n}")).collect();
    let words = run_children(TEST, &names);
    assert_eq!(words, ["Ok", "Range(TooDeep(Ast))"]);
}

/// A2-1 (e), a probe like `probe::measure_forms` (run it on demand, debug and release): the
/// stack bytes per AST level of the layer-2 walk on each chain of [`CHAINS`], at 500 and 1,500
/// levels with the cap `AST_CAP` (no stop), and the JS member chain `a.a.a` of the same size for
/// comparison. The depth-pass row of stack.rs is the unit that these must stay under.
#[test]
#[ignore = "a probe: run on demand with --nocapture"]
fn chain_walk_cost() {
    let per_level = |measure: &dyn Fn(usize) -> (usize, u32)| {
        // `deepest_point` gives the lowest address touched: it falls as the walk goes deeper.
        let ((low1, d1), (low2, d2)) = (measure(500), measure(1_500));
        low1.saturating_sub(low2) / usize::try_from(d2 - d1).unwrap()
    };
    std::thread::Builder::new()
        .stack_size(2 << 30)
        .spawn(move || {
            let paint = 64 << 20;
            for (i, name) in CHAINS.iter().enumerate() {
                let bytes = per_level(&|n| {
                    with_chain(i, n, |program| {
                        let (low, depth) = crate::probe::deepest_point(paint, || {
                            depth::check_ast_depth(program, AST_CAP)
                        });
                        (low, depth.expect("within the cap"))
                    })
                });
                println!("CHAIN-COST {name}: {bytes} B per AST level");
            }
            let bytes = per_level(&|n| {
                let src = format!("a{}", ".a".repeat(n));
                let allocator = Allocator::default();
                let parsed =
                    oxc_parser::Parser::new(&allocator, &src, oxc_span::SourceType::script())
                        .parse();
                let (low, depth) = crate::probe::deepest_point(paint, || {
                    depth::check_ast_depth(&parsed.program, AST_CAP)
                });
                (low, depth.expect("within the cap"))
            });
            println!("CHAIN-COST js-member-chain a.a.a: {bytes} B per AST level");
        })
        .unwrap()
        .join()
        .unwrap();
}
