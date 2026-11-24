//! The WIDE-SET probe (ABI 8; DL M12-P2-WIDE-GRANTS): W-1, the `Far` jump twins, and W-2, the
//! wide table (bit 1 of `EP9_WIDE_FORMS`, set here by the test override of `isolate.rs`).
//!
//! T5: each of the 11 twins is taken and not taken in every dispatch expansion and in both
//! feedback modes (`full` and `ics_only`: the two T0 handler tables), and gives what its base
//! gives at `Wide` scale. The 2 ToBoolean twins also run their cold body (an object and a
//! string accumulator). W-2: T13 (the table entries), T13b (check fix B1), T14 (more than 255
//! slots), T16 (the parse rule and arm 0).

use ep9_bytecode::{Instructions, Opcode, OperandScale, VerifiedUnit};

use super::{Asm, config, on_big_stack, print, take_output, unit};
use crate::{Dispatch, ExecOptions, Feedback, Isolate};

/// The accumulator values of each jump.
#[derive(Clone, Copy, Debug)]
enum V {
    True,
    False,
    Null,
    Undefined,
    Zero,
    One,
    Double,
    Empty,
    Str,
    Obj,
}

const VALUES: [V; 10] = [
    V::True,
    V::False,
    V::Null,
    V::Undefined,
    V::Zero,
    V::One,
    V::Double,
    V::Empty,
    V::Str,
    V::Obj,
];

/// The 11 one-operand forward jumps, in opcode order.
fn bases() -> Vec<Opcode> {
    (Opcode::Jump as u8..=Opcode::JumpIfJSReceiver as u8)
        .map(|b| Opcode::from_u8(b).unwrap())
        .collect()
}

/// Whether `base` jumps for the accumulator `v` (the JS meaning, not the VM).
fn taken(base: Opcode, v: V) -> bool {
    let truthy = matches!(v, V::True | V::One | V::Double | V::Str | V::Obj);
    match base {
        Opcode::Jump => true,
        Opcode::JumpIfTrue => matches!(v, V::True),
        Opcode::JumpIfFalse => matches!(v, V::False),
        Opcode::JumpIfToBooleanTrue => truthy,
        Opcode::JumpIfToBooleanFalse => !truthy,
        Opcode::JumpIfNull => matches!(v, V::Null),
        Opcode::JumpIfNotNull => !matches!(v, V::Null),
        Opcode::JumpIfUndefined => matches!(v, V::Undefined),
        Opcode::JumpIfNotUndefined => !matches!(v, V::Undefined),
        Opcode::JumpIfUndefinedOrNull => matches!(v, V::Null | V::Undefined),
        Opcode::JumpIfJSReceiver => matches!(v, V::Obj),
        other => unreachable!("{other}"),
    }
}

/// The script: for each base and value, `acc = value; <base> taken` over 300 bytes of
/// `LdaZero`, then `print("n")` on the not-taken path or `print("t")` at the target. With
/// `far` the builder makes the twins; without it the jumps are `Wide <base>`.
fn far_jump_unit(far: bool) -> VerifiedUnit {
    let mut s = Asm::script(0);
    s.b.set_far_jumps(far);
    for base in bases() {
        for v in VALUES {
            match v {
                V::True => {
                    s.b.lda_true();
                }
                V::False => {
                    s.b.lda_false();
                }
                V::Null => {
                    s.b.lda_null();
                }
                V::Undefined => {
                    s.b.lda_undefined();
                }
                V::Zero => {
                    s.b.lda_zero();
                }
                V::One => {
                    s.b.lda_smi(1);
                }
                V::Double => {
                    s.lda_num(1.5);
                }
                V::Empty => {
                    s.lda_str("");
                }
                V::Str => {
                    s.lda_str("x");
                }
                V::Obj => {
                    s.b.create_empty_object_literal();
                }
            }
            let (yes, done) = (s.b.new_label(), s.b.new_label());
            s.b.emit(base, &[ep9_bytecode::IntoOperand::into_pending(yes)]);
            for _ in 0..300 {
                s.b.lda_zero();
            }
            s.lda_str("n").print_acc();
            s.b.jump(done);
            s.b.bind(yes);
            s.lda_str("t").print_acc();
            s.b.bind(done);
        }
    }
    s.b.lda_undefined().return_();
    unit("", vec![s.finish()])
}

