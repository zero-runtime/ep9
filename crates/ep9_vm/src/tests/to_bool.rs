//! TO-BOOL (M15-P1): the one ToBoolean truth table of the VM (`conv::cell_truth`), read by
//! `conv::to_boolean` and by the cell arm of the T0 handlers (`interp::ops::fast_truthy`).
//!
//! | Test | It proves |
//! |---|---|
//! | A1 | `cell_truth(k)` for every kind byte is the old rule (strings 1-7, BigInt 9, else true) |
//! | A2 | on one value of each row of the table: `to_boolean` equals its old body and the row; `fast_truthy` answers only the rows it may, with the arm on and off |
//! | A3 | the 4 ToBoolean opcodes at scale 1 and the 2 jumps at `Wide` scale, every dispatch and preset, the arm on and off: the same answers; the arm runs for the symbols and objects only, and never when it is off |
//! | A4 | every registered kind id under 64 has a value in the sweep |
//!
//! The rows (the TO-BOOL design table): 1 undefined, 2 null, 3 the booleans, 4 int32 0,
//! 5 other int32, 6 +0 and -0 as doubles, 7 NaN, 8 other doubles, 9 the empty string,
//! 10 other strings (each string kind), 11 0n, 12 other BigInts, 13 symbols, 14 objects (each JS
//! object kind), 16 the hole. Row 15 (an object that emulates undefined) has no value: ep9 has
//! no such object.

use std::cell::Cell;

use ep9_bytecode::{Instructions, Opcode as O, OperandScale as S, VerifiedUnit};
use ep9_heap::{CellKind, CellSize, KindError, KindId};
use ep9_value::Value;

use super::{Asm, config, isolate, on_big_stack, print, take_output, unit};
use crate::conv::{self, CellTruth, cell_truth};
use crate::frame::{FALSE, HOLE, NULL, TRUE, UNDEFINED};
use crate::interp::ops::{TO_BOOL_ARM_HITS, fast_truthy, to_bool_arm_override};
use crate::runtime::{Root, val};
use crate::symbol::{self, WellKnownSymbol};
use crate::{
    Args, Cx, Dispatch, ExecOptions, Feedback, Isolate, JsResult, Local, atoms, bigint, kinds,
    object, property, string, wrapper,
};

const _: () = assert!(
    kinds::SEQ_LATIN1.get() == 1
        && kinds::EXT_UTF16.get() == 7
        && kinds::SYMBOL.get() == 8
        && kinds::BIGINT.get() == 9
);

/// The values of the sweep: (row of the table, name, ToBoolean of the value).
const SWEEP: &[(u8, &str, bool)] = &[
    (1, "undefined", false),
    (2, "null", false),
    (3, "true", true),
    (3, "false", false),
    (4, "int32 0", false),
    (5, "int32 1", true),
    (5, "int32 -1", true),
    (5, "int32 min", true),
    (5, "int32 max", true),
    (6, "double +0", false),
    (6, "double -0", false),
    (7, "NaN", false),
    (8, "0.5", true),
    (8, "-1.5", true),
    (8, "5e-324", true),
    (8, "1e300", true),
    (8, "+Infinity", true),
    (8, "-Infinity", true),
    (9, "empty string", false),
    (10, "SEQ_LATIN1", true),
    (10, "SEQ_UTF16", true),
    (10, "CONS", true),
    (10, "SLICED", true),
    (10, "THIN", true),
    (11, "0n", false),
    (12, "1n", true),
    (12, "-1n", true),
    (12, "2n ** 64n", true),
    (13, "Symbol()", true),
    (13, "Symbol('d')", true),
    (13, "Symbol.iterator", true),
    (13, "Symbol.for('r')", true),
    (14, "{}", true),
    (14, "a function", true),
    (14, "[]", true),
    (14, "an Error", true),
    (14, "kind ITERATOR", true),
    (14, "new Boolean(false)", true),
    (14, "new Number(0)", true),
    (14, "new String('')", true),
    (14, "kind BOUND_FUNCTION", true),
    (14, "kind GENERATOR", true),
    (14, "kind SLOT_OBJECT", true),
    (14, "kind ARGUMENTS", true),
    (14, "kind MODULE_NAMESPACE", true),
    (14, "kind PROXY", true),
    (14, "kind PROXY_CALLABLE", true),
    (14, "view 29", true),
    (14, "view 30", true),
    (14, "view 31", true),
    (14, "view 32", true),
    (14, "view 33", true),
    (14, "view 34", true),
    (14, "view 35", true),
    (14, "view 36", true),
    (14, "view 37", true),
    (14, "view 38", true),
    (14, "view 39", true),
    (14, "view 40", true),
    (16, "the hole", true),
];

