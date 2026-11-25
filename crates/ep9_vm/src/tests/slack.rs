//! Tests of slack (`slack.rs`) (owner M5-P2 SLACK; the H17 "Slack tracking" row): the scan
//! of a constructor's `this.<name> =` stores, the per-function memo of its result, and the
//! in-object capacity of the objects `new F` makes. Programs run under every dispatch expansion, with and without
//! GC stress, with eager feedback vectors (threshold 0: the construct site hits from its second
//! construct) and with none (`u32::MAX`: every construct takes the cold path).

use ep9_bytecode::{FeedbackSlotKind, FunctionBuilder, FunctionCode, FunctionId, VerifiedUnit};

use super::{Asm, config, isolate, meta, on_big_stack, take_output, unit};
use crate::frame::UNDEFINED;
use crate::kinds;
use crate::object::{self, cell, obj, rd};
use crate::runtime::{Atom, Root};
use crate::shape;
use crate::slack::{self, GENEROUS, MAX_NAMES};
use crate::{Dispatch, Isolate, IsolateConfig};

/// The scan result of `code`.
fn scan(code: &FunctionCode) -> Option<usize> {
    let mut names = [0u32; MAX_NAMES];
    slack::this_store_names(code.bytecode(), &mut names)
}

/// A constructor `name(a, b)` with no feedback slots of its own but the ones `body` asks for.
fn ctor(name: &str, body: impl FnOnce(&mut Asm)) -> FunctionCode {
    let mut a = Asm::new(meta(name, 2, 0, 0, 0));
    body(&mut a);
    a.b.lda_undefined().return_();
    a.finish()
}

/// `function P(x, y) { this.x = x; this.y = y; }`.
fn point(a: &mut Asm) {
    let this = a.b.receiver();
    a.b.ldar(a.b.parameter(0));
    a.set_named(this, "x");
    a.b.ldar(a.b.parameter(1));
    a.set_named(this, "y");
}

#[test]
fn the_scan_counts_distinct_this_stores() {
    assert_eq!(scan(&ctor("P", point)), Some(2));
    // `this.x = a; this.x = b; this.y = 1`: x counts once.
    let twice = ctor("P", |a| {
        let this = a.b.receiver();
        a.b.ldar(a.b.parameter(0));
        a.set_named(this, "x");
        a.b.ldar(a.b.parameter(1));
        a.set_named(this, "x");
        a.b.lda_smi(1);
        a.set_named(this, "y");
    });
    assert_eq!(scan(&twice), Some(2));
    // `var t = this; t.x = a`: followed through a register.
    let through = ctor("P", |a| {
        let (this, t) = (a.b.receiver(), a.b.new_register());
        a.b.mov(this, t);
        a.b.ldar(a.b.parameter(0));
        a.set_named(t, "x");
    });
    assert_eq!(scan(&through), Some(1));
    // `a.x = 1`: a store on another object does not count.
    let other = ctor("P", |a| {
        let p = a.b.parameter(0);
        a.b.lda_smi(1);
        a.set_named(p, "x");
    });
    assert_eq!(scan(&other), Some(0));
    // `this.x` read, then stored: a read keeps `this` in the frame.
    let read = ctor("P", |a| {
        let this = a.b.receiver();
        a.get_named(this, "x");
        a.set_named(this, "y");
    });
    assert_eq!(scan(&read), Some(1));
}

#[test]
fn the_scan_gives_none_when_this_leaves_the_frame() {
    // `this.self = this`: the stored value is `this`.
    let self_store = ctor("E", |a| {
        let this = a.b.receiver();
        a.b.ldar(this);
        a.set_named(this, "self");
    });
    assert_eq!(scan(&self_store), None);
    // `this.a = 1; g(this)`: an argument.
    let call = ctor("E", |a| {
        let this = a.b.receiver();
        a.b.lda_smi(1);
        a.set_named(this, "a");
        let (f, a0) = (a.f, a.a.get(0));
        a.b.mov(this, a0);
        a.lda_global("g");
        a.b.star(f);
        a.b.call_undefined_receiver1(f, a0, a.call);
    });
    assert_eq!(scan(&call), None);
    // `this.m()`: the receiver of a call.
    let method = ctor("E", |a| {
        let this = a.b.receiver();
        let f = a.f;
        a.get_named(this, "m");
        a.b.star(f);
        a.b.call_property0(f, this, a.call);
    });
    assert_eq!(scan(&method), None);
    // `this[k] = 1`: a keyed store.
    let keyed = ctor("E", |a| {
        let this = a.b.receiver();
        let k = a.b.parameter(0);
        a.b.lda_smi(1);
        a.b.set_keyed_property(this, k, a.set_keyed);
    });
    assert_eq!(scan(&keyed), None);
    // A register that held `this` and then got another value no longer counts: `t = this;
    // t = a; g(t)`.
    let overwritten = ctor("P", |a| {
        let (this, t) = (a.b.receiver(), a.b.new_register());
        a.b.mov(this, t);
        a.b.ldar(a.b.parameter(0)).star(t);
        let f = a.f;
        a.lda_global("g");
        a.b.star(f);
        a.b.call_undefined_receiver1(f, t, a.call);
        a.b.lda_smi(1);
        a.set_named(this, "x");
    });
    assert_eq!(scan(&overwritten), Some(1));
}