/// The lines the script prints.
fn want() -> Vec<String> {
    let mut out = Vec::new();
    for base in bases() {
        for v in VALUES {
            out.push(if taken(base, v) { "t" } else { "n" }.to_string());
        }
    }
    out
}

/// The printed lines of `unit` in a new isolate of dispatch `d` and feedback mode `f`.
fn run(d: Dispatch, f: Feedback, far: bool) -> Vec<String> {
    on_big_stack(move || {
        let mut iso =
            Isolate::new_with(config(d, false), ExecOptions::default().feedback(f)).unwrap();
        iso.register_host_function("print", 1, print);
        take_output();
        let r =
            iso.with_cx_checkpoint(|cx| cx.run_script(far_jump_unit(far), "far.js").map(|_| ()));
        let mut out = take_output();
        if let Err(e) = r {
            out.push(format!("uncaught {e}"));
        }
        iso.collect_garbage();
        iso.verify_heap().unwrap();
        out
    })
}

/// The far jumps of the script's code (opcode, scale), in order: a twin, or one of the 11
/// bases with a prefix (the `Jump done` of each case is short).
fn jumps(u: &VerifiedUnit) -> Vec<(Opcode, OperandScale)> {
    Instructions::new(&u.functions[0].bytecode)
        .map(|i| i.unwrap())
        .filter(|i| {
            let b = i.opcode.near_form();
            bases().contains(&b) && (b != i.opcode || i.scale != OperandScale::Single)
        })
        .map(|i| (i.opcode, i.scale))
        .collect()
}

#[test]
fn far_twins_taken_and_not_taken() {
    // The two units: every tested jump is a twin with `far`, `Wide <base>` without.
    let (on, off) = (far_jump_unit(true), far_jump_unit(false));
    let (jon, joff) = (jumps(&on), jumps(&off));
    assert_eq!(jon.len(), 11 * VALUES.len());
    assert_eq!(joff.len(), jon.len());
    for ((t, ts), (b, bs)) in jon.iter().zip(&joff) {
        assert_eq!((t.near_form(), *ts), (*b, OperandScale::Single), "{t}");
        assert_ne!(t, b);
        assert_eq!(*bs, OperandScale::Wide, "{b}");
    }
    let far_jump = Instructions::new(&on.functions[0].bytecode)
        .map(|i| i.unwrap())
        .filter(|i| i.opcode == Opcode::JumpFar)
        .count();
    assert_eq!(far_jump, VALUES.len());
    let want = want();
    for &d in Dispatch::ALL {
        for f in [Feedback::FULL, Feedback::ICS_ONLY] {
            for far in [false, true] {
                assert_eq!(run(d, f, far), want, "{} {f:?} far={far}", d.name());
            }
        }
    }
}

// ---- W-2: the wide table (T13, T13b, T14, T16; DL M12-P2-WIDE-GRANTS) ------------------------

use ep9_bytecode::{FeedbackSlotKind, FunctionBuilder, FunctionId};

use super::meta;
use crate::interp::Handler;
use crate::interp::ops::{WIDE_COUNT, if_wide, prefix_runs_fast};
use crate::isolate::wide_forms::{self, with_wide_table};
use crate::runtime::val;
use crate::{IsolateConfig, Value, feedback};

/// The runs of the `_w2` fast and cold handlers on this thread so far.
fn w2_runs() -> [u64; 2] {
    crate::interp::dsl::W2_RUNS.with(|c| c.get())
}

/// The runs of `interp::scaled::step` and `step_cold` on this thread so far.
fn scaled_runs() -> [u64; 2] {
    crate::interp::dsl::SCALED_RUNS.with(|c| c.get())
}

/// The wide list, by `if_wide!`.
fn wide_list() -> Vec<Opcode> {
    (0..=255u8)
        .filter_map(Opcode::from_u8)
        .filter(|&op| {
            macro_rules! is_wide {
                ($($name:ident),*) => {
                    match op {
                        $(Opcode::$name => if_wide!($name { true } else { false }),)*
                        _ => false,
                    }
                };
            }
            is_wide!(
                GetNamedProperty,
                SetNamedProperty,
                JumpLoop,
                Wide,
                GetNamedPropertyStar
            )
        })
        .collect()
}