/// A BigInt cell from a StringIntegerLiteral.
fn big(cx: &mut crate::interp::Cx, s: &str) -> u64 {
    let b = ep9_bigint::parse_literal(s.as_bytes(), &mut || true)
        .unwrap()
        .unwrap();
    bigint::alloc(cx.rt(), &b)
}

/// A JS object of kind `k` with %Object.prototype%, no properties, its fixed fields undefined.
fn of_kind(cx: &mut crate::interp::Cx, k: KindId) -> u64 {
    let rt = cx.rt();
    object::new_js_object(rt, k, rt.root(Root::ObjectPrototype), 0)
        .to_value()
        .to_bits()
}

/// The value of entry `i` of [`SWEEP`]. GC point: the caller uses it before the next one.
fn make(cx: &mut crate::interp::Cx, i: usize) -> u64 {
    let rt = cx.rt();
    let name = SWEEP[i].1;
    if let Some(n) = name.strip_prefix("view ") {
        let k = KindId::new(n.parse().unwrap()).unwrap();
        assert!(kinds::is_view_kind(k.get()));
        return of_kind(cx, k);
    }
    match name {
        "undefined" => UNDEFINED,
        "null" => NULL,
        "true" => TRUE,
        "false" => FALSE,
        "int32 0" => Value::int32(0).to_bits(),
        "int32 1" => Value::int32(1).to_bits(),
        "int32 -1" => Value::int32(-1).to_bits(),
        "int32 min" => Value::int32(i32::MIN).to_bits(),
        "int32 max" => Value::int32(i32::MAX).to_bits(),
        "double +0" => Value::double(0.0).to_bits(),
        "double -0" => Value::double(-0.0).to_bits(),
        "NaN" => Value::double(f64::NAN).to_bits(),
        "0.5" => Value::double(0.5).to_bits(),
        "-1.5" => Value::double(-1.5).to_bits(),
        "5e-324" => Value::double(5e-324).to_bits(),
        "1e300" => Value::double(1e300).to_bits(),
        "+Infinity" => Value::double(f64::INFINITY).to_bits(),
        "-Infinity" => Value::double(f64::NEG_INFINITY).to_bits(),
        "empty string" => string::new_string(rt, ""),
        "SEQ_LATIN1" => string::new_string(rt, "a"),
        "SEQ_UTF16" => string::new_string_utf16(rt, &[0x3b1, 0x3b2]),
        "CONS" => {
            let a = string::new_string(rt, "to-bool-cons-");
            let b = string::new_string(rt, "with-no-atom-yet");
            string::concat(rt, a, b).unwrap()
        }
        "SLICED" => {
            let p = string::new_string(rt, "0123456789abcdefghijklmnopqrstuvwxyz");
            string::substring(rt, p, 3, 20)
        }
        "THIN" => {
            let a = string::new_string(rt, "to-bool-thin-");
            let b = string::new_string(rt, "with-no-atom-yet");
            let r = string::concat(rt, a, b).unwrap();
            let _atom = atoms::atomize(rt, r);
            r
        }
        "0n" => big(cx, "0"),
        "1n" => big(cx, "1"),
        "-1n" => big(cx, "-1"),
        "2n ** 64n" => big(cx, "18446744073709551616"),
        "Symbol()" => symbol::new_symbol(rt, UNDEFINED),
        "Symbol('d')" => {
            let d = string::new_string(rt, "d");
            symbol::new_symbol(rt, d)
        }
        "Symbol.iterator" => symbol::well_known(rt, WellKnownSymbol::Iterator),
        "Symbol.for('r')" => {
            let k = string::new_string(rt, "r");
            symbol::symbol_for(cx, k)
        }
        "{}" => object::new_object(rt, rt.root(Root::ObjectPrototype)),
        "a function" => {
            let key = atoms::intern_str(rt, "print");
            property::get(cx, rt.root(Root::GlobalObject), key).unwrap()
        }
        "[]" => object::new_array(rt, 0),
        "an Error" => object::new_js_object(rt, kinds::ERROR, rt.root(Root::ErrorPrototype), 0)
            .to_value()
            .to_bits(),
        "kind ITERATOR" => of_kind(cx, kinds::ITERATOR),
        "new Boolean(false)" => wrapper::new_wrapper(rt, rt.root(Root::BooleanPrototype), FALSE),
        "new Number(0)" => wrapper::new_wrapper(
            rt,
            rt.root(Root::NumberPrototype),
            Value::int32(0).to_bits(),
        ),
        "new String('')" => {
            let e = string::new_string(rt, "");
            wrapper::new_wrapper(rt, rt.root(Root::StringPrototype), e)
        }
        "kind BOUND_FUNCTION" => of_kind(cx, kinds::BOUND_FUNCTION),
        "kind GENERATOR" => of_kind(cx, kinds::GENERATOR),
        "kind SLOT_OBJECT" => of_kind(cx, kinds::SLOT_OBJECT),
        "kind ARGUMENTS" => of_kind(cx, kinds::ARGUMENTS),
        "kind MODULE_NAMESPACE" => of_kind(cx, kinds::MODULE_NAMESPACE),
        "kind PROXY" => of_kind(cx, kinds::PROXY),
        "kind PROXY_CALLABLE" => of_kind(cx, kinds::PROXY_CALLABLE),
        "the hole" => HOLE,
        _ => panic!("no value for {name}"),
    }
}

