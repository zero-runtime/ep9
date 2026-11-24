//! A copy of the test suite of the oxc_parser patch
//! `third_party/patches/oxc/0002-parser-recursion-check.patch` (its new file
//! `crates/oxc_parser/tests/recursion_check.rs`), from the line after this head down to the test
//! `the_copy_follows_patch_0002`.
//!
//! Why it lives here: patch 0001 gives oxc_parser its published manifest, which sets
//! `autotests = false`, so cargo never runs the tests of the patched crate, and no CI step runs
//! the oxc workspace. Here the suite runs in every ep9_frontend test run (CI: 4 targets, debug
//! and release) against the path dependency of the pinned and patched oxc_parser.
//!
//! Keep it equal to the suite of patch 0002: at each move of the oxc pin, and at each change of
//! patch 0002, copy the suite again (`the_copy_follows_patch_0002` checks it). rustfmt formats
//! the copy with the ep9 settings, so the check compares the 2 texts without white space and
//! without a comma just before a closing bracket.
//!
//! The opt-in recursion check of `ParserConfig` (`CHECKS_RECURSION`, `may_recurse`).

use std::cell::Cell;

use oxc_allocator::Allocator;
use oxc_parser::{
    Parser,
    config::{NoTokensLexerConfig, ParserConfig},
};
use oxc_span::SourceType;

/// Allows `left` recursion entries, then stops, and stays stopped.
#[derive(Default)]
struct Budget {
    left: Cell<u32>,
}

impl ParserConfig for Budget {
    type LexerConfig = NoTokensLexerConfig;
    const CHECKS_RECURSION: bool = true;

    fn lexer_config(&self) -> NoTokensLexerConfig {
        NoTokensLexerConfig
    }

    fn may_recurse(&self) -> bool {
        let left = self.left.get();
        if left == 0 {
            return false;
        }
        self.left.set(left - 1);
        true
    }
}

/// Says "stop" once, at entry number `at` (from 1; 0 = never), and "go on" at every other
/// entry. A rewind after the stop drops its fatal error and the parse goes on, so only the stop
/// flag can give the result.
#[derive(Default)]
struct StopOnce {
    at: u32,
    calls: Cell<u32>,
}

impl ParserConfig for StopOnce {
    type LexerConfig = NoTokensLexerConfig;
    const CHECKS_RECURSION: bool = true;

    fn lexer_config(&self) -> NoTokensLexerConfig {
        NoTokensLexerConfig
    }

    fn may_recurse(&self) -> bool {
        let n = self.calls.get() + 1;
        self.calls.set(n);
        n != self.at
    }
}

struct Outcome {
    stack_limit_hit: bool,
    fatal_error: bool,
    messages: Vec<String>,
    statements: usize,
}

fn parse<C: ParserConfig>(source: &str, source_type: SourceType, config: C) -> Outcome {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, source_type)
        .with_config(config)
        .parse();
    Outcome {
        stack_limit_hit: ret.stack_limit_hit,
        fatal_error: ret.fatal_error,
        messages: ret.diagnostics.iter().map(ToString::to_string).collect(),
        statements: ret.program.body.len(),
    }
}

fn assert_stopped(o: &Outcome, what: &str) {
    assert!(o.stack_limit_hit, "{what}: no stop");
    assert!(o.fatal_error, "{what}: not fatal");
    assert_eq!(o.messages, ["Maximum call stack size exceeded"], "{what}");
    assert_eq!(o.statements, 0, "{what}: the program has a body");
}

fn deep(open: &str, mid: &str, close: &str, n: usize) -> String {
    format!("{}{mid}{}", open.repeat(n), close.repeat(n))
}

