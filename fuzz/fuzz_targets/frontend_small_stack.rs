//! Source text -> the compile entry on a small worker (2 MiB of room above the layer-1 limit,
//! then RED) -> the oracle: the result of the full worker, or RangeError StackLimit. Plan v18 §5
//! FUZZ-SMALL-STACK; H01 layer 1 (`crates/ep9_frontend/src/l1.rs`).
//!
//! The entry `CompileOptions::fuzz_compile_small_stack` is only in a `cargo fuzz build`
//! (`cfg(fuzzing)`; `crates/ep9_frontend/src/lib.rs`): no other build has it.
//!
//! ```text
//! input --> text --> small = the entry, on the caller thread "small-stack" (its worker is
//!                    always the small one: it makes no other front-end call)
//!   common::check(small)               Internal -> panic; a unit must reload
//!   small = Err(Range StackLimit)      fine: layer 1 stopped the input
//!   small = anything else              big = ep9_frontend::compile on this thread (the full
//!                                      worker); small must be big (the unit bytes or the error)
//!   a signal or an abort anywhere      a libFuzzer crash (the point of this target: the small
//!                                      stack makes the stack end near for every input)
//! ```
//!
//! Input: byte 0 picks the mode and the goal (0 sloppy script, 1 strict script, 2 module; mod 3).
//!
//! - byte 0 < 0x80, text mode: the rest is the source (the form of `frontend_compile`).
//! - byte 0 >= 0x80, form mode: bytes 1-2 (u16, little end) pick a row of the 2 nesting tables
//!   below (mod the row count), bytes 3-4 (u16, little end, the low 15 bits) are the level count
//!   n, and the rest (UTF-8) goes after the text of the row. The tables: FORMS
//!   (`crates/ep9_frontend/src/forms.rs`; its module forms as a module, the other forms with the
//!   goal of byte 0), then the `compile` rows of T-HOLES
//!   (`crates/ep9_frontend/tests/stack_holes.tsv`; the goal of the row), but not its slow rows
//!   (`hunt` = timing: their memory grows as n squared; route STACK-HOLES-SLOW).
#![no_main]

use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread;

use ep9_bytecode::VerifiedUnit;
use ep9_frontend::{
    CodegenError, CompileError, CompileOptions, RangeErrorKind, SourceOpts, Strictness,
};

#[path = "common.rs"]
mod common;

#[path = "../../crates/ep9_frontend/src/forms.rs"]
#[allow(dead_code, reason = "the target reads only module and build")]
mod forms;

/// The T-HOLES table (its header has the column rules).
const HOLES: &str = include_str!("../../crates/ep9_frontend/tests/stack_holes.tsv");

/// The largest level count of form mode (15 bits): above `PRESCAN_CAP` and `AST_CAP`.
const N_MASK: u16 = 0x7fff;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let goal = goal(mode);
    let (source, opts) = if mode < 0x80 {
        let Some(source) = common::text(rest) else {
            return;
        };
        (source.to_owned(), goal)
    } else {
        let [r0, r1, n0, n1, tail @ ..] = rest else {
            return;
        };
        let Some(tail) = common::text(tail) else {
            return;
        };
        let row = usize::from(u16::from_le_bytes([*r0, *r1]));
        let n = usize::from(u16::from_le_bytes([*n0, *n1]) & N_MASK);
        let (mut source, opts) = form_text(row, n, goal);
        source.push_str(tail);
        (source, opts)
    };
    let small = on_small_stack(&source, opts);
    let stopped = matches!(
        &small,
        Err(CodegenError::Compile(CompileError::Range(e))) if e.kind == RangeErrorKind::StackLimit
    );
    let small_bytes = small.as_ref().ok().map(|unit| unit.to_bytes());
    let small_err = small.as_ref().err().cloned();
    common::check(small);
    if stopped {
        return;
    }
    let big = ep9_frontend::compile(&source, opts, CompileOptions::default());
    match (small_bytes, small_err, big) {
        (Some(a), None, Ok(b)) => assert!(a == b.to_bytes(), "small and full worker units differ"),
        (None, Some(a), Err(b)) => {
            assert!(a == b, "small and full worker errors differ: {a:?} / {b:?}")
        }
        (_, small, big) => panic!(
            "small and full worker results differ: small {}, full {}",
            if small.is_none() { "Ok" } else { "Err" },
            if big.is_ok() { "Ok" } else { "Err" },
        ),
    }
});

fn goal(byte: u8) -> SourceOpts {
    match byte % 3 {
        0 => SourceOpts::script(Strictness::Sloppy),
        1 => SourceOpts::script(Strictness::Strict),
        _ => SourceOpts::module(),
    }
}

