//! T3 of the STAR-DST probe (ABI 7; DL M11-P3-PICK): each fused opcode
//! (`GetNamedPropertyStar`, `LdaCurrentContextSlotStar`, `LdaContextSlotStar`) runs as its pair
//! (the load, then `Star dst`). The front end emits the set by default; the units here are
//! hand-built with the generic emitters, so each test fixes the shape of the pair and of the
//! fused form.
//!
//! Each program is built twice, once with the pairs ([`Form::Pair`]) and once with the fused
//! opcodes ([`Form::Fused`]), and runs under every dispatch expansion, in both feedback modes
//! (`full` and `ics_only`: the two T0 handler tables), with and without GC stress, with eager
//! feedback. The 2 forms must print the same lines (0 mismatches: the FASTDIFF form of each
//! fused fast path against its pair) and export the same facts (each fused site is the same
//! feedback site as its pair, with the same records). The receivers come from the runtime
//! as globals: plain objects (in-object and out-of-line fields, a prototype field, a missing
//! name), an array and a string (the length handlers), a getter that returns and one that
//! throws, a proxy with a `get` trap, more shapes than a polymorphic entry holds (the stub
//! cache probe of the fast body), and `undefined` and `null` (the TypeError). Each fused form
//! also runs with a `Wide` and an `ExtraWide` prefix (`dst` past r127 and past r32767), and the
//! context forms at depth 0 and 1 and on their cold path.

use ep9_bytecode::{
    ConstIndex, FeedbackSlot, FunctionCode, FunctionFlags, FunctionId, HandlerKind, Register,
    RegisterList, RuntimeId, ScopeId, ScopeKind, VariableMode, VerifiedUnit,
};

use super::ic::{int, key, object_prototype, object_with};
use super::{Asm, ScopeDef, config, meta, on_big_stack, print, take_output, unit_with_scopes};
use crate::abi::export_facts;
use crate::frame::{NULL, UNDEFINED};
use crate::object::{self, attr};
use crate::runtime::Runtime;
use crate::{
    Args, Cx, Dispatch, ExecOptions, Feedback, Isolate, JsResult, Local, accessor, ic, props,
    proxy, realm, string,
};

/// The pair or the fused opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    Pair,
    Fused,
}

/// A load of the fused set.
#[derive(Clone, Copy)]
enum Load {
    /// `GetNamedProperty object, name, slot`.
    Named(Register, ConstIndex, FeedbackSlot),
    /// `LdaCurrentContextSlot slot_index`.
    Current(u32),
    /// `LdaContextSlot context, slot_index, depth`.
    Outer(Register, u32, u32),
}

/// `load` then `Star dst`: two instructions, or the one fused instruction.
fn load_star(a: &mut Asm, form: Form, load: Load, dst: Register) {
    let b = &mut a.b;
    match (form, load) {
        (Form::Pair, Load::Named(o, k, s)) => {
            b.get_named_property(o, k, s).star(dst);
        }
        (Form::Fused, Load::Named(o, k, s)) => {
            b.get_named_property_star(o, k, s, dst);
        }
        (Form::Pair, Load::Current(i)) => {
            b.lda_current_context_slot(i).star(dst);
        }
        (Form::Fused, Load::Current(i)) => {
            b.lda_current_context_slot_star(i, dst);
        }
        (Form::Pair, Load::Outer(c, i, d)) => {
            b.lda_context_slot(c, i, d).star(dst);
        }
        (Form::Fused, Load::Outer(c, i, d)) => {
            b.lda_context_slot_star(c, i, d, dst);
        }
    }
}

/// `try { body } catch {}` in a function: the handler does nothing.
fn try_catch(a: &mut Asm, body: impl FnOnce(&mut Asm)) {
    let ctx = a.b.new_register();
    let (start, end, handler, done) = (
        a.b.new_label(),
        a.b.new_label(),
        a.b.new_label(),
        a.b.new_label(),
    );
    a.b.lda_current_context().star(ctx);
    a.b.add_handler(start, end, handler, ctx, HandlerKind::Caught);
    a.b.bind(start);
    body(a);
    a.b.bind(end);
    a.b.jump(done);
    a.b.bind(handler);
    a.b.bind(done);
}

/// One test function: its global name and its code.
type Def = (String, FunctionCode);