/// The wide table of one expansion against what it must hold: the `_w2` handler of an opcode
/// of the wide list where `prefix_runs_fast` is false, else the `Wide` cold handler (also for
/// a byte that is no opcode).
fn check_wide(name: &str, t: &[Handler; 256], generic: Handler, w2: [(Opcode, Handler); 3]) {
    let mut own = Vec::new();
    for (b, &h) in t.iter().enumerate() {
        let want = w2
            .iter()
            .find(|(op, _)| *op as usize == b && !prefix_runs_fast(b as u8))
            .map_or(generic, |&(_, h)| h);
        assert_eq!(h as usize, want as usize, "{name}: entry {b:#04x}");
        if h as usize != generic as usize {
            own.push(Opcode::from_u8(b as u8).unwrap());
        }
    }
    // F3: aarch64 runs GetNamedProperty and JumpLoop in the `Wide` fast body.
    let want: &[Opcode] = if cfg!(target_arch = "aarch64") {
        &[Opcode::SetNamedProperty]
    } else {
        &[
            Opcode::GetNamedProperty,
            Opcode::SetNamedProperty,
            Opcode::JumpLoop,
        ]
    };
    assert_eq!(own, want, "{name}: the _w2 entries");
    assert!(Opcode::from_u8(0xff).is_none());
    assert_eq!(
        t[0xff] as usize, generic as usize,
        "{name}: a byte that is no opcode"
    );
}

/// T13: every entry of each wide table outside the wide list, a byte that is no opcode too, is
/// the `Wide` cold handler of its expansion; the entries of the list are the `_w2` handlers by
/// check fix B1 (x86-64: all 3; aarch64: only `SetNamedProperty_w2`).
#[test]
fn wide_table_entries() {
    assert_eq!(WIDE_COUNT, 3);
    assert_eq!(
        wide_list(),
        [
            Opcode::GetNamedProperty,
            Opcode::SetNamedProperty,
            Opcode::JumpLoop
        ]
    );
    macro_rules! check {
        ($($m:ident),*) => {$({
            use crate::interp::$m as m;
            check_wide(
                stringify!($m),
                &m::WIDE,
                m::cold::Wide as Handler,
                [
                    (Opcode::GetNamedProperty, m::w2::GetNamedProperty as Handler),
                    (Opcode::SetNamedProperty, m::w2::SetNamedProperty as Handler),
                    (Opcode::JumpLoop, m::w2::JumpLoop as Handler),
                ],
            );
        })*};
    }
    check!(sibling, sib_ics);
    #[cfg(feature = "become")]
    check!(tail, tail_ics);
}

/// T16 (VM half): the parse rule of `EP9_WIDE_FORMS`, the same table as ep9_frontend.
#[test]
fn wide_forms_parse_table() {
    use wide_forms::wide_forms_from as p;
    for (v, want) in [("1", 1), ("2", 2), ("3", 3)] {
        assert_eq!(p(Some(v)), want, "{v:?}");
    }
    assert_eq!(p(None), 0);
    for v in ["0", "", "4", "01", " 1", "1 ", "x"] {
        assert_eq!(p(Some(v)), 0, "{v:?}");
    }
}

/// T16 (arm 0): with bit 1 off, the `Cx.table` of every expansion and feedback mode is
/// `Dispatch::table` in all 256 entries (entry `Wide` = the `Wide` handler of today); with bit
/// 1 on, only entry `Wide` differs, and it is the `Wide_w` handler of the same table.
#[test]
fn arm0_table_is_the_base_table() {
    let wide = Opcode::Wide as usize;
    for &d in Dispatch::ALL {
        for f in [Feedback::FULL, Feedback::ICS_ONLY] {
            let base = d.table(f);
            let label = format!("{} {f:?}", d.name());
            let (off, iso_off, on, iso_on) = on_big_stack(move || {
                let new = || {
                    let iso =
                        Isolate::new_with(config(d, false), ExecOptions::default().feedback(f))
                            .unwrap();
                    iso.cx.table.map(|h| h as usize)
                };
                (
                    with_wide_table(false, || wide_forms::table(d, f).map(|h| h as usize)),
                    with_wide_table(false, new),
                    with_wide_table(true, || wide_forms::table(d, f).map(|h| h as usize)),
                    with_wide_table(true, new),
                )
            });
            let base = base.map(|h| h as usize);
            assert_eq!(off, base, "{label}: bit 1 off");
            assert_eq!(iso_off, base, "{label}: Cx.table with bit 1 off");
            assert_eq!(on, iso_on, "{label}: Cx.table with bit 1 on");
            for b in 0..256 {
                if b == wide {
                    assert_eq!(on[b], wide_forms::wide_w(d, f) as usize, "{label}");
                    assert_ne!(on[b], base[b], "{label}");
                } else {
                    assert_eq!(on[b], base[b], "{label}: entry {b:#04x}");
                }
            }
        }
    }
}

