//! Worker stack size, from measured stack use per depth unit (`probe.rs`, H23).
//!
//! ```text
//! BUDGET_BYTES = BASE + SAFETY x max( PRESCAN_CAP x PARSE_PER_UNIT,     oxc_parser (layer 0 bound)
//!                                     AST_CAP     x PASS_PER_LEVEL )    depth pass, oxc_semantic,
//!                                                                       scope pass (layer 2 bound)
//! WORKER_STACK_BYTES = BUDGET_BYTES + RED                               layer 1 (l1.rs): the stack
//!                                                                       limit is RED above the end
//! ```
//!
//! Every share that a test or a check takes as its budget is a share of `BUDGET_BYTES`, never of
//! `WORKER_STACK_BYTES`: RED is never budget. The ceiling asserts stay on `WORKER_STACK_BYTES`,
//! the real reservation.
//!
//! Every row: `probe::measure_forms` (T-A, n = 500 -> 1500) on the layer-1 build (oxc 0.158 with
//! patch 0002) of the 4 CI targets (macOS and Linux arm64, Linux and Windows x86-64), 2026-11-24,
//! the worst of the 8 runs (4 targets x 2 profiles) and its form. CI runs the depth tests (every
//! form at the caps) on the 4 targets in debug and in release.
//!
//! | Bytes per unit | debug | release | worst form |
//! |---|---|---|---|
//! | oxc_parser, per pre-scan unit | 6,480 | 1,600 | debug: `spread-args` (macOS arm64, Linux arm64); release: `bracket-function` and `function-body-list-first` (Windows x86-64) |
//! | oxc regex parser, per group | 16,864 | 784 | `regex-named-group` (debug: macOS arm64, Linux arm64; release: Windows x86-64); weighted 4 units, `prescan::REGEX_WEIGHT` |
//! | layer-2 depth pass, per AST level | 992 | 112 | debug: `bracket-array`; release: `class-heritage` (Windows x86-64) |
//! | oxc_semantic, per AST level | 1,600 | 368 | debug: `bracket-array`; release: `class-heritage` (Windows x86-64) |
//! | scope pass (`scope::analyze`), per AST level | 3,584 | 528 | `class-heritage` (debug: macOS arm64, Linux arm64; release: Windows x86-64) |
//!
//! The units from the rows (the rule of plan v17 section 5.2 step 3, and H01 B2):
//!
//! - `PARSE_PER_UNIT`: debug = the worst + 1,024 B (6,480 + 1,024 = 7,504). Release stays 2,560 B:
//!   the release worst (1,600 B) fits the T-A release budget (2,615 B), so it does not move.
//! - `REGEX_PER_GROUP` and `PASS_PER_LEVEL` (the factors of the budget checks of layer 1): the
//!   worst of the rows, rounded up to 8 B. `PASS_PER_LEVEL` is the worst of the 3 pass rows and
//!   of the 4 walks of the eval path (`probe::measure_forms` prints them as T-PASS). The pass bound
//!   stays below the parse bound in both profiles.
//! - `probe::measure_forms` asserts every row against its unit on each target.
//!
//! The reservation is virtual: a thread only commits the pages it touches. Normal code touches
//! well under 1 MiB of it.

use crate::depth::{AST_CAP, PRESCAN_CAP};

/// Fixed cost: thread start, the bases of oxc_parser (about 6 KiB) and oxc_semantic (about
/// 40 KiB), our pass frames, and slack.
const BASE: usize = 2 << 20;

/// Margin for targets that were not measured (Windows x64: shadow space and larger frames).
/// The run-time budget checks of layer 1 (l1.rs) use the same margin.
pub(crate) const SAFETY_NUM: usize = 3;
pub(crate) const SAFETY_DEN: usize = 2;

/// Layer 1 (l1.rs, H01): the stack limit of a worker is `RED` above the end of its stack. The
/// red zone holds the thread start frames above the first job, the frames between 2 checks, the
/// leaf calls below the last check (lexer, arena growth, a diagnostic) and the unwind path. It
/// is not budget (`BUDGET_BYTES`).
pub(crate) const RED: usize = 1 << 20;

/// oxc_regular_expression stack per group or class level (the table above: the worst regex row
/// of the 4 targets, 16,864 B debug, 784 B release): the per-literal budget check of layer 1
/// (`regex_check`). The regex parse has no check of its own.
#[cfg(debug_assertions)]
pub(crate) const REGEX_PER_GROUP: usize = 16_864;
#[cfg(not(debug_assertions))]
pub(crate) const REGEX_PER_GROUP: usize = 784;

/// oxc_parser stack per pre-scan unit: debug = the worst of the 4 targets (6,480 B) + 1,024 B;
/// release = 2,560 B (the worst, 1,600 B, fits the T-A release budget).
#[cfg(debug_assertions)]
const PARSE_PER_UNIT: usize = 7_504;
#[cfg(not(debug_assertions))]
const PARSE_PER_UNIT: usize = 2_560;

/// The largest per-AST-level cost of the walks that recurse over the AST after layer 2: the
/// depth pass, oxc_semantic, the scope pass and the 4 walks of the eval path (the worst: the
/// scope pass, `class-heritage`, 3,584 B debug, 528 B release). In both profiles the parse bound
/// is larger, so this constant does not set the stack size. The run-time budget check of layer 1
/// (l1.rs) uses it as the stack per AST level of the walks after layer 2.
#[cfg(debug_assertions)]
pub(crate) const PASS_PER_LEVEL: usize = 3_584;
#[cfg(not(debug_assertions))]
pub(crate) const PASS_PER_LEVEL: usize = 528;

const fn max(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

/// The stack that the parse and the passes may use on the worker: the base and both bounds with
/// the margin. Every budget share comes from this value (RED is not in it).
pub(crate) const BUDGET_BYTES: usize = BASE
    + max(
        PRESCAN_CAP as usize * PARSE_PER_UNIT,
        AST_CAP as usize * PASS_PER_LEVEL,
    ) / SAFETY_DEN
        * SAFETY_NUM;

/// Stack of the front-end worker thread: the budget and the red zone of layer 1 on top.
pub(crate) const WORKER_STACK_BYTES: usize = BUDGET_BYTES + RED;

// Checked in every build: the budget fits both bounds with the margin, and the reservation stays
// a sane size (under `CEILING`: 512 MiB in debug, 256 MiB in release). T-BUDGET: RED is on top
// of the budget, never inside it. The pass bound stays below the parse bound (H01 B2: the
// worst pass row does not grow the stack).
const _: () = {
    let parse = PRESCAN_CAP as usize * PARSE_PER_UNIT;
    let passes = AST_CAP as usize * PASS_PER_LEVEL;
    assert!(passes <= parse);
    assert!(BUDGET_BYTES >= BASE + parse + parse / 2);
    assert!(BUDGET_BYTES >= BASE + passes + passes / 2);
    assert!(BUDGET_BYTES + RED == WORKER_STACK_BYTES);
    assert!(WORKER_STACK_BYTES < CEILING);
};

/// The largest worker stack reservation: 512 MiB in debug (oxc debug frames are large, H01 B2),
/// 256 MiB in release.
#[cfg(debug_assertions)]
const CEILING: usize = 512 << 20;
#[cfg(not(debug_assertions))]
const CEILING: usize = 256 << 20;