/// The script: it defines the global of each function of `defs` (functions 1, 2, ...), then
/// for each (function, receiver global) of `calls` prints one line: `f(receiver)`, or
/// `throws: ` and the message of what it throws.
fn program(scopes: Vec<ScopeDef>, defs: Vec<Def>, calls: &[(String, String)]) -> VerifiedUnit {
    let mut s = Asm::script(0);
    let (c, x, ctx) = (s.b.new_register(), s.b.new_register(), s.b.new_register());
    s.b.lda_current_context().star(ctx);
    for (i, (name, _)) in defs.iter().enumerate() {
        let k = s.b.function_constant(FunctionId(i as u32 + 1));
        s.b.create_closure(k, s.closure, 0);
        s.sta_global(name);
    }
    for (f, recv) in calls {
        let (start, end, handler, done) = (
            s.b.new_label(),
            s.b.new_label(),
            s.b.new_label(),
            s.b.new_label(),
        );
        s.b.add_handler(start, end, handler, ctx, HandlerKind::Caught);
        s.b.bind(start);
        s.lda_global(f);
        s.b.star(c);
        s.lda_global(recv);
        s.b.star(x);
        s.b.call_undefined_receiver1(c, x, s.call);
        s.print_acc();
        s.b.bind(end);
        s.b.jump(done);
        s.b.bind(handler);
        s.b.star(x);
        s.lda_str("throws: ");
        s.b.star(c);
        s.get_named(x, "message");
        s.b.add(c, s.bin);
        s.print_acc();
        s.b.bind(done);
    }
    s.b.lda_undefined().return_();
    let mut fns = vec![s.finish()];
    fns.extend(defs.into_iter().map(|(_, f)| f));
    unit_with_scopes("", scopes, fns)
}

/// What one run gives: the printed lines, the stub cache hits of the named loads (of the
/// one-byte fast bodies, `ic::FAST_PROBE_HITS`, and of all their probes, fast and cold bodies,
/// `ic::HANDLER_PROBE_HITS`), and the exported facts of the unit, one line per record (key,
/// facts, flags), with the unit hash written as `UNIT` (the call records name the unit by its
/// content hash, which differs between the 2 forms).
struct RunOut {
    out: Vec<String>,
    probes: [u32; 2],
    facts: Vec<String>,
}