#[test]
fn more_names_than_the_scan_keeps_give_none() {
    let many = |n: usize| {
        ctor("M", move |a| {
            let this = a.b.receiver();
            for i in 0..n {
                a.b.lda_smi(i as i32);
                a.set_named(this, &format!("f{i}"));
            }
        })
    };
    assert_eq!(scan(&many(MAX_NAMES)), Some(MAX_NAMES));
    assert_eq!(scan(&many(MAX_NAMES + 1)), None);
}

/// What a program left in the globals `o0` (the first instance) and `o` (the last).
struct Seen {
    /// In-object capacity and property count of `o0` and `o`.
    first: (usize, usize),
    last: (usize, usize),
    /// `o0` and `o` have one shape.
    same_shape: bool,
    /// `o0` and `o` have a SLOTS cell (out-of-line fields).
    slots: (bool, bool),
}

/// Runs `make()` in every dispatch expansion, with and without GC stress, with eager
/// vectors and with none; checks the output and returns what each run saw. The CTOR-FIT
/// probe arm is 0 (today's count; `slack::slack_arm`).
fn run_each(make: fn() -> VerifiedUnit, want: &'static [&'static str]) -> Vec<(String, Seen)> {
    run_each_arm(0, make, want)
}

/// [`run_each`] with the CTOR-FIT probe arm `arm` (`EP9_SLACK_ARM`) on each run thread.
fn run_each_arm(
    arm: u8,
    make: fn() -> VerifiedUnit,
    want: &'static [&'static str],
) -> Vec<(String, Seen)> {
    let mut seen = Vec::new();
    for &d in Dispatch::ALL {
        for stress in [false, true] {
            for threshold in [0, u32::MAX] {
                let c = IsolateConfig {
                    feedback_threshold: threshold,
                    ..config(d, stress)
                };
                let label = format!("{} stress={stress} threshold={threshold}", d.name());
                let l = label.clone();
                let s = on_big_stack(move || {
                    slack::slack_arm_override::ARM.set(Some(arm));
                    let mut iso = isolate(c);
                    take_output();
                    let r =
                        iso.with_cx_checkpoint(|cx| cx.run_script(make(), "slack.js").map(|_| ()));
                    let out = take_output();
                    if let Err(e) = r {
                        panic!("{l}: uncaught {e}");
                    }
                    assert_eq!(out, want, "{l}");
                    let s = observe(&iso);
                    iso.collect_garbage();
                    iso.verify_heap().unwrap();
                    s
                });
                seen.push((label, s));
            }
        }
    }
    seen
}

fn observe(iso: &Isolate) -> Seen {
    let read = |name: &str| {
        let o = iso.global_raw(name).unwrap().to_bits();
        assert!(kinds::is_object(o), "{name} is an object");
        let s = shape::shape_of(o);
        let slots = rd(cell(o), obj::SLOTS) != UNDEFINED;
        (s, (shape::field_layout(s).0, shape::count(s)), slots)
    };
    let (s0, first, slots0) = read("o0");
    let (s1, last, slots1) = read("o");
    Seen {
        first,
        last,
        same_shape: s0 == s1,
        slots: (slots0, slots1),
    }
}

/// The script: `P = f1`, then `prelude(s, P)`, then 10 times `o = new P(1, 2)` (`o0` = the
/// first), then `after(s, o)`, and `print(o.x + o.y)`.
fn script(
    p: FunctionCode,
    prelude: impl FnOnce(&mut Asm, ep9_bytecode::Register),
    after: impl FnOnce(&mut Asm, ep9_bytecode::Register),
) -> VerifiedUnit {
    let mut s = Asm::script(0);
    let k = s.b.function_constant(FunctionId(1));
    s.b.create_closure(k, s.closure, 0);
    s.sta_global("P");
    let (ctor_reg, o) = (s.b.new_register(), s.b.new_register());
    s.lda_global("P");
    s.b.star(ctor_reg);
    prelude(&mut s, ctor_reg);
    let site = s.b.feedback_slot(FeedbackSlotKind::Call);
    let args = s.b.new_register_list(2);
    let i = s.b.new_register();
    s.b.lda_zero().star(i);
    let head = s.b.bound_label();
    let exit = s.b.new_label();
    s.b.lda_smi(10).test_less_than(i, s.cmp).jump_if_false(exit);
    s.b.lda_smi(1)
        .star(args.get(0))
        .lda_smi(2)
        .star(args.get(1));
    s.lda_global("P");
    s.b.star(ctor_reg);
    s.b.construct(ctor_reg, args, 2, site).star(o);
    s.sta_global("o");
    // `if (i == 0) o0 = o`
    let skip = s.b.new_label();
    s.b.lda_zero()
        .test_equal_strict(i, s.cmp)
        .jump_if_false(skip);
    s.b.ldar(o);
    s.sta_global("o0");
    s.b.bind(skip);
    s.b.ldar(i).inc(s.bin).star(i).jump_loop(head, 0, s.loop_);
    s.b.bind(exit);
    after(&mut s, o);
    let t = s.b.new_register();
    s.get_named(o, "x");
    s.b.star(t);
    s.get_named(o, "y");
    s.b.add(t, s.bin);
    s.print_acc();
    s.b.lda_undefined().return_();
    unit("", vec![s.finish(), p])
}