/// One input per recursion form, each `n` levels deep.
fn deep_inputs(n: usize) -> Vec<(String, SourceType)> {
    let js = SourceType::mjs();
    let ts = SourceType::ts();
    let jsx = SourceType::jsx();
    vec![
        (deep("[", "", "]", n), js),
        (deep("(", "a", ")", n), js),
        (deep("!", "a", "", n), js),
        (deep("++", "a", "", n), js),
        (deep("a ** ", "a", "", n), js),
        (deep("a = ", "0", "", n), js),
        (deep("a ? b : ", "c", "", n), js),
        (deep("new ", "a", "", n), js),
        (deep("`${", "a", "}`", n), js),
        (deep("{", "", "}", n), js),
        (deep("if (a) ", ";", "", n), js),
        (deep("function f() {", "", "}", n), js),
        (deep("x => ", "0", "", n), js),
        (format!("var {} = 0", deep("[", "a", "]", n)), js),
        (format!("{} = 0", deep("[", "a", "]", n)), js),
        (format!("({}) = 0", deep("(", "a", ")", n)), js),
        (format!("({}) => 0", deep("[", "a", "]", n)), js),
        (
            format!("async function f() {{ {} }}", deep("await ", "a", "", n)),
            js,
        ),
        (format!("let x: {}", deep("keyof ", "T", "", n)), ts),
        (format!("let x: {}", deep("(", "T", ")", n)), ts),
        (format!("let x: {}", deep("{ a: ", "T", " }", n)), ts),
        (format!("let x: {}", deep("[", "T", "]", n)), ts),
        (deep("<T>", "a", "", n), ts),
        (format!("namespace {}a {{}}", "a.".repeat(n)), ts),
        (deep("<a>", "", "</a>", n), jsx),
        (deep("<>", "", "</>", n), jsx),
    ]
}

#[test]
fn a_deep_input_stops_with_one_error() {
    for (source, source_type) in deep_inputs(2000) {
        let o = parse(
            &source,
            source_type,
            Budget {
                left: Cell::new(100),
            },
        );
        assert_stopped(&o, &source[..source.len().min(40)]);
    }
}

#[test]
fn a_config_that_goes_on_never_stops() {
    // 20 levels: every site is passed, and any stack is large enough. A config that checks and
    // always goes on gives the result of a config that does not check (`++ ++a` is an error in
    // both).
    for (source, source_type) in deep_inputs(20) {
        let what = &source[..source.len().min(40)];
        let plain = parse(
            &source,
            source_type,
            oxc_parser::config::NoTokensParserConfig,
        );
        let checked = parse(&source, source_type, StopOnce::default());
        assert!(!plain.stack_limit_hit && !checked.stack_limit_hit, "{what}");
        assert_eq!(
            (plain.fatal_error, &plain.messages, plain.statements),
            (checked.fatal_error, &checked.messages, checked.statements),
            "{what}"
        );
    }
}

#[test]
fn the_stop_survives_a_rewind() {
    // Inputs that the parser reads twice (a lookahead or a speculative parse that is rewound):
    // arrow heads, TS type arguments and parenthesized types, `async (` heads.
    let inputs = [
        ("((a, [b], {c: [d]}) => 0)", SourceType::mjs()),
        ("async (a = [[b]], ...c) => 0", SourceType::mjs()),
        ("async ([[a]]) => 0", SourceType::mjs()),
        ("((a)) ? (b) : (c) => d", SourceType::mjs()),
        ("let f = <T,>(a: [[T]]) => a", SourceType::tsx()),
        ("f<A<B<C>>>(x)", SourceType::ts()),
        ("let x: ((((a)))) = 0", SourceType::ts()),
        ("let x: ((a: [b]) => c) = 0", SourceType::ts()),
        ("(a?: [[b]]) => c", SourceType::ts()),
        ("<a b={[[c]]}><d.e.f></d.e.f></a>", SourceType::jsx()),
    ];
    for (source, source_type) in inputs {
        let total = count_calls(source, source_type);
        assert!(total > 0, "{source}: no recursion entry");
        let never = parse(source, source_type, StopOnce::default());
        assert!(
            !never.stack_limit_hit && !never.fatal_error,
            "{source}: {:?}",
            never.messages
        );
        for at in 1..=total {
            let o = parse(
                source,
                source_type,
                StopOnce {
                    at,
                    calls: Cell::new(0),
                },
            );
            assert_stopped(&o, &format!("{source} (stop at entry {at} of {total})"));
        }
    }
}