/// The run of `make(form)` after `setup` defined its globals, in a new isolate of dispatch `d`
/// and feedback mode `f` (GC stress `stress`, eager feedback).
fn run_form(
    d: Dispatch,
    f: Feedback,
    stress: bool,
    setup: fn(&mut Cx<'_>) -> JsResult<()>,
    make: fn(Form) -> VerifiedUnit,
    form: Form,
) -> RunOut {
    on_big_stack(move || {
        let mut c = config(d, stress);
        c.feedback_threshold = 0;
        let mut iso = Isolate::new_with(c, ExecOptions::default().feedback(f)).unwrap();
        iso.register_host_function("print", 1, print);
        take_output();
        ic::FAST_PROBE_HITS.with(|c| c.set(0));
        ic::HANDLER_PROBE_HITS.with(|c| c.set(0));
        iso.with_cx_checkpoint(setup).unwrap();
        let unit = make(form);
        let hash = unit.content_hash();
        let r = iso.with_cx_checkpoint(|cx| cx.run_script(unit, "star.js").map(|_| ()));
        let mut out = take_output();
        if let Err(e) = r {
            out.push(format!("uncaught {e} {:?}", e.stack));
        }
        let probes = [
            ic::FAST_PROBE_HITS.with(|c| c.get()),
            ic::HANDLER_PROBE_HITS.with(|c| c.get()),
        ];
        let snap = export_facts(&iso, &hash).expect("the unit is loaded");
        let named = format!("{hash:?}");
        let facts = snap
            .records_with_meta()
            .map(|(k, v, m)| format!("{k:?} {v:?} {m:?}").replace(&named, "UNIT"))
            .collect();
        iso.collect_garbage();
        iso.verify_heap().unwrap();
        RunOut { out, probes, facts }
    })
}

/// What [`same`] gives: the output of the first run, the smallest hit counts of the fused
/// form in a run (`[fast bodies, all probes]`; all probes over the runs without GC stress
/// only, as a GC empties the stub cache and a prefixed load can then miss each time), and the
/// facts of the first run.
struct Same {
    out: Vec<String>,
    probes: [u32; 2],
    facts: Vec<String>,
}

/// Runs both forms of `make` everywhere: each run must print what the pair form printed under
/// the first dispatch, both forms must hit the stub cache as often, and both forms must export
/// the same facts (in `full` mode the facts are also the same in each run). With
/// `one_byte` (the fused instructions of `make` have no prefix) the hits of the fast bodies are
/// equal too; a `Wide` or `ExtraWide` fused load probes in its cold body
/// (`ic::get_named_cold_len`), where the pair with a one-byte load probed in the fast body.
fn same(
    setup: fn(&mut Cx<'_>) -> JsResult<()>,
    make: fn(Form) -> VerifiedUnit,
    one_byte: bool,
) -> Same {
    let mut first: Option<Vec<String>> = None;
    let mut first_facts: Option<Vec<String>> = None;
    let mut probes = [u32::MAX; 2];
    for &d in Dispatch::ALL {
        for f in [Feedback::FULL, Feedback::ICS_ONLY] {
            for stress in [false, true] {
                let pair_run = run_form(d, f, stress, setup, make, Form::Pair);
                let fused_run = run_form(d, f, stress, setup, make, Form::Fused);
                let (pair, pair_probes) = (pair_run.out, pair_run.probes);
                let (fused, fused_probes) = (fused_run.out, fused_run.probes);
                let label = format!("{} {} stress={stress}", d.name(), f.name());
                let diff: Vec<String> = pair
                    .iter()
                    .zip(&fused)
                    .enumerate()
                    .filter(|(_, (p, f))| p != f)
                    .map(|(i, (p, f))| format!("line {i}: pair {p:?}, fused {f:?}"))
                    .collect();
                assert!(
                    diff.is_empty() && pair.len() == fused.len(),
                    "{label}: the fused form differs from the pair ({} and {} lines):\n{}",
                    pair.len(),
                    fused.len(),
                    diff.join("\n")
                );
                // The same IC path: the fast body probes the stub cache as often (one-byte forms).
                assert_eq!(pair_probes[1], fused_probes[1], "{label}: stub cache hits");
                if one_byte {
                    assert_eq!(pair_probes[0], fused_probes[0], "{label}: fast-body hits");
                }
                // The same feedback site: the same records under the same keys (V1, A3 of the
                // review round 1).
                assert_eq!(
                    pair_run.facts, fused_run.facts,
                    "{label}: the facts of the fused form differ from the pair"
                );
                assert!(!fused_run.facts.is_empty(), "{label}: no facts");
                probes[0] = probes[0].min(fused_probes[0]);
                if !stress {
                    probes[1] = probes[1].min(fused_probes[1]);
                }
                match &first {
                    None => first = Some(pair),
                    Some(f0) => assert_eq!(&pair, f0, "{label} differs from the first run"),
                }
                if f == Feedback::FULL {
                    match &first_facts {
                        None => first_facts = Some(fused_run.facts),
                        Some(f0) => {
                            assert_eq!(
                                &fused_run.facts, f0,
                                "{label}: facts differ from the first run"
                            );
                        }
                    }
                }
            }
        }
    }
    Same {
        out: first.unwrap(),
        probes,
        facts: first_facts.unwrap(),
    }
}

// ---- receivers (globals) --------------------------------------------------------------------

fn host(rt: &Runtime, f: crate::HostFn, name: &str) -> u64 {
    realm::new_host_function(rt, f, name, 0, false)
}

/// A getter: `return 2`.
fn get_two<'s>(_: &mut Cx<'s>, _: Local<'s>, _: Args<'s>, _: Local<'s>) -> JsResult<Local<'s>> {
    Ok(Local::int32(2))
}

/// A getter that throws a TypeError.
fn get_throws<'s>(cx: &mut Cx<'s>, _: Local<'s>, _: Args<'s>, _: Local<'s>) -> JsResult<Local<'s>> {
    Err(cx.throw_type_error("getter threw"))
}

/// A proxy `get` trap: `return 42`.
fn trap_get<'s>(_: &mut Cx<'s>, _: Local<'s>, _: Args<'s>, _: Local<'s>) -> JsResult<Local<'s>> {
    Ok(Local::int32(42))
}

fn set_global(cx: &mut Cx<'_>, name: &str, v: u64) -> JsResult<()> {
    let g = cx.global();
    let v: Local<'_> = Local::from_bits(v);
    cx.set(g, name, v, true).map(|_| ())
}

/// The receiver globals of the named loads (`MEGA`: the shapes that make a site
/// megamorphic, then `oIn` again).
const RECEIVERS: &[&str] = &[
    "oIn",
    "oIn",
    "oPoly",
    "oPoly",
    "oBig",
    "oBig",
    "oProto",
    "oMiss",
    "oGet",
    "oGet",
    "oThrow",
    "oThrow",
    "oProxy",
    "oProxy",
    "sAbc",
    "sAbc",
    "arr",
    "arr",
    "undefined",
    "nul",
    "m0",
    "m1",
    "m2",
    "m3",
    "m4",
    "m5",
    "oIn",
    "oIn",
    "m0",
    "m1",
];

fn receivers(cx: &mut Cx<'_>) -> JsResult<()> {
    let rt = cx.inner().rt();
    let p = object_prototype(rt);
    let o = object_with(rt, p, &[("x", int(1)), ("length", int(11))]);
    set_global(cx, "oIn", o)?;
    let o = object_with(rt, p, &[("y", int(0)), ("x", int(2)), ("length", int(12))]);
    set_global(cx, "oPoly", o)?;
    let names: Vec<String> = (0..40).map(|i| format!("p{i}")).collect();
    let mut fields: Vec<(&str, u64)> = names.iter().map(|n| (n.as_str(), int(0))).collect();
    fields.push(("x", int(3)));
    fields.push(("length", int(13)));
    let o = object_with(rt, p, &fields);
    set_global(cx, "oBig", o)?;
    let proto = object_with(rt, p, &[("x", int(4)), ("length", int(14))]);
    let o = object_with(rt, proto, &[("z", int(0))]);
    set_global(cx, "oProto", o)?;
    let o = object_with(rt, p, &[("z", int(0))]);
    set_global(cx, "oMiss", o)?;
    for (name, f) in [
        ("oGet", get_two as crate::HostFn),
        ("oThrow", get_throws as crate::HostFn),
    ] {
        let o = object::new_object(rt, p);
        for k in ["x", "length"] {
            let g = host(rt, f, "g");
            let pair = accessor::new_pair(rt, g, UNDEFINED);
            let a = attr::ACCESSOR | attr::ENUMERABLE | attr::CONFIGURABLE;
            props::own_define(rt, o, key(rt, k), pair, a);
        }
        set_global(cx, name, o)?;
    }
    let target = object_with(rt, p, &[("x", int(5))]);
    let handler = object::new_object(rt, p);
    let trap = host(rt, trap_get, "get");
    props::own_define(rt, handler, key(rt, "get"), trap, attr::DEFAULT);
    let px = proxy::proxy_create(cx.inner(), target, handler).expect("proxy_create");
    set_global(cx, "oProxy", px)?;
    let s = string::new_string(rt, "abc");
    set_global(cx, "sAbc", s)?;
    let items = [Local::int32(1), Local::int32(2), Local::int32(3)];
    let a = cx.create_array_from_list(&items)?.bits();
    set_global(cx, "arr", a)?;
    set_global(cx, "nul", NULL)?;
    for i in 0..6 {
        let first = format!("q{i}");
        let o = object_with(
            rt,
            p,
            &[
                (first.as_str(), int(0)),
                ("x", int(20 + i)),
                ("length", int(30 + i)),
            ],
        );
        set_global(cx, &format!("m{i}"), o)?;
    }
    Ok(())
}

// ---- the named load -------------------------------------------------------------------------

/// The test functions of the named form for the name `nm`, each `f(a)`:
///
/// - `dst`: `r = 7; r = a[nm]; return r` (the store of the pair);
/// - `acc`: `r = 7; r = a[nm]; return acc`;
/// - `self`: `a = a[nm]; return a` (`dst` is the object register, a parameter);
/// - `try`: `r = 7; try { r = a[nm] } catch {} return r` (a throw leaves `dst` as it was);
/// - `wide`, `xwide`: `dst` past r127 (a `Wide` prefix) and past r32767 (`ExtraWide`);
/// - `wobj`: the object in r200 (`Wide`), `dst` r4;
/// - `wtry`: `try` with a `Wide` `dst`.
fn named_defs(form: Form, nm: &str, cases: &[&str]) -> Vec<Def> {
    let mut out = Vec::new();
    for &case in cases {
        let mut a = Asm::new(meta(&format!("{nm}_{case}"), 1, 0, 0, 0));
        let k = a.b.string_constant(nm);
        let s = a.named;
        let p = a.b.parameter(0);
        let r = a.b.new_register();
        match case {
            "dst" | "acc" => {
                a.b.lda_smi(7).star(r);
                load_star(&mut a, form, Load::Named(p, k, s), r);
                if case == "dst" {
                    a.b.ldar(r);
                }
            }
            "self" => {
                load_star(&mut a, form, Load::Named(p, k, s), p);
                a.b.ldar(p);
            }
            "try" => {
                a.b.lda_smi(7).star(r);
                try_catch(&mut a, |a| load_star(a, form, Load::Named(p, k, s), r));
                a.b.ldar(r);
            }
            _ => {
                let n = if case == "xwide" { 33_000 } else { 200 };
                let list = a.b.new_register_list(n);
                let w = list.get(n - 1);
                match case {
                    "wobj" => {
                        a.b.ldar(p).star(w);
                        load_star(&mut a, form, Load::Named(w, k, s), r);
                        a.b.ldar(r);
                    }
                    "wtry" => {
                        a.b.lda_smi(7).star(w);
                        try_catch(&mut a, |a| load_star(a, form, Load::Named(p, k, s), w));
                        a.b.ldar(w);
                    }
                    _ => {
                        a.b.lda_smi(7).star(w);
                        load_star(&mut a, form, Load::Named(p, k, s), w);
                        a.b.ldar(w);
                    }
                }
            }
        }
        a.b.return_();
        out.push((format!("{nm}_{case}"), a.finish()));
    }
    out
}

const ONE_BYTE: &[&str] = &["dst", "acc", "self", "try"];
const PREFIXED: &[&str] = &["wide", "xwide", "wobj", "wtry"];

/// The receivers of the prefixed forms: a hit, a getter, a throw, a proxy, a string, an array,
/// the TypeError, and the `MEGA` shapes (the stub cache probe of the cold body); their frames
/// are large, so fewer calls.
const PREFIXED_RECEIVERS: &[&str] = &[
    "oIn",
    "oIn",
    "oGet",
    "oThrow",
    "oProxy",
    "sAbc",
    "arr",
    "undefined",
    "m0",
    "m1",
    "m2",
    "m3",
    "m4",
    "m5",
    "m0",
    "m1",
];

fn named_unit(form: Form) -> VerifiedUnit {
    named_unit_of(form, ONE_BYTE, RECEIVERS)
}

fn named_prefixed_unit(form: Form) -> VerifiedUnit {
    named_unit_of(form, PREFIXED, PREFIXED_RECEIVERS)
}

fn named_unit_of(form: Form, cases: &[&str], receivers: &[&str]) -> VerifiedUnit {
    let mut defs = named_defs(form, "x", cases);
    defs.extend(named_defs(form, "length", cases));
    let mut calls = Vec::new();
    for (f, _) in &defs {
        for r in receivers {
            calls.push((f.clone(), (*r).to_owned()));
        }
    }
    program(vec![], defs, &calls)
}

/// T3, the named form: every receiver kind, the hit, miss, getter, proxy and megamorphic
/// paths, at one-byte, `Wide` and `ExtraWide` scale, with `dst` a local, a parameter (the
/// object register) or past r127 and r32767.
#[test]
fn get_named_property_star_runs_as_its_pair() {
    let Same { out, probes, facts } = same(receivers, named_unit, true);
    // The megamorphic site hits the stub cache in the fast body (one-byte forms).
    assert!(probes[0] > 0, "no stub cache probe hit");
    // The fused named loads (functions 1-8) have LoadNamed records.
    let loads = facts
        .iter()
        .filter(|r| !r.contains("function: 0,") && r.contains("LoadNamed("))
        .count();
    assert!(
        loads >= 8,
        "{loads} LoadNamed records of the test functions: {facts:#?}"
    );
    let n = RECEIVERS.len();
    assert_eq!(out.len(), 8 * n);
    let line = |f: usize, r: usize| out[f * n + r].as_str();
    // x_dst: oIn 1, oGet 2 (the getter), oProxy 42 (the trap), sAbc undefined.
    assert_eq!(
        [line(0, 0), line(0, 8), line(0, 12), line(0, 14)],
        ["1", "2", "42", "undefined"]
    );
    // x_try: the getter throws inside the try; `dst` keeps 7. x_dst: the getter's TypeError
    // and the TypeError of `undefined.x` reach the script.
    assert_eq!(line(3, 10), "7");
    assert_eq!(line(0, 10), "throws: getter threw");
    assert!(line(0, 18).starts_with("throws: "), "{}", line(0, 18));
    // length_dst on the array and the string: 3.
    let length = 4 * n;
    assert_eq!(out[length + 16], "3");
    assert_eq!(out[length + 14], "3");
    // The prefixed forms print what the one-byte forms print (x_dst, x_try, length_dst):
    // wide, xwide and wobj as dst, wtry as try.
    let Same {
        out: wide, probes, ..
    } = same(receivers, named_prefixed_unit, false);
    // The `MEGA` shapes: a prefixed fused load probes the stub cache in its cold body.
    assert!(probes[1] > 0, "no stub cache probe hit (prefixed)");
    let m = PREFIXED_RECEIVERS.len();
    assert_eq!(wide.len(), 8 * m);
    let like = |f: usize| -> Vec<&str> {
        PREFIXED_RECEIVERS
            .iter()
            .map(|r| {
                let at = RECEIVERS.iter().position(|x| x == r).unwrap();
                out[f * n + at].as_str()
            })
            .collect()
    };
    for (f, one) in [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 3),
        (4, 4),
        (5, 4),
        (6, 4),
        (7, 7),
    ] {
        let got: Vec<&str> = wide[f * m..(f + 1) * m]
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(got, like(one), "prefixed function {f}");
    }
}

// ---- the context forms ----------------------------------------------------------------------

/// Scope 0: a function scope of 2 slots; scope 1: a block scope of 1 slot inside it; scope 2:
/// a function scope of 300 slots (a `Wide` slot index).
fn context_scopes() -> Vec<ScopeDef> {
    let wide: Vec<(&'static str, VariableMode)> = (0..300)
        .map(|i| {
            let s: &'static str = Box::leak(format!("w{i}").into_boxed_str());
            (s, VariableMode::Var)
        })
        .collect();
    vec![
        (
            ScopeKind::Function,
            None,
            vec![("p", VariableMode::Var), ("q", VariableMode::Var)],
        ),
        (ScopeKind::Block, Some(0), vec![("r", VariableMode::Var)]),
        (ScopeKind::Function, None, wide),
    ]
}

/// The context test functions, each `f(a)`. The function context (scope 0) holds 5 and 6;
/// the block context (scope 1) inside it holds 9.
fn context_defs(form: Form) -> Vec<Def> {
    let mut out = Vec::new();
    for case in [
        "cur",
        "cur_acc",
        "cur_wide",
        "cur_xwide",
        "cur_wslot",
        "out0",
        "out1",
        "out1_acc",
        "out_self",
        "out_wide",
        "out_xwide",
        "cold_cur",
        "cold_out",
        "cold_try",
    ] {
        let mut a = Asm::new(meta(case, 1, 0, 0, 0));
        let save = a.b.new_register();
        let r = a.b.new_register();
        let rc = a.b.new_register();
        let p = a.b.parameter(0);
        if case == "cur_wslot" {
            let ks = a.b.scope_constant(ScopeId(2));
            a.b.create_function_context(ks, 300).push_context(save);
            a.b.lda_smi(8).sta_current_context_slot(299);
            a.b.lda_smi(7).star(r);
            load_star(&mut a, form, Load::Current(299), r);
            a.b.ldar(r).return_();
            out.push((case.to_owned(), a.finish()));
            continue;
        }
        let ks0 = a.b.scope_constant(ScopeId(0));
        a.b.create_function_context(ks0, 2).push_context(save);
        a.b.lda_smi(5).sta_current_context_slot(0);
        a.b.lda_smi(6).sta_current_context_slot(1);
        let inner = a.b.new_register();
        let ks1 = a.b.scope_constant(ScopeId(1));
        if case.starts_with("out") || case.starts_with("cold") {
            a.b.create_block_context(ks1).push_context(inner);
            a.b.lda_smi(9).sta_current_context_slot(0);
        }
        a.b.lda_current_context().star(rc);
        a.b.lda_smi(7).star(r);
        let wide = |a: &mut Asm, n: u32| {
            let list = a.b.new_register_list(n);
            list.get(n - 1)
        };
        match case {
            "cur" => load_star(&mut a, form, Load::Current(1), r),
            "cur_acc" => {
                load_star(&mut a, form, Load::Current(0), r);
                a.b.return_();
            }
            "cur_wide" | "cur_xwide" => {
                let w = wide(&mut a, if case == "cur_wide" { 200 } else { 33_000 });
                load_star(&mut a, form, Load::Current(1), w);
                a.b.ldar(w).star(r);
            }
            "out0" => load_star(&mut a, form, Load::Outer(rc, 0, 0), r),
            "out1" => load_star(&mut a, form, Load::Outer(rc, 1, 1), r),
            "out1_acc" => {
                load_star(&mut a, form, Load::Outer(rc, 0, 1), r);
                a.b.return_();
            }
            "out_self" => {
                load_star(&mut a, form, Load::Outer(rc, 0, 1), rc);
                a.b.ldar(rc).star(r);
            }
            "out_wide" | "out_xwide" => {
                let w = wide(&mut a, if case == "out_wide" { 200 } else { 33_000 });
                a.b.ldar(rc).star(w);
                load_star(&mut a, form, Load::Outer(w, 1, 1), w);
                a.b.ldar(w).star(r);
            }
            // The block context has 1 slot: slot 1 is out of range (the cold body throws).
            "cold_cur" => load_star(&mut a, form, Load::Current(1), r),
            // The parameter is not a context.
            "cold_out" => load_star(&mut a, form, Load::Outer(p, 0, 0), r),
            _ => try_catch(&mut a, |a| load_star(a, form, Load::Current(1), r)),
        }
        a.b.ldar(r).return_();
        out.push((case.to_owned(), a.finish()));
    }
    out
}

fn context_unit(form: Form) -> VerifiedUnit {
    let defs = context_defs(form);
    let mut calls = Vec::new();
    for (f, _) in &defs {
        for _ in 0..3 {
            calls.push((f.clone(), "oIn".to_owned()));
        }
    }
    program(context_scopes(), defs, &calls)
}

/// T3, the context forms: depth 0 (the current context and a context register) and depth 1,
/// `dst` = the context register, `Wide` and `ExtraWide` forms (`dst`, and a slot index past
/// 255), and the cold path (a slot out of range, a register that is not a context; inside a
/// `try`, `dst` keeps its value).
#[test]
fn context_slot_star_runs_as_its_pair() {
    let out = same(receivers, context_unit, true).out;
    let want: Vec<&str> = ["6", "5", "6", "6", "8", "9", "6", "5", "5", "6", "6"]
        .iter()
        .flat_map(|v| [*v; 3])
        .collect();
    assert_eq!(out[..want.len()], want[..], "{out:?}");
    // cold_cur and cold_out throw; cold_try gives 7.
    let rest = &out[want.len()..];
    assert_eq!(rest.len(), 3 * 3, "{rest:?}");
    assert!(
        rest[..6].iter().all(|l| l.starts_with("throws: ")),
        "{rest:?}"
    );
    assert_eq!(rest[6..], ["7", "7", "7"]);
}

// ---- a mapped arguments object and a generator ----------------------------------------------

/// `function f(a, b)` (sloppy, scope 0 holds `a` and `b`, a mapped arguments object):
/// `b = a.x` with `dst` the parameter register `b`, then the values of the register `b`,
/// `arguments[1]` and the context slot of `b` (3 lines through `print`).
fn mapped_def(form: Form) -> Def {
    let mut a = Asm::new(meta("mapped", 2, 0, 0, 0));
    let ctx = a.b.new_register();
    let args = a.b.new_register();
    let ks = a.b.scope_constant(ScopeId(0));
    a.b.create_function_context(ks, 2).push_context(ctx);
    for i in 0..2 {
        a.b.ldar(Register::parameter(i)).sta_current_context_slot(i);
    }
    a.b.set_mapped_parameters(ScopeId(0), vec![Some(0), Some(1)]);
    a.b.create_mapped_arguments().star(args);
    let k = a.b.string_constant("x");
    let s = a.named;
    let (p0, p1) = (a.b.parameter(0), a.b.parameter(1));
    load_star(&mut a, form, Load::Named(p0, k, s), p1);
    a.b.ldar(p1);
    a.print_acc();
    a.b.lda_smi(1).get_keyed_property(args, a.keyed);
    a.print_acc();
    a.b.lda_current_context_slot(1);
    a.print_acc();
    a.b.ldar(p1).return_();
    ("mapped".to_owned(), a.finish())
}

/// A generator `gen(a)` in the V8 shape (`tests::generator`): after the initial suspend,
/// `r = 7; r = a.x; yield r; return r`. Every register is allocated before the first
/// suspend, so each suspend saves `dst`.
fn generator_def(form: Form) -> Def {
    let mut m = meta("gen", 1, 0, 0, 0);
    m.flags = FunctionFlags::SIMPLE_PARAMETERS | FunctionFlags::GENERATOR;
    let mut a = Asm::new(m);
    let g = a.b.new_register();
    let t = a.b.new_register_list(2);
    let r = a.b.new_register();
    let save = RegisterList::new(0, a.b.register_count());
    let (table, resumes) = a.b.jump_table(2);
    let this = a.b.receiver();
    a.b.lda_new_target().star(g);
    a.b.switch_on_generator_state(g, table);
    a.b.lda_current_function().star(t.get(0));
    a.b.ldar(this).star(t.get(1));
    a.b.call_runtime(RuntimeId::CreateGeneratorObject, t, 2);
    a.b.star(g);
    a.b.suspend_generator(g, save, save.len(), 0);
    a.b.bind(resumes[0]);
    a.b.resume_generator(g, save, save.len());
    a.b.lda_smi(7).star(r);
    let k = a.b.string_constant("x");
    let (p, s) = (a.b.parameter(0), a.named);
    load_star(&mut a, form, Load::Named(p, k, s), r);
    a.b.ldar(r).star(t.get(0)).lda_false().star(t.get(1));
    a.b.call_runtime(RuntimeId::CreateIterResultObject, t, 2);
    a.b.suspend_generator(g, save, save.len(), 1);
    a.b.bind(resumes[1]);
    a.b.resume_generator(g, save, save.len());
    a.b.ldar(r).return_();
    ("gen".to_owned(), a.finish())
}

/// `run(o)`: `mapped(o, 2)`, then `it = gen(o)`, and prints `it.next().value` 3 times.
fn caller_def() -> Def {
    let mut a = Asm::new(meta("run", 1, 0, 0, 0));
    let (c, x, it, next) = (
        a.b.new_register(),
        a.b.new_register(),
        a.b.new_register(),
        a.b.new_register(),
    );
    let p = a.b.parameter(0);
    a.lda_global("mapped");
    a.b.star(c);
    a.b.lda_smi(2).star(x);
    a.b.call_undefined_receiver2(c, p, x, a.call);
    a.lda_global("gen");
    a.b.star(c);
    a.b.call_undefined_receiver1(c, p, a.call);
    a.b.star(it);
    for _ in 0..3 {
        a.get_named(it, "next");
        a.b.star(next);
        a.b.call_property0(next, it, a.call);
        a.b.star(x);
        a.get_named(x, "value");
        a.print_acc();
    }
    a.b.lda_undefined().return_();
    ("run".to_owned(), a.finish())
}

fn frames_unit(form: Form) -> VerifiedUnit {
    let defs = vec![mapped_def(form), generator_def(form), caller_def()];
    let calls: Vec<(String, String)> = ["oIn", "oGet", "oProxy", "oIn", "m0", "m1"]
        .iter()
        .map(|r| ("run".to_owned(), (*r).to_owned()))
        .collect();
    let scopes = vec![(
        ScopeKind::Function,
        None,
        vec![("a", VariableMode::Var), ("b", VariableMode::Var)],
    )];
    program(scopes, defs, &calls)
}

/// T3: `dst` = a parameter of a function with a mapped arguments object (the object maps the
/// context slots, so `arguments[1]` keeps the argument), and a generator that suspends after
/// the fused instruction (the suspend saves `dst`, the resume restores it).
#[test]
fn fused_loads_in_mapped_and_generator_frames_run_as_their_pairs() {
    let out = same(receivers, frames_unit, true).out;
    // oIn: the register b is 1, arguments[1] and the context slot are 2; the generator yields
    // 1, returns 1, then gives undefined; run returns undefined.
    assert_eq!(
        out[..7],
        ["1", "2", "2", "1", "1", "undefined", "undefined"]
    );
}
