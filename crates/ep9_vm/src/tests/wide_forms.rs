//! The WIDE-SET probe (ABI 8; DL M12-P2-WIDE-GRANTS): W-1, the `Far` jump twins.
//!
//! T5: each of the 11 twins is taken and not taken in every dispatch expansion and in both
//! feedback modes (`full` and `ics_only`: the two T0 handler tables), and gives what its base
//! gives at `Wide` scale. The 2 ToBoolean twins also run their cold body (an object and a
//! string accumulator).

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
