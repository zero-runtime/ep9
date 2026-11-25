//! `cargo xtask oxc-sites` (T-SITES) on the gap fixtures in `tests/oxc_sites/` (README there).
//!
//! Each fixture is a false PASS of the lint before route OXC-SITES-LINT-GAPS (review R2 lens B,
//! B2-1..B2-5): an unbounded recursion that the lint read as safe. Each test runs the lint on
//! one fixture dir and asserts the finding that the fixed lint gives. The fixtures are only
//! parsed, never built.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("oxc_sites")
        .join(case)
}

/// The lint on one fixture dir: (exit code, stdout, stderr).
fn oxc_sites(case: &str) -> (Option<i32>, String, String) {
    let out: Output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("oxc-sites")
        .arg("--src")
        .arg(fixture(case))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    eprintln!(
        "--- oxc-sites {case}: {}\n--- stdout\n{stdout}--- stderr\n{stderr}",
        out.status
    );
    (out.status.code(), stdout, stderr)
}

/// The line of `out` that starts with `head` and holds each of `parts`.
fn has_line(out: &str, head: &str, parts: &[&str]) -> bool {
    out.lines()
        .any(|l| l.starts_with(head) && parts.iter().all(|p| l.contains(p)))
}

/// A cycle with exactly these node labels (each with its place) is reported.
fn assert_cycle(out: &str, labels: &[&str]) {
    let head = format!("oxc-sites: CYCLE with no site ({} fns): ", labels.len());
    let at: Vec<String> = labels.iter().map(|l| format!("{l} (")).collect();
    let at: Vec<&str> = at.iter().map(String::as_str).collect();
    assert!(has_line(out, &head, &at), "no cycle {labels:?} in:\n{out}");
}

/// B2-1: a cycle that a `Drop` at the end of a scope closes. `parse_x` names `Guard` (a struct
/// literal), so it gets an edge to each fn of an impl of a trait of another crate for `Guard`.
#[test]
fn b2_1_a_cycle_closed_by_an_implicit_call_fails() {
    let (code, out, _) = oxc_sites("b2_1_implicit_drop");
    assert_eq!(code, Some(1));
    assert_cycle(&out, &["Guard::drop", "parse_x"]);
}