/// The body of `conv::to_boolean` before TO-BOOL (0b710261), frozen.
fn old_to_boolean(b: u64) -> bool {
    match b {
        TRUE => true,
        FALSE | UNDEFINED | NULL => false,
        _ => {
            let v = val(b);
            if let Some(i) = v.as_int32() {
                i != 0
            } else if let Some(d) = v.as_double() {
                !(d == 0.0 || d.is_nan())
            } else if object::is_string(b) {
                object::str_len(b) != 0
            } else if kinds::is_bigint(b) {
                !bigint::is_zero(b)
            } else {
                true
            }
        }
    }
}

/// A1: the table is the old rule for every kind byte.
#[test]
fn a1_cell_truth_is_the_old_rule_for_every_kind_byte() {
    for k in 0..=u8::MAX {
        let want = if matches!(k, 1..=7) {
            CellTruth::ByLength
        } else if k == kinds::BIGINT.get() {
            CellTruth::ByValue
        } else {
            CellTruth::Always
        };
        assert_eq!(cell_truth(k), want, "kind {k}");
    }
    assert_eq!(cell_truth(kinds::SYMBOL.get()), CellTruth::Always);
    assert_eq!(cell_truth(0), CellTruth::Always, "not a cell (the hole)");
}

/// Runs `f` with the interpreter context of a fresh isolate (no GC stress), then collects
/// and verifies the heap.
fn with_cx<R: Send + 'static>(f: impl FnOnce(&mut crate::interp::Cx) -> R + Send + 'static) -> R {
    on_big_stack(move || {
        let mut iso = isolate(config(Dispatch::DEFAULT, false));
        let r = f(&mut iso.cx);
        iso.collect_garbage();
        iso.verify_heap().unwrap();
        r
    })
}

