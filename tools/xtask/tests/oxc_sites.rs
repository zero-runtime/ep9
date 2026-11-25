//! `cargo xtask oxc-sites` (T-SITES) on the gap fixtures in `tests/oxc_sites/` (README there).
//!
//! Each fixture is a false PASS of the lint before route OXC-SITES-LINT-GAPS (review R2 lens B,
//! B2-1..B2-5; the M13-P0 review round 1, A1-A5, and round 2, A-B1..A-B4): an unbounded
//! recursion that the lint read as
//! safe. Each test runs the lint on
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

/// B2-5: two structs of one name whose fields of one name have other types: the field type is
/// not known (every method of the name is an edge), never the type of one of them.
#[test]
fn b2_5_a_field_of_two_structs_of_one_name_fails() {
    let (code, out, _) = oxc_sites("b2_5_field_collision");
    assert_eq!(code, Some(1));
    assert_cycle(&out, &["Ctx::go", "Walker::step", "again"]);
}

/// Review A1: an implicit call into a fn of an impl of a trait of another crate in a fn that
/// does not name the type: an index on a field, a drop of a field, a derived `Default` of a
/// holder, and a value made in macro tokens. Each closes a cycle.
#[test]
fn a1_an_implicit_call_in_a_fn_that_does_not_name_the_type_fails() {
    for (case, cycle) in [
        ("a1_index_field", &["Tab::index", "walk"][..]),
        ("a1_drop_field", &["Guard::drop", "parse_x"][..]),
        ("a1_derive_default", &["Guard::drop", "parse_x"][..]),
        ("a1_macro_tokens", &["Guard::drop", "parse_x"][..]),
    ] {
        let (code, out, _) = oxc_sites(case);
        assert_eq!(code, Some(1), "{case}");
        assert_cycle(&out, cycle);
    }
}

/// Review A2 and A5: an `Fn` bound on a method of the impl, a trait with `Fn` as a supertrait,
/// a parameter inside the argument of the self type, and an impl of a type alias each make a
/// fn value field.
#[test]
fn a2_a5_more_fn_bound_forms_make_a_fn_value_field() {
    for (case, at) in [
        (
            "a2_method_where",
            "lib.rs:5: Hook.f: the field holds a fn value",
        ),
        (
            "a2_fn_supertrait",
            "lib.rs:7: Hook.f: the field holds a fn value",
        ),
        (
            "a2_impl_wrapped",
            "lib.rs:6: Hook.f: the field holds a fn value",
        ),
        (
            "a5_alias_impl",
            "lib.rs:5: Hook.f: the field holds a fn value",
        ),
    ] {
        let (code, _, err) = oxc_sites(case);
        assert_eq!(code, Some(2), "{case}");
        assert!(err.contains(at), "{case}: {err}");
    }
}

/// Review A3: an ALLOW entry holds its use only at its own count, so a second call of the same
/// form in the fn keeps the edge.
#[test]
fn a3_a_second_call_of_an_allowed_form_fails() {
    let (code, out, _) = oxc_sites("a3_allow_count");
    assert_eq!(code, Some(1));
    assert_cycle(&out, &["parse", "parse_regex_pattern"]);
}

/// Review A4: the gate of a site is the config of the parser R (a type argument of the type
/// of R), not another type parameter of the fn.
#[test]
fn a4_a_gate_of_another_type_parameter_is_not_a_site() {
    let (code, out, _) = oxc_sites("a4_other_param_gate");
    assert_eq!(code, Some(1));
    assert!(
        has_line(
            &out,
            "oxc-sites: NOT A SITE FORM at lib.rs:8 in `parse_a`: ",
            &["a type argument of the type of R"]
        ),
        "{out}"
    );
    assert_cycle(&out, &["parse_a"]);
}

/// Review round 2, A-B1, A-B2 and A-B4: a fn value field when the type with the `Fn` bound
/// argument is inside another type (a slice, an `Option`, a `Box` self type, a type alias of a
/// `Box`), in the inputs of a trait fn or of a free fn, and when `Fn` or the struct has a
/// `use .. as` name.
#[test]
fn round_2_more_fn_bound_places_make_a_fn_value_field() {
    for (case, at) in [
        (
            "b1_slice_input",
            "lib.rs:5: Hook.f: the field holds a fn value",
        ),
        (
            "b1_option_input",
            "lib.rs:5: Hook.f: the field holds a fn value",
        ),
        (
            "b1_box_impl",
            "lib.rs:5: Hook.f: the field holds a fn value",
        ),
        (
            "b1_alias_box_impl",
            "lib.rs:5: Hook.f: the field holds a fn value",
        ),
        (
            "b2_trait_default",
            "lib.rs:5: Hook.f: the field holds a fn value",
        ),
        (
            "b4_fn_input",
            "lib.rs:5: Hook.f: the field holds a fn value",
        ),
        (
            "b4_use_as_fn",
            "lib.rs:7: Hook.f: the field holds a fn value",
        ),
        (
            "b4_use_as_impl",
            "lib.rs:6: Hook.f: the field holds a fn value",
        ),
    ] {
        let (code, _, err) = oxc_sites(case);
        assert_eq!(code, Some(2), "{case}");
        assert!(err.contains(at), "{case}: {err}");
    }
}

/// Review round 2, A-B3: a top-level `let` that binds the parser name again before the check
/// leaves R with no known config, so the check is not a site.
#[test]
fn round_2_a_gate_after_a_new_let_of_the_parser_is_not_a_site() {
    let (code, out, _) = oxc_sites("b3_rebound_parser");
    assert_eq!(code, Some(1));
    assert!(
        has_line(
            &out,
            "oxc-sites: NOT A SITE FORM at lib.rs:8 in `parse_a`: ",
            &["a type argument of the type of R"]
        ),
        "{out}"
    );
    assert_cycle(&out, &["parse_a"]);
}

/// Review round 2, A-B4: `self.t` of a known type `S` is the field `t` of `S` only, not the
/// field `t` of another struct: the cycle has `Tab::index` and no `Plain::index`.
#[test]
fn round_2_a_field_of_a_known_type_is_its_own_field() {
    let (code, out, _) = oxc_sites("b4_own_field");
    assert_eq!(code, Some(1));
    assert_cycle(&out, &["S::walk", "Tab::index", "back"]);
    assert!(!out.contains("Plain::index"), "{out}");
}