/// The number of recursion entries of a parse that never stops.
fn count_calls(source: &str, source_type: SourceType) -> u32 {
    thread_local!(static CALLS: Cell<u32> = const { Cell::new(0) });
    #[derive(Default)]
    struct Count;
    impl ParserConfig for Count {
        type LexerConfig = NoTokensLexerConfig;
        const CHECKS_RECURSION: bool = true;
        fn lexer_config(&self) -> NoTokensLexerConfig {
            NoTokensLexerConfig
        }
        fn may_recurse(&self) -> bool {
            CALLS.with(|c| c.set(c.get() + 1));
            true
        }
    }
    CALLS.with(|c| c.set(0));
    let _ = parse(source, source_type, Count);
    CALLS.with(Cell::get)
}

#[test]
fn the_stop_wins_over_a_lexer_error_at_the_end() {
    // A lexer error before the deep part: the parse exit keeps `fatal_error` but drops the
    // fatal message when the lexer has errors at EOF. The flag still gives the stop.
    let source = format!("'\\u{{110000}}'; {}", deep("[", "", "]", 2000));
    let o = parse(
        &source,
        SourceType::mjs(),
        Budget {
            left: Cell::new(100),
        },
    );
    assert_stopped(&o, "lexer error first");
    let o = parse(
        "'\\u{110000}';",
        SourceType::mjs(),
        oxc_parser::config::NoTokensParserConfig,
    );
    assert!(
        !o.messages.is_empty(),
        "the lexer error is a recoverable error"
    );
}

#[test]
fn chains_use_no_stack_per_member() {
    // A member chain is built by a loop; the `/*@__PURE__*/` walk down it and the compare of
    // 2 JSX name chains are loops too, so a long chain fits a small stack.
    let n = 200_000;
    let chain = ".a".repeat(n);
    let inputs = [
        (format!("/*@__PURE__*/a{chain}"), SourceType::mjs()),
        (format!("/*@__PURE__*/a(){chain}"), SourceType::mjs()),
        (format!("<a{chain}></a{chain}>"), SourceType::jsx()),
    ];
    for (source, source_type) in inputs {
        let o = std::thread::Builder::new()
            .stack_size(1 << 20)
            .spawn(move || {
                let o = parse(
                    &source,
                    source_type,
                    oxc_parser::config::NoTokensParserConfig,
                );
                (o.fatal_error, o.messages)
            })
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(o, (false, Vec::<String>::new()));
    }
}

/// ep9: this file holds the suite of patch 0002 (the new file of the patch, each `+` line
/// without its `+`), up to the white space and the trailing commas that rustfmt changes.
#[test]
fn the_copy_follows_patch_0002() {
    let patch = include_str!("../../../third_party/patches/oxc/0002-parser-recursion-check.patch");
    let mut lines = patch.lines();
    lines
        .by_ref()
        .find(|l| *l == "+++ b/crates/oxc_parser/tests/recursion_check.rs")
        .expect("patch 0002 adds the suite");
    let hunk = lines.next().expect("a hunk head");
    let count: usize = hunk
        .strip_prefix("@@ -0,0 +1,")
        .and_then(|rest| rest.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("a new-file hunk: {hunk}"));
    let mut suite = String::new();
    for line in lines.take(count) {
        let line = line.strip_prefix('+').expect("an added line");
        suite.push_str(line);
        suite.push('\n');
    }
    let this = include_str!("oxc_recursion_check.rs");
    let start = this
        .find("//! The opt-in recursion check")
        .expect("the suite head");
    assert!(
        squeeze(&this[start..]).starts_with(&squeeze(&suite)),
        "tests/oxc_recursion_check.rs differs from the suite of patch 0002: copy it again"
    );
}

/// `text` without white space and without a comma just before `)`, `]` or `}`.
fn squeeze(text: &str) -> String {
    let mut out: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    for close in [",)", ",]", ",}"] {
        out = out.replace(close, &close[1..]);
    }
    out
}