/// A2: on one value of each row, `to_boolean` equals its old body and the row, and
/// `fast_truthy` gives no answer or the row's answer: with the arm on, an answer for rows
/// 1-5, 13 and 14 only (never for a double, a string, a BigInt or the hole); with it off,
/// for rows 1-5 only (the function before TO-BOOL).
#[test]
fn a2_to_boolean_and_fast_truthy_agree_on_every_row() {
    with_cx(|cx| {
        for (i, &(row, name, want)) in SWEEP.iter().enumerate() {
            let v = make(cx, i);
            assert_eq!(conv::to_boolean(v), want, "{name}: to_boolean");
            assert_eq!(old_to_boolean(v), want, "{name}: the old body");
            for arm in [true, false] {
                let got = fast_truthy(v, arm);
                assert!(
                    got.is_none_or(|b| b == want),
                    "{name}: fast_truthy(arm {arm}) = {got:?}"
                );
                let answers = matches!(row, 1..=5) || (arm && matches!(row, 13 | 14));
                assert_eq!(
                    got.is_some(),
                    answers,
                    "{name} (row {row}): fast_truthy(arm {arm})"
                );
            }
            if row == 14 {
                assert!(object::is_object(v), "{name}: a JS object");
            }
            if row == 10 {
                assert!(object::is_string(v), "{name}: a string");
            }
        }
        // Each string kind of row 10 is the kind it names.
        for (i, &(_, name, _)) in SWEEP.iter().enumerate() {
            let k = match name {
                "SEQ_LATIN1" => kinds::SEQ_LATIN1,
                "SEQ_UTF16" => kinds::SEQ_UTF16,
                "CONS" => kinds::CONS,
                "SLICED" => kinds::SLICED,
                "THIN" => kinds::THIN,
                _ => continue,
            };
            let v = make(cx, i);
            assert_eq!(kinds::cell_kind(v), k.get(), "{name}");
        }
    });
}

/// A4: every kind id under 64 that the heap has registered (a second registration is
/// `KindError::Taken`) has a value of that kind in the sweep, and the sweep has no value of a
/// kind that is not registered.
#[test]
fn a4_every_registered_kind_under_64_has_a_sweep_value() {
    with_cx(|cx| {
        fn no_trace(_: ep9_heap::CellPtr, _: &mut ep9_heap::Tracer<'_>) {}
        let probe = CellKind {
            name: "to-bool-probe",
            size: CellSize::Fixed(16),
            trace: no_trace,
        };
        let mut swept = [false; 64];
        for i in 0..SWEEP.len() {
            let k = kinds::cell_kind(make(cx, i));
            if k != 0 {
                swept[usize::from(k)] = true;
            }
        }
        let mut registered = Vec::new();
        for k in 1..64u8 {
            let id = KindId::new(k).unwrap();
            // The probe registers only kinds that nobody registered; this isolate is dropped.
            match cx.rt().heap.register_kind(id, probe) {
                Err(KindError::Taken(_)) => registered.push(k),
                Ok(()) => {}
                Err(e) => panic!("kind {k}: {e:?}"),
            }
        }
        for &k in &registered {
            assert!(
                swept[usize::from(k)],
                "registered kind {k} has no sweep value"
            );
        }
        for k in 1..64u8 {
            assert!(
                !swept[usize::from(k)] || registered.contains(&k),
                "kind {k} not registered"
            );
        }
        // The hand count: 5 string kinds, SYMBOL, BIGINT, 13 object kinds, 12 views.
        assert_eq!(registered.len(), 5 + 2 + 13 + 12, "{registered:?}");
    });
}

// ---- A3: the handlers ------------------------------------------------------------------------

thread_local! {
    /// The sweep entry that `tbv()` gives (one isolate per thread).
    static ENTRY: Cell<usize> = const { Cell::new(0) };
}

/// `tbv()`: the value of the current sweep entry, made fresh.
fn tbv<'s>(
    cx: &mut Cx<'s>,
    _this: Local<'s>,
    _args: Args<'s>,
    _nt: Local<'s>,
) -> JsResult<Local<'s>> {
    let i = ENTRY.with(Cell::get);
    Ok(Local::from_bits(make(cx.inner(), i)))
}

/// `hits()`: the arm hits of this thread so far.
fn hits<'s>(
    _cx: &mut Cx<'s>,
    _this: Local<'s>,
    _args: Args<'s>,
    _nt: Local<'s>,
) -> JsResult<Local<'s>> {
    let n = TO_BOOL_ARM_HITS.with(Cell::get);
    Ok(Local::from_bits(Value::int32(n as i32).to_bits()))
}