/// The script defines the global `f0(a, n)`: `for (i = 0; i < n; i++) { t = a.x; sum += t;
/// a.y = i } return sum`. Its feedback slots 2-301 are not used, so the slot operands of the
/// load, the store and the loop are past 255: each is a `Wide` instruction. The function
/// has more than 255 slots (the H15 rule row of T14).
pub(super) fn wide_slots_program() -> VerifiedUnit {
    let mut s = Asm::script(0);
    let k = s.b.function_constant(FunctionId(1));
    s.b.create_closure(k, s.closure, 0);
    s.sta_global("f0");
    s.b.lda_undefined().return_();
    let mut b = FunctionBuilder::new(meta("f0", 2, 0, 0, 0));
    let (a, n) = (b.parameter(0), b.parameter(1));
    let cmp = b.feedback_slot(FeedbackSlotKind::CompareOp);
    let bin = b.feedback_slot(FeedbackSlotKind::BinaryOp);
    for _ in 0..300 {
        b.feedback_slot(FeedbackSlotKind::BinaryOp);
    }
    let named = b.feedback_slot(FeedbackSlotKind::LoadNamed);
    let store = b.feedback_slot(FeedbackSlotKind::StoreNamed);
    let loop_ = b.feedback_slot(FeedbackSlotKind::JumpLoop);
    let (kx, ky) = (b.string_constant("x"), b.string_constant("y"));
    let (sum, i, t) = (b.new_register(), b.new_register(), b.new_register());
    b.lda_zero().star(sum).lda_zero().star(i);
    let header = b.bound_label();
    let exit = b.new_label();
    b.ldar(n).test_less_than(i, cmp).jump_if_false(exit);
    b.get_named_property(a, kx, named).star(t);
    b.ldar(t).add(sum, bin).star(sum);
    b.ldar(i).set_named_property(a, ky, store);
    b.ldar(i).inc(bin).star(i);
    b.jump_loop(header, 0, loop_);
    b.bind(exit);
    b.ldar(sum).return_();
    unit("", vec![s.finish(), b.finish().unwrap()])
}

/// The feedback slots of `f0` in [`wide_slots_program`]: load, store, compare, binary, loop.
pub(super) const WIDE_SLOTS: [u32; 5] = [302, 303, 0, 1, 304];

/// What one run of the cases of [`run_wide_cases`] gave: the results, the IC states of the
/// load and the store, the values of the other slots, the budget, the cold body runs (the
/// misses and the polls: `step_cold` and the `_w2` cold handlers), the stub cache probe hits
/// of the fast bodies and of all probes, and the `_w2` runs.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct WideRun {
    pub results: Vec<u64>,
    pub states: Vec<crate::ic::State>,
    pub values: Vec<u64>,
    pub budget: i64,
    pub cold: u64,
    pub probes: [u32; 2],
    pub w2: [u64; 2],
}