fn plain_point() -> VerifiedUnit {
    script(ctor("P", point), |_, _| {}, |_, _| {})
}

#[test]
fn new_f_gets_the_store_count_and_one_shape() {
    for (label, s) in run_each(plain_point, &["3"]) {
        // Two stores: two in-object slots (8 before slack), for the first instance too.
        assert_eq!(s.first, (2, 2), "{label}");
        assert_eq!(s.last, (2, 2), "{label}");
        // One shape family: the cold first construct and the hits make the same shape.
        assert!(s.same_shape, "{label}");
    }
}

/// `P.prototype.left = null; P.prototype.x = 0; P.prototype.m = P` before the constructs.
fn point_with_prototype_fields() -> VerifiedUnit {
    script(
        ctor("P", point),
        |s, p| {
            let proto = s.b.new_register();
            s.get_named(p, "prototype");
            s.b.star(proto);
            s.b.lda_null();
            s.set_named(proto, "left");
            s.b.lda_zero();
            s.set_named(proto, "x");
            s.b.ldar(p);
            s.set_named(proto, "m");
        },
        |_, _| {},
    )
}

#[test]
fn prototype_data_fields_add_room_for_their_shadows() {
    for (label, s) in run_each(point_with_prototype_fields, &["3"]) {
        // x, y from the stores; `left` (a data value on the prototype); not `x` again, not
        // `m` and `constructor` (callable).
        assert_eq!(s.first.0, 3, "{label}");
        assert_eq!(s.last, (3, 2), "{label}");
        assert!(s.same_shape, "{label}");
    }
}

/// `function P(x, y) { this.x = x; this.y = y; this.self = this; }`: `this` escapes.
fn escaping_point() -> VerifiedUnit {
    let p = ctor("P", |a| {
        point(a);
        let this = a.b.receiver();
        a.b.ldar(this);
        a.set_named(this, "self");
    });
    script(p, |_, _| {}, |_, _| {})
}

#[test]
fn an_escaping_this_keeps_the_generous_capacity() {
    for (label, s) in run_each(escaping_point, &["3"]) {
        assert_eq!(s.last, (usize::from(GENEROUS), 3), "{label}");
        assert!(s.same_shape, "{label}");
    }
}

/// After the constructs: `o.z = 4; print(o.z)` (a field past the capacity, then a load of
/// it).
fn point_with_a_later_field() -> VerifiedUnit {
    script(
        ctor("P", point),
        |_, _| {},
        |s, o| {
            s.b.lda_smi(4);
            s.set_named(o, "z");
            s.get_named(o, "z");
            s.print_acc();
        },
    )
}

#[test]
fn a_field_added_after_the_constructor_goes_out_of_line() {
    for (label, s) in run_each(point_with_a_later_field, &["4", "3"]) {
        // `o` has 3 properties and keeps its 2 slots (a live object never grows or shrinks
        // in place: H21 P5); the heap check after the run checks every size.
        assert_eq!(s.last, (2, 3), "{label}");
        assert_eq!(s.first, (2, 2), "{label}");
        assert!(!s.same_shape, "{label}");
    }
}

#[test]
fn the_capacity_reads_no_feedback_policy() {
    // Rule 40 / M-R9: with eager vectors (hits) and with none (all cold), every dispatch
    // and GC mode makes the same capacity.
    let seen = run_each(point_with_prototype_fields, &["3"]);
    let (l0, s0) = &seen[0];
    for (label, s) in &seen[1..] {
        assert_eq!((s.first, s.last), (s0.first, s0.last), "{label} vs {l0}");
    }
}

/// A constructor with no store scans to 0 (`instance_capacity` then gives `None`: the
/// generous capacity).
#[test]
fn a_constructor_with_no_store_scans_to_zero() {
    let mut b = FunctionBuilder::new(meta("Z", 0, 0, 0, 0));
    b.lda_undefined().return_();
    assert_eq!(scan(&b.finish().unwrap()), Some(0));
}

/// A constructor of `n` stores with no vector, run 10 times cold in a new thread: the scan
/// runs once (the FunctionInfo keeps the capacity).
#[test]
fn the_capacity_is_chosen_once_per_function() {
    on_big_stack(|| {
        let c = IsolateConfig {
            feedback_threshold: u32::MAX,
            ..config(Dispatch::ALL[0], false)
        };
        let mut iso = isolate(c);
        take_output();
        iso.with_cx_checkpoint(|cx| cx.run_script(plain_point(), "slack.js").map(|_| ()))
            .unwrap();
        assert_eq!(take_output(), ["3"]);
        assert_eq!(slack::SCANS.with(|c| c.get()), 1);
    });
}