/// The filler between a `Wide` jump and its target: 300 one-byte instructions.
const FILLER: usize = 300;

/// `acc = tbv()` (clobbers `f`).
fn load(s: &mut Asm) {
    let f = s.f;
    s.lda_global("tbv");
    s.b.star(f);
    s.b.call_undefined_receiver0(f, s.call);
}

/// `print("hits"); print(hits())` (clobbers `f`, `a[0]` and the accumulator).
fn print_hits(s: &mut Asm) {
    let f = s.f;
    s.lda_str("hits");
    s.print_acc();
    s.lda_global("hits");
    s.b.star(f);
    s.b.call_undefined_receiver0(f, s.call);
    s.print_acc();
}

/// A jump `op` over `filler` one-byte instructions: prints 1 when it jumps, else 0.
fn jump(s: &mut Asm, op: O, filler: usize) {
    load(s);
    let (t, e) = (s.b.new_label(), s.b.new_label());
    match op {
        O::JumpIfToBooleanTrue => s.b.jump_if_to_boolean_true(t),
        _ => s.b.jump_if_to_boolean_false(t),
    };
    s.b.lda_smi(0);
    s.print_acc();
    s.b.jump(e);
    for _ in 0..filler {
        s.b.lda_zero();
    }
    s.b.bind(t);
    s.b.lda_smi(1);
    s.print_acc();
    s.b.bind(e);
}

/// The unit of A3: `ToBoolean`, `ToBooleanLogicalNot` and the 2 jumps at scale 1, the hits,
/// then the 2 jumps at `Wide` scale, the hits.
fn a3_unit() -> VerifiedUnit {
    let mut s = Asm::script(0);
    load(&mut s);
    s.b.to_boolean();
    s.print_acc();
    load(&mut s);
    s.b.to_boolean_logical_not();
    s.print_acc();
    jump(&mut s, O::JumpIfToBooleanTrue, 0);
    jump(&mut s, O::JumpIfToBooleanFalse, 0);
    print_hits(&mut s);
    jump(&mut s, O::JumpIfToBooleanTrue, FILLER);
    jump(&mut s, O::JumpIfToBooleanFalse, FILLER);
    print_hits(&mut s);
    s.b.lda_undefined().return_();
    unit("", vec![s.finish()])
}

/// Every preset and feedback mode this build runs.
fn presets() -> Vec<ExecOptions> {
    let mut v = vec![
        ExecOptions::jitless(),
        ExecOptions::jitless().feedback(Feedback::FULL),
    ];
    if crate::Preset::JIT.available() {
        v.push(ExecOptions::jit());
        v.push(ExecOptions::jit().feedback(Feedback::ICS_ONLY));
    }
    v
}

/// Runs the A3 unit for sweep entry `i` with the arm `on` (the override is set in the thread
/// of the isolate): the printed answers, and the arm hits after the scale-1 part and after
/// the `Wide` part.
fn a3_run(i: usize, d: Dispatch, exec: ExecOptions, on: bool) -> (Vec<String>, [i64; 2]) {
    on_big_stack(move || {
        to_bool_arm_override::ON.set(Some(on));
        ENTRY.with(|e| e.set(i));
        TO_BOOL_ARM_HITS.with(|n| n.set(0));
        let mut iso = Isolate::new_with(config(d, false), exec).unwrap();
        iso.register_host_function("print", 1, print);
        iso.register_host_function("tbv", 0, tbv);
        iso.register_host_function("hits", 0, hits);
        take_output();
        let r = iso.with_cx_checkpoint(|cx| cx.run_script(a3_unit(), "to_bool.js").map(|_| ()));
        let out = take_output();
        to_bool_arm_override::ON.set(None);
        assert!(r.is_ok(), "{}: {:?}", SWEEP[i].1, r.err());
        iso.collect_garbage();
        iso.verify_heap().unwrap();
        // The lines: 4 answers, "hits", n1, 2 answers, "hits", n2.
        assert_eq!(out.len(), 10, "{out:?}");
        assert_eq!((out[4].as_str(), out[8].as_str()), ("hits", "hits"));
        let n = [out[5].parse().unwrap(), out[9].parse().unwrap()];
        let answers = [&out[0..4], &out[6..8]].concat();
        (answers, n)
    })
}