/// Runs `f0` of [`wide_slots_program`] on the cases mono (one shape), poly (3 shapes), mega (6
/// shapes, then the stub cache), a transition and a miss (a number receiver), in a new isolate
/// of `c` and `f` with bit 1 = `on`; `budget` is the budget before the calls.
pub(super) fn run_wide_cases(c: IsolateConfig, f: Feedback, on: bool, budget: i64) -> WideRun {
    use super::ic::{int, object_prototype, object_with};
    with_wide_table(on, move || {
        let mut c = c;
        c.feedback_threshold = 0;
        let mut iso = Isolate::new_with(c, ExecOptions::default().feedback(f)).unwrap();
        iso.with_cx_checkpoint(|cx| cx.run_script(wide_slots_program(), "w2.js").map(|_| ()))
            .unwrap();
        let fun = iso.global_raw("f0").unwrap();
        let mk = |iso: &Isolate, props: &[(&str, u64)]| {
            let o = object_with(&iso.rt, object_prototype(&iso.rt), props);
            (o, iso.rt.heap.new_global(val(o)))
        };
        let mut cases = vec![mk(&iso, &[("x", int(2)), ("y", int(0))])];
        for j in 0..6 {
            let pad = format!("p{j}");
            cases.push(mk(&iso, &[(pad.as_str(), int(j)), ("x", int(j + 1))]));
        }
        let before = (scaled_runs(), w2_runs());
        crate::ic::FAST_PROBE_HITS.with(|c| c.set(0));
        crate::ic::HANDLER_PROBE_HITS.with(|c| c.set(0));
        iso.cx.budget = budget;
        let mut results = Vec::new();
        let mut call = |iso: &mut Isolate, a: u64, n: i32| {
            let r = iso.call_raw(fun, Value::UNDEFINED, &[val(a), val(int(n))]);
            results.push(r.unwrap().to_bits());
        };
        // Mono: one shape (x and y exist: the store hits).
        call(&mut iso, cases[0].0, 50);
        // Poly: 3 shapes; each new one adds y (a transition), then hits.
        for _ in 0..2 {
            for &(o, _) in &cases[1..4] {
                call(&mut iso, o, 5);
            }
        }
        // Mega: 6 shapes and more; the loads probe the stub cache.
        for _ in 0..2 {
            for &(o, _) in &cases {
                call(&mut iso, o, 4);
            }
        }
        // A miss: a number receiver (no own x; the store does nothing).
        call(&mut iso, int(7), 3);
        let after = (scaled_runs(), w2_runs());
        let (states, values) = {
            let units = iso.rt.units.borrow();
            let v = feedback::vector(&units.last().unwrap().infos[1]).unwrap();
            (
                WIDE_SLOTS[..2]
                    .iter()
                    .map(|&s| crate::ic::state(v, s))
                    .collect(),
                WIDE_SLOTS[2..]
                    .iter()
                    .map(|&s| feedback::value(v, s))
                    .collect(),
            )
        };
        for (_, h) in cases {
            iso.rt.heap.drop_global(h);
        }
        let w2 = [after.1[0] - before.1[0], after.1[1] - before.1[1]];
        WideRun {
            results,
            states,
            values,
            budget: iso.cx.budget,
            cold: (after.0[1] - before.0[1]) + w2[1],
            probes: [
                crate::ic::FAST_PROBE_HITS.with(|c| c.get()),
                crate::ic::HANDLER_PROBE_HITS.with(|c| c.get()),
            ],
            w2,
        }
    })
}

/// T14: a function with more than 255 feedback slots gives the same results, IC states,
/// feedback, budget, cold body runs and probe hits in every expansion, both feedback modes,
/// with and without GC stress, with bit 1 off and on. With bit 1 on, the threaded expansions
/// run `SetNamedProperty_w2` (its entry is `_w2` on every target); with bit 1 off no `_w2`
/// handler runs.
#[test]
fn wide_forms_all_expansions_over_255_slots() {
    let code = &wide_slots_program().functions[1].bytecode;
    let prefixed: Vec<Opcode> = Instructions::new(code)
        .map(|i| i.unwrap())
        .filter(|i| i.scale == OperandScale::Wide)
        .map(|i| i.opcode)
        .collect();
    assert_eq!(
        prefixed,
        [
            Opcode::GetNamedProperty,
            Opcode::SetNamedProperty,
            Opcode::JumpLoop
        ]
    );
    // The first run of each GC stress value (a GC clears the stub cache, so the probe hits
    // differ with stress).
    let mut firsts: [Option<WideRun>; 2] = [None, None];
    for &d in Dispatch::ALL {
        for f in [Feedback::FULL, Feedback::ICS_ONLY] {
            for stress in [false, true] {
                for on in [false, true] {
                    let label = format!("{} {f:?} stress={stress} on={on}", d.name());
                    let mut r = on_big_stack(move || {
                        run_wide_cases(config(d, stress), f, on, crate::interp::BUDGET)
                    });
                    if !on || d == Dispatch::Match {
                        assert_eq!(r.w2, [0, 0], "{label}");
                    } else {
                        assert!(r.w2[0] > 0, "{label}: the _w2 runs {:?}", r.w2);
                    }
                    r.w2 = [0, 0];
                    // The feedback mode changes the records, not the results.
                    if f == Feedback::ICS_ONLY {
                        r.values.clear();
                    }
                    assert_eq!(r.results[0], super::ic::int(100), "{label}");
                    let first = &mut firsts[usize::from(stress)];
                    match first {
                        None if f == Feedback::FULL => *first = Some(r),
                        Some(r0) if f == Feedback::FULL => assert_eq!(&r, &*r0, "{label}"),
                        Some(r0) => {
                            assert_eq!(
                                (&r.results, r.budget, r.cold, r.probes),
                                (&r0.results, r0.budget, r0.cold, r0.probes),
                                "{label}"
                            );
                        }
                        None => unreachable!(),
                    }
                }
            }
        }
    }
}