/// Like [`script`] with no prelude, with `P.prototype.z = 0` after each construct: the
/// first construct runs before the prototype has `z`, the later ones after.
fn script_proto_grows(p: FunctionCode) -> VerifiedUnit {
    let mut s = Asm::script(0);
    let k = s.b.function_constant(FunctionId(1));
    s.b.create_closure(k, s.closure, 0);
    s.sta_global("P");
    let (ctor_reg, o, proto) = (s.b.new_register(), s.b.new_register(), s.b.new_register());
    let site = s.b.feedback_slot(FeedbackSlotKind::Call);
    let args = s.b.new_register_list(2);
    let i = s.b.new_register();
    s.b.lda_zero().star(i);
    let head = s.b.bound_label();
    let exit = s.b.new_label();
    s.b.lda_smi(10).test_less_than(i, s.cmp).jump_if_false(exit);
    s.b.lda_smi(1)
        .star(args.get(0))
        .lda_smi(2)
        .star(args.get(1));
    s.lda_global("P");
    s.b.star(ctor_reg);
    s.b.construct(ctor_reg, args, 2, site).star(o);
    s.sta_global("o");
    let skip = s.b.new_label();
    s.b.lda_zero()
        .test_equal_strict(i, s.cmp)
        .jump_if_false(skip);
    s.b.ldar(o);
    s.sta_global("o0");
    s.b.bind(skip);
    s.get_named(ctor_reg, "prototype");
    s.b.star(proto);
    s.b.lda_zero();
    s.set_named(proto, "z");
    s.b.ldar(i).inc(s.bin).star(i).jump_loop(head, 0, s.loop_);
    s.b.bind(exit);
    let t = s.b.new_register();
    s.get_named(o, "x");
    s.b.star(t);
    s.get_named(o, "y");
    s.b.add(t, s.bin);
    s.print_acc();
    s.b.lda_undefined().return_();
    unit("", vec![s.finish(), p])
}

#[test]
fn a_prototype_field_added_after_the_first_construct_changes_no_capacity() {
    // Rule 40 / M-R9: the cold path (no vector) and the construct-site hit (eager vector)
    // give the same capacity: the one chosen at the first construct (x, y; no `z` yet).
    let seen = run_each(|| script_proto_grows(ctor("P", point)), &["3"]);
    for (label, s) in &seen {
        assert_eq!(s.first, (2, 2), "{label}");
        assert_eq!(s.last, (2, 2), "{label}");
        assert!(s.same_shape, "{label}");
    }
}

/// A constructor of 0 parameters whose code is `LdaZero; this.<names[k]> = acc;
/// LdaZero x3; LdaSmi i1; LdaZero x2; LdaSmi i2; LdaZero x2; Return`, with the string
/// constants `consts` (slot 0 the store's).
fn colliding_ctor(name: &str, consts: [&str; 2], k: usize, i1: i32, i2: i32) -> FunctionCode {
    let mut b = FunctionBuilder::new(meta(name, 0, 0, 0, 0));
    let slot = b.feedback_slot(FeedbackSlotKind::StoreNamed);
    let ks = consts.map(|c| b.string_constant(c));
    let this = b.receiver();
    b.lda_zero().set_named_property(this, ks[k], slot);
    b.lda_zero().lda_zero().lda_zero().lda_smi(i1);
    b.lda_zero().lda_zero().lda_smi(i2);
    b.lda_zero().lda_zero().return_();
    b.finish().unwrap()
}