/// The text and goal of row `row` (FORMS, then T-HOLES; mod the row count) at `n` levels.
fn form_text(row: usize, n: usize, goal: SourceOpts) -> (String, SourceOpts) {
    let holes = holes();
    let row = row % (forms::FORMS.len() + holes.len());
    match forms::FORMS.get(row) {
        Some(form) if form.module => ((form.build)(n), SourceOpts::module()),
        Some(form) => ((form.build)(n), goal),
        None => holes[row - forms::FORMS.len()].text(n),
    }
}

/// One `compile` row of T-HOLES: its goal and its 5 text parts (unescaped).
struct Hole {
    goal: SourceOpts,
    /// prefix, open, mid, close, suffix.
    parts: [String; 5],
}

impl Hole {
    /// `prefix + open x n + mid + close x n + suffix`; `{i}` is the level index (the rule of
    /// `tests/stack_holes.rs`).
    fn text(&self, n: usize) -> (String, SourceOpts) {
        let [prefix, open, mid, close, suffix] = &self.parts;
        let mut s = String::with_capacity(prefix.len() + (open.len() + close.len() + 7) * n + 64);
        s.push_str(prefix);
        repeat_into(&mut s, open, n);
        s.push_str(mid);
        repeat_into(&mut s, close, n);
        s.push_str(suffix);
        (s, self.goal)
    }
}

fn repeat_into(s: &mut String, part: &str, n: usize) {
    match part.split_once("{i}") {
        Some((head, tail)) => {
            for i in 0..n {
                s.push_str(head);
                s.push_str(&i.to_string());
                s.push_str(tail);
            }
        }
        None => {
            for _ in 0..n {
                s.push_str(part);
            }
        }
    }
}

/// The `compile` rows of T-HOLES without the slow rows, read once.
fn holes() -> &'static [Hole] {
    static ROWS: OnceLock<Vec<Hole>> = OnceLock::new();
    ROWS.get_or_init(|| {
        HOLES
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|line| {
                let cols: Vec<&str> = line.split('\t').collect();
                assert_eq!(cols.len(), 13, "a T-HOLES row has 13 columns");
                if cols[3] != "compile" || cols[10] == "timing" {
                    return None;
                }
                let goal = match cols[2] {
                    "script" => SourceOpts::script(Strictness::Sloppy),
                    "strict" => SourceOpts::script(Strictness::Strict),
                    "module" => SourceOpts::module(),
                    other => panic!("T-HOLES goal {other:?}"),
                };
                Some(Hole {
                    goal,
                    parts: std::array::from_fn(|i| unescape(cols[4 + i])),
                })
            })
            .collect()
    })
}

/// The escapes of the T-HOLES table: `\n \r \t \\ \u{HEX}`.
fn unescape(field: &str) -> String {
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
                assert_eq!(chars.next(), Some('{'), "a T-HOLES \\u escape");
                let hex: String = chars.by_ref().take_while(|&c| c != '}').collect();
                let code = u32::from_str_radix(&hex, 16).expect("hex digits");
                out.push(char::from_u32(code).expect("a scalar value"));
            }
            other => panic!("a T-HOLES escape \\{other:?}"),
        }
    }
    out
}

type Small = Result<VerifiedUnit, CodegenError>;

/// The caller thread of the small-stack entry: one thread for the life of the process, which
/// makes no other front-end call, so its front-end worker is always the small one.
struct Caller {
    jobs: mpsc::Sender<(String, SourceOpts)>,
    results: mpsc::Receiver<Small>,
}

/// The small-stack compile of `source`, on the caller thread. A panic there (also one resumed
/// from its worker) aborts the process through the libFuzzer panic hook.
fn on_small_stack(source: &str, opts: SourceOpts) -> Small {
    static CALLER: OnceLock<Mutex<Caller>> = OnceLock::new();
    let caller = CALLER.get_or_init(|| {
        let (job_tx, job_rx) = mpsc::channel::<(String, SourceOpts)>();
        let (result_tx, result_rx) = mpsc::channel::<Small>();
        thread::Builder::new()
            .name("small-stack".into())
            .spawn(move || {
                for (source, opts) in job_rx {
                    let result = CompileOptions::default().fuzz_compile_small_stack(&source, opts);
                    if result_tx.send(result).is_err() {
                        return;
                    }
                }
            })
            .expect("the small-stack caller thread");
        Mutex::new(Caller {
            jobs: job_tx,
            results: result_rx,
        })
    });
    let caller = caller.lock().expect("one fuzz input at a time");
    caller
        .jobs
        .send((source.to_owned(), opts))
        .expect("the small-stack caller thread is alive");
    caller
        .results
        .recv()
        .expect("the small-stack caller thread gave a result")
}