/// T13b (check fix B1): where `prefix_runs_fast` is true (aarch64), a `GetNamedProperty.Wide`
/// miss and a `JumpLoop.Wide` poll run their fast body once (in the `Wide` fast body) and their
/// cold body once (`step_cold`), with bit 1 off and on: the wide table entry is the `Wide` cold
/// handler, not `_w2` (a `_w2` entry would run the fast body a second time: 2 budget units, 2
/// loop records, 2 `_w2` runs). Elsewhere the same counts come from the `_w2` handlers with
/// bit 1 on.
#[test]
fn aarch64_wide_fast_set_runs_once() {
    use super::ic::{int, object_prototype, object_with};
    for &d in Dispatch::ALL {
        let mut arms = Vec::new();
        for on in [false, true] {
            let label = format!("{} on={on}", d.name());
            let (budget, cold, w2, v) = on_big_stack(move || {
                with_wide_table(on, move || {
                    let mut c = config(d, false);
                    c.feedback_threshold = 0;
                    let mut iso = Isolate::new_with(c, ExecOptions::default()).unwrap();
                    iso.with_cx_checkpoint(|cx| {
                        cx.run_script(wide_slots_program(), "w2.js").map(|_| ())
                    })
                    .unwrap();
                    let fun = iso.global_raw("f0").unwrap();
                    let o = object_with(&iso.rt, object_prototype(&iso.rt), &[("x", int(3))]);
                    let h = iso.rt.heap.new_global(val(o));
                    // Three turns: the first load and store miss (uninitialized slots), the
                    // budget runs out at a back edge, which polls.
                    iso.cx.budget = 3;
                    let before = (scaled_runs(), w2_runs());
                    let r = iso.call_raw(fun, Value::UNDEFINED, &[val(o), val(int(3))]);
                    let after = (scaled_runs(), w2_runs());
                    iso.rt.heap.drop_global(h);
                    let budget = iso.cx.budget;
                    let v = {
                        let units = iso.rt.units.borrow();
                        let fv = feedback::vector(&units.last().unwrap().infos[1]).unwrap();
                        feedback::value(fv, WIDE_SLOTS[4])
                    };
                    assert_eq!(r.unwrap().to_bits(), int(9));
                    let w2 = [after.1[0] - before.1[0], after.1[1] - before.1[1]];
                    (budget, after.0[1] - before.0[1] + w2[1], w2, v)
                })
            });
            // The cold bodies: the 2 misses and at least the poll.
            assert!(cold >= 3, "{label}: the cold body runs {cold}");
            let threaded = d != Dispatch::Match;
            let want = match (
                on && threaded,
                prefix_runs_fast(Opcode::GetNamedProperty as u8),
            ) {
                (false, _) => [0, 0],
                // Only SetNamedProperty_w2: 3 fast runs (one per turn), its misses.
                (true, true) => [3, w2[1].max(1)],
                // All 3: 9 fast runs (3 per turn); the 2 misses and the poll.
                (true, false) => [9, cold],
            };
            assert_eq!(w2, want, "{label}: the _w2 runs");
            arms.push((budget, cold, v));
        }
        // The same budget units, cold body runs and loop record in both arms.
        assert_eq!(arms[0], arms[1], "{}", d.name());
    }
}