/// `A = f1; B = f2; B.prototype.x = null; new A(); o0 = o = new B(); print(1)`.
fn colliding_constructors() -> VerifiedUnit {
    // Two codes of equal length that differ in the stored name (constant 1 in A, 0 in B)
    // and in the LdaSmi immediates, chosen so the two have one 64-bit FNV-1a hash (the
    // M5-P2 SLACK scan cache key, length and hash: Codex round 1 finding, slack 1).
    let a = colliding_ctor("A", ["q", "y"], 1, 0x1234_5678, 0x7654_3210);
    let b = colliding_ctor(
        "B",
        ["x", "y"],
        0,
        0xf873_f47f_u32 as i32,
        0x80d5_dba7_u32 as i32,
    );
    let hex = |c: &FunctionCode| {
        c.bytecode()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    assert_eq!(
        hex(&a),
        "0324ff01000303030204785634120303020410325476030392"
    );
    assert_eq!(
        hex(&b),
        "0324ff000003030302047ff473f803030204a7dbd580030392"
    );
    let mut s = Asm::script(0);
    for (i, g) in [(1, "A"), (2, "B")] {
        let k = s.b.function_constant(FunctionId(i));
        s.b.create_closure(k, s.closure, 0);
        s.sta_global(g);
    }
    let (f, proto) = (s.b.new_register(), s.b.new_register());
    s.lda_global("B");
    s.b.star(f);
    s.get_named(f, "prototype");
    s.b.star(proto);
    s.b.lda_null();
    s.set_named(proto, "x");
    let args = s.b.new_register_list(0);
    let site = s.b.feedback_slot(FeedbackSlotKind::Call);
    s.lda_global("A");
    s.b.star(f);
    s.b.construct(f, args, 0, site);
    s.lda_global("B");
    s.b.star(f);
    s.b.construct(f, args, 0, site);
    s.sta_global("o");
    s.sta_global("o0");
    s.b.lda_smi(1);
    s.print_acc();
    s.b.lda_undefined().return_();
    unit("", vec![s.finish(), a, b])
}

#[test]
fn a_constructor_never_reads_another_constructors_scan() {
    // B stores `x`, which B.prototype has as a data field: one slot (x counts once). A
    // capacity from A's names (constant 1 = "y", not on the prototype) would be 2.
    for (label, s) in run_each(colliding_constructors, &["1"]) {
        assert_eq!(s.last, (1, 1), "{label}");
    }
}

// ---- the fused named load (STAR-DST probe, ABI 7; SC b6) ---------------------------------------

/// The names the scan gives for `code` (the list, not only the count).
fn scan_names(code: &FunctionCode) -> Option<Vec<u32>> {
    let mut names = [0u32; MAX_NAMES];
    slack::this_store_names(code.bytecode(), &mut names).map(|n| names[..n].to_vec())
}

/// `GetNamedPropertyStar obj, name, slot, dst` is the pair `GetNamedProperty obj, name, slot`
/// then `Star dst` for the scan: `this.a.b; this.x = 1` and the forms where `dst` holds `this`
/// before the load (the store through it then is not a `this` store) give the same names for
/// the pair unit and the fused unit.
#[test]
fn a_fused_named_load_scans_as_its_pair() {
    // `fused`: the load and its `Star` as one instruction.
    fn load_star(
        a: &mut Asm,
        fused: bool,
        obj: ep9_bytecode::Register,
        name: &str,
        dst: ep9_bytecode::Register,
    ) {
        let k = a.b.string_constant(name);
        if fused {
            a.b.get_named_property_star(obj, k, a.named, dst);
        } else {
            a.b.get_named_property(obj, k, a.named).star(dst);
        }
    }
    for fused in [false, true] {
        // `this.a.b; this.x = 1`.
        let chain = ctor("P", |a| {
            let (this, t) = (a.b.receiver(), a.b.new_register());
            load_star(a, fused, this, "a", t);
            a.get_named(t, "b");
            a.b.lda_smi(1);
            a.set_named(this, "x");
        });
        // `var t = this; t = this.a; t.y = 1; this.x = 1`: the load clears `t`.
        let cleared = ctor("P", |a| {
            let (this, t) = (a.b.receiver(), a.b.new_register());
            a.b.mov(this, t);
            load_star(a, fused, this, "a", t);
            a.b.lda_smi(1);
            a.set_named(t, "y");
            a.set_named(this, "x");
        });
        // `var t = this; t = t.a; this.x = t`: dst is the object register.
        let same = ctor("P", |a| {
            let (this, t) = (a.b.receiver(), a.b.new_register());
            a.b.mov(this, t);
            load_star(a, fused, t, "a", t);
            a.b.ldar(t);
            a.set_named(this, "x");
        });
        // `this.a` into a parameter that holds `this`: a1 = this; a1 = this.a; a1.y = 1.
        let param = ctor("P", |a| {
            let (this, p) = (a.b.receiver(), a.b.parameter(1));
            a.b.mov(this, p);
            load_star(a, fused, this, "a", p);
            a.b.lda_smi(1);
            a.set_named(p, "y");
        });
        let got: Vec<Option<Vec<u32>>> = [&chain, &cleared, &same, &param]
            .iter()
            .map(|c| scan_names(c))
            .collect();
        // The constant indices: "a" k0 (and "b" k1 in `chain`), then the stored names.
        assert_eq!(
            got,
            [Some(vec![2]), Some(vec![2]), Some(vec![1]), Some(vec![])],
            "fused {fused}"
        );
    }
}

// ---- CTOR-FIT probe (f-design L1; `EP9_SLACK_ARM`, probe branch only) -----------------------

#[test]
fn the_slack_arm_parse() {
    assert_eq!(slack::slack_arm_from(None), 0);
    for (v, arm) in [("0", 0), ("1", 1), ("2", 2), ("3", 3)] {
        assert_eq!(slack::slack_arm_from(Some(v)), arm, "{v}");
    }
    // Every other value is arm 0 (today's count).
    for v in ["", "4", " 1", "1 ", "01", "on", "true", "-1"] {
        assert_eq!(slack::slack_arm_from(Some(v)), 0, "{v:?}");
    }
    // Arm 1 = ESCAPE_SLACK 1, arm 2 = 0, arm 3 = 2; arm 0 reads no chain.
    assert_eq!(slack::escape_slack(0), None);
    assert_eq!(slack::escape_slack(1), Some(1));
    assert_eq!(slack::escape_slack(2), Some(0));
    assert_eq!(slack::escape_slack(3), Some(2));
    assert_eq!(slack::escape_slack(4), None);
    // The per-thread arm of the tests wins over the process value.
    on_big_stack(|| {
        for arm in 0..=3 {
            slack::slack_arm_override::ARM.set(Some(arm));
            assert_eq!(slack::slack_arm(), arm);
        }
    });
}

/// The escape-mode scan of `code`: (names, escaped).
fn scan_escape(code: &FunctionCode) -> Option<(Vec<u32>, bool)> {
    let mut names = [0u32; MAX_NAMES];
    slack::scan_this_stores::<true>(code.bytecode(), &mut names)
        .map(|(n, e)| (names[..n].to_vec(), e))
}

/// `_super.call(this, a, ..)` (the ES5 class form that TypeScript emits) with `extra` more
/// arguments past `a`: `CallProperty2` (a `Reg` operand holds `this`) for 0, `CallProperty`
/// (a register list holds `this`) for 1 or more.
fn super_call(a: &mut Asm, extra: u32) {
    let this = a.b.receiver();
    let (sup, f) = (a.b.new_register(), a.b.new_register());
    a.lda_global("_super");
    a.b.star(sup);
    a.get_named(sup, "call");
    a.b.star(f);
    if extra == 0 {
        a.b.call_property2(f, sup, this, a.b.parameter(0), a.call);
    } else {
        let args = a.b.new_register_list(3 + extra);
        a.b.mov(sup, args.get(0));
        a.b.mov(this, args.get(1));
        for i in 0..=extra {
            a.b.ldar(a.b.parameter(0)).star(args.get(2 + i));
        }
        a.b.call_property(f, args, 3 + extra, a.call);
    }
}

/// `this.<name> = a` for each name.
fn stores(a: &mut Asm, names: &[&str]) {
    let this = a.b.receiver();
    for n in names {
        a.b.ldar(a.b.parameter(0));
        a.set_named(this, n);
    }
}

#[test]
fn the_escape_mode_goes_on_past_a_leave_of_this() {
    for extra in [0, 1, 2] {
        // `_super.call(this, a, ..); this.x = a; this.y = a`.
        let d = ctor("D", |a| {
            super_call(a, extra);
            stores(a, &["x", "y"]);
        });
        // Today's scan: no count. The escape mode: both names, `escaped`.
        assert_eq!(scan(&d), None, "extra {extra}");
        let (names, escaped) = scan_escape(&d).unwrap();
        assert_eq!(names.len(), 2, "extra {extra}");
        assert!(escaped, "extra {extra}");
    }
    // `this.self = this; this.x = a`: the stored value is `this`.
    let self_store = ctor("E", |a| {
        let this = a.b.receiver();
        a.b.ldar(this);
        a.set_named(this, "self");
        stores(a, &["x"]);
    });
    assert_eq!(scan(&self_store), None);
    assert_eq!(scan_escape(&self_store).unwrap().0.len(), 2);
    // No leave: the escape mode gives today's names and no `escaped`.
    let p = ctor("P", point);
    assert_eq!(scan_escape(&p), Some((scan_names(&p).unwrap(), false)));
}

/// The functions of the chain tests: f1 `D` (`_super.call(this, a); this.d0 = a; this.d1 =
/// a`), f2 `B` (`this.b0 = a .. this.b6 = a`), f3 `E` (no store), f4 `K` (a DERIVED class
/// constructor), each a global; `_super = B`.
fn chain_functions() -> VerifiedUnit {
    let d = ctor("D", |a| {
        super_call(a, 0);
        stores(a, &["d0", "d1"]);
    });
    let b = ctor("B", |a| {
        stores(a, &["b0", "b1", "b2", "b3", "b4", "b5", "b6"])
    });
    let e = ctor("E", |_| {});
    let mut m = meta("K", 0, 0, 0, 0);
    m.kind = ep9_bytecode::FunctionKind::DerivedConstructor;
    m.flags = ep9_bytecode::FunctionFlags::STRICT | ep9_bytecode::FunctionFlags::SIMPLE_PARAMETERS;
    let mut k = Asm::new(m);
    k.b.lda_undefined().return_();
    let k = k.finish();
    let mut s = Asm::script(0);
    for (i, g) in [(1, "D"), (2, "B"), (3, "E"), (4, "K")] {
        let c = s.b.function_constant(FunctionId(i));
        s.b.create_closure(c, s.closure, 0);
        s.sta_global(g);
    }
    s.lda_global("B");
    s.sta_global("_super");
    s.b.lda_undefined().return_();
    unit("", vec![s.finish(), d, b, e, k])
}

/// How a prototype object of a chain test is made.
#[derive(Clone, Copy)]
enum Level {
    /// Own data property `constructor` = the global function of this name.
    Ctor(&'static str),
    /// `constructor` = the global function, then the object goes to dictionary mode.
    Dictionary(&'static str),
    /// `constructor` is an accessor (its getter: the global function).
    Accessor(&'static str),
    /// No own `constructor`.
    Bare,
}

/// Makes the prototype chain `levels` (the first is `F.prototype`, the last has
/// %Object.prototype% as its prototype) and returns the chain count of `D` on it:
/// (names, escaped, levels read, capacity at ESCAPE_SLACK 0 / 1 / 2).
fn chain_of(levels: &'static [Level]) -> (usize, bool, u32, [usize; 3]) {
    on_big_stack(move || {
        let mut iso = isolate(config(Dispatch::ALL[0], false));
        iso.run_raw(chain_functions(), "chain.js").unwrap();
        let rt = &*iso.rt;
        let global = |n: &str| iso.global_raw(n).unwrap().to_bits();
        let ctor_key = rt.atom(Atom::Constructor);
        let mut proto = rt.root(Root::ObjectPrototype);
        for l in levels.iter().rev() {
            let p = object::new_object(rt, proto);
            match *l {
                Level::Ctor(f) => {
                    object::own_define(rt, p, ctor_key, global(f), object::attr::HIDDEN)
                }
                Level::Dictionary(f) => {
                    object::own_define(rt, p, ctor_key, global(f), object::attr::HIDDEN);
                    crate::props::to_dictionary(rt, p);
                    assert!(shape::is_dictionary(shape::shape_of(p)));
                }
                Level::Accessor(f) => {
                    let pair = crate::accessor::new_pair(rt, global(f), UNDEFINED);
                    let at = object::attr::ACCESSOR | object::attr::CONFIGURABLE;
                    object::own_define(rt, p, ctor_key, pair, at);
                }
                Level::Bare => {}
            }
            proto = p;
        }
        let fi = object::function_info(global("D"));
        // SAFETY: `D` is a bytecode function: its code lives while the isolate does.
        let code = unsafe { core::slice::from_raw_parts(fi.code, fi.code_len as usize) };
        let ch = slack::chain_names(rt, fi, code, proto).unwrap();
        let cap = [0, 1, 2].map(|k| slack::chain_capacity(rt, fi, code, proto, k).unwrap());
        (ch.keys().len(), ch.escaped, ch.levels, cap)
    })
}

#[test]
fn the_chain_adds_the_names_of_each_constructor_level() {
    use Level::*;
    // D.prototype -> B.prototype -> %Object.prototype% (constructor Object: a host function).
    let (n, escaped, levels, cap) = chain_of(&[Ctor("D"), Ctor("B")]);
    assert_eq!((n, escaped, levels), (9, true, 2));
    // d0, d1, b0..b6 = 9; + ESCAPE_SLACK when `this` left the frame.
    assert_eq!(cap, [9, 10, 11]);
    // A level whose constructor has no store adds no name and the walk goes on.
    assert_eq!(chain_of(&[Ctor("D"), Ctor("E"), Ctor("B")]).0, 9);
    // D.prototype with no own `constructor`: only D's own names.
    assert_eq!(chain_of(&[Bare, Ctor("B")]).0, 2);
}

#[test]
fn the_chain_stops_at_each_stop_rule() {
    use Level::*;
    // A host function (`print`) as `constructor`: B is not read.
    let (n, _, levels, _) = chain_of(&[Ctor("D"), Ctor("print"), Ctor("B")]);
    assert_eq!((n, levels), (2, 1));
    // A dictionary-mode prototype: its constructor B is not read.
    let (n, _, levels, _) = chain_of(&[Ctor("D"), Dictionary("B")]);
    assert_eq!((n, levels), (2, 1));
    // An accessor `constructor` (its getter is not called).
    let (n, _, levels, _) = chain_of(&[Ctor("D"), Accessor("B")]);
    assert_eq!((n, levels), (2, 1));
    // A DERIVED class constructor: no count (its `this` comes from the super call).
    let (n, _, levels, _) = chain_of(&[Ctor("D"), Ctor("K"), Ctor("B")]);
    assert_eq!((n, levels), (2, 1));
}

#[test]
fn the_chain_reads_at_most_8_levels() {
    use Level::*;
    // B at level 8: read.
    let (n, _, levels, _) = chain_of(&[
        Ctor("D"),
        Ctor("E"),
        Ctor("E"),
        Ctor("E"),
        Ctor("E"),
        Ctor("E"),
        Ctor("E"),
        Ctor("B"),
    ]);
    assert_eq!((n, levels), (9, 8));
    // B at level 9: not read.
    let (n, _, levels, _) = chain_of(&[
        Ctor("D"),
        Ctor("E"),
        Ctor("E"),
        Ctor("E"),
        Ctor("E"),
        Ctor("E"),
        Ctor("E"),
        Ctor("E"),
        Ctor("B"),
    ]);
    assert_eq!((n, levels), (2, 8));
    assert_eq!(slack::CHAIN_LEVELS, 8);
}

/// An ES5 class chain as TypeScript emits it, with `base` names in the base constructor `B`
/// and `own` names in `D` (`_super.call(this, a); this.d<i> = a`). The VM tests have no
/// built-ins, so `B.call` is a bytecode function `callB(t, a)` that calls B with receiver `t`,
/// and `__extends` is the TypeScript form `__.prototype = B.prototype; D.prototype = new __()`
/// with `function __() { this.constructor = D; }`. Then 10 times `o = new D(1, 2)` (`o0` =
/// the first), and `print(o.d0)`.
fn es5_chain(base: &'static [&'static str], own: &'static [&'static str]) -> VerifiedUnit {
    let d = ctor("D", |a| {
        super_call(a, 0);
        stores(a, own);
    });
    let b = ctor("B", |a| stores(a, base));
    // `callB(t, a)`: B with receiver `t` and argument `a`.
    let call_b = ctor("callB", |a| {
        let bf = a.b.new_register();
        a.lda_global("B");
        a.b.star(bf);
        a.b.call_property1(bf, a.b.parameter(0), a.b.parameter(1), a.call);
    });
    // `function __() { this.constructor = D; }`.
    let ext = ctor("__", |a| {
        let this = a.b.receiver();
        a.lda_global("D");
        a.set_named(this, "constructor");
    });
    let mut s = Asm::script(0);
    for (i, g) in [(1, "D"), (2, "B"), (3, "callB"), (4, "__")] {
        let c = s.b.function_constant(FunctionId(i));
        s.b.create_closure(c, s.closure, 0);
        s.sta_global(g);
    }
    let (rd, rb, rx, rp, none) = (
        s.b.new_register(),
        s.b.new_register(),
        s.b.new_register(),
        s.b.new_register(),
        s.b.new_register_list(0),
    );
    // `_super = B; B.call = callB`.
    s.lda_global("B");
    s.sta_global("_super");
    s.b.star(rb);
    s.lda_global("callB");
    s.set_named(rb, "call");
    // `__.prototype = B.prototype; D.prototype = new __()`.
    s.lda_global("__");
    s.b.star(rx);
    s.get_named(rb, "prototype");
    s.set_named(rx, "prototype");
    s.b.construct(rx, none, 0, s.call).star(rp);
    s.lda_global("D");
    s.b.star(rd);
    s.b.ldar(rp);
    s.set_named(rd, "prototype");
    let site = s.b.feedback_slot(FeedbackSlotKind::Call);
    let args = s.b.new_register_list(2);
    let (i, o) = (s.b.new_register(), s.b.new_register());
    s.b.lda_zero().star(i);
    let head = s.b.bound_label();
    let exit = s.b.new_label();
    s.b.lda_smi(10).test_less_than(i, s.cmp).jump_if_false(exit);
    s.b.lda_smi(1)
        .star(args.get(0))
        .lda_smi(2)
        .star(args.get(1));
    s.lda_global("D");
    s.b.star(rd);
    s.b.construct(rd, args, 2, site).star(o);
    s.sta_global("o");
    let skip = s.b.new_label();
    s.b.lda_zero()
        .test_equal_strict(i, s.cmp)
        .jump_if_false(skip);
    s.b.ldar(o);
    s.sta_global("o0");
    s.b.bind(skip);
    s.b.ldar(i).inc(s.bin).star(i).jump_loop(head, 0, s.loop_);
    s.b.bind(exit);
    s.get_named(o, "d0");
    s.print_acc();
    s.b.lda_undefined().return_();
    unit("", vec![s.finish(), d, b, call_b, ext])
}

/// 7 base names + 3 own names: n_chain = 10 (above GENEROUS).
fn es5_chain_10() -> VerifiedUnit {
    es5_chain(
        &["b0", "b1", "b2", "b3", "b4", "b5", "b6"],
        &["d0", "d1", "d2"],
    )
}

/// 3 base names + 2 own names: n_chain = 5 (+ slack; at most GENEROUS).
fn es5_chain_5() -> VerifiedUnit {
    es5_chain(&["b0", "b1", "b2"], &["d0", "d1"])
}

#[test]
fn an_es5_class_chain_gets_its_chain_count_in_object() {
    // Arm 0 (today): `this` leaves in the super call, so no count: GENEROUS slots, and
    // fields 9 and 10 in a SLOTS cell.
    for (label, s) in run_each_arm(0, es5_chain_10, &["1"]) {
        assert_eq!(s.first, (usize::from(GENEROUS), 10), "arm 0 {label}");
        assert_eq!(s.last, (usize::from(GENEROUS), 10), "arm 0 {label}");
        assert_eq!(s.slots, (true, true), "arm 0 {label}");
        assert!(s.same_shape, "arm 0 {label}");
    }
    // Arms 1, 2, 3 (ESCAPE_SLACK 1, 0, 2): n_chain = 10 + slack in-object slots and no
    // SLOTS cell, for the first instance too; one shape family.
    for (arm, cap) in [(1, 11), (2, 10), (3, 12)] {
        for (label, s) in run_each_arm(arm, es5_chain_10, &["1"]) {
            assert_eq!(s.first, (cap, 10), "arm {arm} {label}");
            assert_eq!(s.last, (cap, 10), "arm {arm} {label}");
            assert_eq!(s.slots, (false, false), "arm {arm} {label}");
            assert!(s.same_shape, "arm {arm} {label}");
        }
    }
}

#[test]
fn a_chain_count_of_8_or_less_keeps_todays_answer() {
    // n_chain = 5 + ESCAPE_SLACK <= GENEROUS: every arm gives today's answer (no count for
    // an escaping `this`: GENEROUS).
    let today = run_each_arm(0, es5_chain_5, &["1"]);
    for (label, s) in &today {
        assert_eq!(s.last, (usize::from(GENEROUS), 5), "{label}");
        assert_eq!(s.slots, (false, false), "{label}");
    }
    for arm in [1, 2, 3] {
        let seen = run_each_arm(arm, es5_chain_5, &["1"]);
        for ((label, s), (_, t)) in seen.iter().zip(&today) {
            assert_eq!((s.first, s.last), (t.first, t.last), "arm {arm} {label}");
            assert_eq!(s.slots, t.slots, "arm {arm} {label}");
        }
    }
}

#[test]
fn every_arm_keeps_the_count_of_a_constructor_with_no_chain() {
    // A plain constructor (no chain level adds a name, n_chain = 2): every arm gives
    // today's 2 slots.
    for arm in 0..=3 {
        for (label, s) in run_each_arm(arm, plain_point, &["3"]) {
            assert_eq!(s.last, (2, 2), "arm {arm} {label}");
        }
    }
}