/// A3: each of the 4 opcodes on each sweep value gives the row's answer at scale 1, and the 2
/// jumps at `Wide` scale too, in every dispatch and preset, with the arm on and off. The arm
/// runs once per opcode run for a symbol or an object when it is on (4 at scale 1, 2 at
/// `Wide` scale: the prefixed jumps take it too), and never for another value or when it is
/// off.
#[test]
fn a3_the_handlers_give_the_row_answer_with_the_arm_on_and_off() {
    // The jumps of the second part are `Wide`, the others not.
    let u = a3_unit();
    let scales: Vec<(O, S)> = Instructions::new(&u.functions[0].bytecode)
        .map(|i| i.unwrap())
        .filter(|i| matches!(i.opcode, O::JumpIfToBooleanTrue | O::JumpIfToBooleanFalse))
        .map(|i| (i.opcode, i.scale))
        .collect();
    assert_eq!(
        scales,
        [
            (O::JumpIfToBooleanTrue, S::Single),
            (O::JumpIfToBooleanFalse, S::Single),
            (O::JumpIfToBooleanTrue, S::Wide),
            (O::JumpIfToBooleanFalse, S::Wide),
        ]
    );
    for (i, &(row, name, want)) in SWEEP.iter().enumerate() {
        let b = |x: bool| x.to_string();
        let n = |x: bool| String::from(if x { "1" } else { "0" });
        let answers = vec![b(want), b(!want), n(want), n(!want), n(want), n(!want)];
        let arm_rows = matches!(row, 13 | 14);
        for &d in Dispatch::ALL {
            for exec in presets() {
                for on in [true, false] {
                    let label = format!("{name}: {} {exec:?} arm {on}", d.name());
                    let (got, hits) = a3_run(i, d, exec, on);
                    assert_eq!(got, answers, "{label}");
                    let want_hits = if on && arm_rows { [4, 6] } else { [0, 0] };
                    assert_eq!(hits, want_hits, "{label}: the arm hits");
                }
            }
        }
    }
}

/// SET-15 (phase 2f): the isolate copies the handler side of the TO-BOOL bit into its table
/// once (`ops::cx_table`). With the bit off every entry is the base table of its dispatch and
/// feedback mode (`Dispatch::table`), so arm 0 runs the base handlers and no handler reads the
/// bit; with it on, the 6 entries of the TO-BOOL side (the 4 ToBoolean opcodes, `Wide` and
/// `ExtraWide`) differ, and no other entry. Every dispatch and preset.
#[test]
fn the_isolate_copies_the_handler_side_of_the_bit_once() {
    let words = |t: &[crate::interp::Handler; 256]| -> Vec<usize> {
        t.iter().map(|h| *h as usize).collect()
    };
    let side = [
        O::ToBooleanLogicalNot,
        O::ToBoolean,
        O::JumpIfToBooleanTrue,
        O::JumpIfToBooleanFalse,
        O::Wide,
        O::ExtraWide,
    ];
    for &d in Dispatch::ALL {
        for exec in presets() {
            // `None`: the arm bits of a test runtime (arm S: the bit is on).
            for on in [None, Some(true), Some(false)] {
                let (cx, base) = on_big_stack(move || {
                    to_bool_arm_override::ON.set(on);
                    let iso = Isolate::new_with(config(d, false), exec).unwrap();
                    to_bool_arm_override::ON.set(None);
                    let f = crate::exec::feedback_mode(&iso.rt);
                    (words(&iso.cx.table), words(d.table(f)))
                });
                let label = format!("{} {exec:?} override {on:?}", d.name());
                for (op, (&c, &b)) in cx.iter().zip(&base).enumerate() {
                    if on != Some(false) && side.iter().any(|&o| o as usize == op) {
                        assert_ne!(c, b, "{label}: entry {op:#x}, the TO-BOOL side");
                    } else {
                        assert_eq!(c, b, "{label}: entry {op:#x}, the base table");
                    }
                }
            }
        }
    }
}
