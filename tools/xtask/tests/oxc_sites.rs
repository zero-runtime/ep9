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

/// B2-2 (a): a call through a trait renamed by `use .. as` is an edge to the fns of the trait.
#[test]
fn b2_2a_a_call_through_a_renamed_trait_fails() {
    let (code, out, _) = oxc_sites("b2_2a_renamed_trait");
    assert_eq!(code, Some(1));
    assert_cycle(&out, &["go", "parse_x"]);
}

/// B2-2 (b): a receiver whose type is a type alias of the sources has the type of the alias.
#[test]
fn b2_2b_a_call_on_a_type_alias_fails() {
    let (code, out, _) = oxc_sites("b2_2b_type_alias");
    assert_eq!(code, Some(1));
    assert_cycle(&out, &["parse_x", "walk"]);
}

/// B2-2 (c): a field whose type is a type parameter with an `Fn` bound holds a fn value, as a
/// fn pointer field does: the run stops (exit 2).
#[test]
fn b2_2c_a_generic_fn_field_fails() {
    let (code, _, err) = oxc_sites("b2_2c_generic_fn_field");
    assert_eq!(code, Some(2));
    assert!(
        err.contains("lib.rs:6: Hook.f: the field holds a fn value"),
        "{err}"
    );
}

/// B2-3: only the const of a type parameter of the fn or its impl gates a site.
#[test]
fn b2_3_a_gate_of_a_fixed_config_type_is_not_a_site() {
    let (code, out, _) = oxc_sites("b2_3_foreign_gate");
    assert_eq!(code, Some(1));
    assert!(
        has_line(
            &out,
            "oxc-sites: NOT A SITE FORM at lib.rs:7 in `parse_a`: ",
            &["CHECKS_RECURSION"]
        ),
        "{out}"
    );
    assert_cycle(&out, &["parse_a"]);
}

/// B2-4: an ALLOW entry removes only the edge of its own use, and an entry that removes no edge
/// is a finding.
#[test]
fn b2_4_allow_holds_only_its_own_use() {
    let (code, out, _) = oxc_sites("b2_4_allow_by_use");
    assert_eq!(code, Some(1));
    assert_cycle(&out, &["JSXClosing::dummy"]);
    assert!(
        has_line(
            &out,
            "oxc-sites: UNUSED ALLOW entry: ",
            &[
                "`JSXClosing::dummy` -> `JSXClosing::dummy`",
                "Path(\"Dummy\", \"dummy\")"
            ]
        ),
        "{out}"
    );
}
