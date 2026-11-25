//! STAR-DST probe (ABI 7): the 3 fused opcodes `GetNamedPropertyStar`,
//! `LdaCurrentContextSlotStar` and `LdaContextSlotStar` (T7).
//!
//! Each fused opcode is its load with the `dst` operand of a `Star` appended as the last
//! operand, so the load operands keep their byte offsets and the slot of the named form is the
//! slot of its `GetNamedProperty`.

mod common;

use ep9_bytecode::*;

const FUSED: [(Opcode, Opcode); 3] = [
    (Opcode::GetNamedPropertyStar, Opcode::GetNamedProperty),
    (
        Opcode::LdaCurrentContextSlotStar,
        Opcode::LdaCurrentContextSlot,
    ),
    (Opcode::LdaContextSlotStar, Opcode::LdaContextSlot),
];

/// T7: the rows are appended at 0x9e-0xa0, and each is its load plus a last `RegOut`.
#[test]
fn fused_rows_are_the_load_with_a_last_dst() {
    assert_eq!(OPCODE_COUNT, 0xad);
    assert_eq!(ABI_VERSION, 8);
    assert_eq!(Opcode::GetNamedPropertyStar as u8, 0x9e);
    assert_eq!(Opcode::LdaCurrentContextSlotStar as u8, 0x9f);
    assert_eq!(Opcode::LdaContextSlotStar as u8, 0xa0);
    assert_eq!(MAX_OPERANDS, 5);
    assert_eq!(MAX_INSTRUCTION_SIZE, 22);
    for (fused, load) in FUSED {
        let (f, l) = (fused.info(), load.info());
        assert_eq!(f.name, format!("{}Star", l.name));
        assert_eq!(f.acc, AccUse::Write, "{}", f.name);
        assert_eq!(f.acc, l.acc, "{}", f.name);
        assert_eq!(f.flags, l.flags, "{}", f.name);
        assert_eq!(f.operands.len(), l.operands.len() + 1, "{}", f.name);
        assert_eq!(&f.operands[..l.operands.len()], l.operands, "{}", f.name);
        let dst = f.operands.last().unwrap();
        assert_eq!((dst.name, dst.kind), ("dst", OperandKind::RegOut));
        for scale in [
            OperandScale::Single,
            OperandScale::Wide,
            OperandScale::ExtraWide,
        ] {
            for k in 0..l.operands.len() {
                assert_eq!(f.operand_offset(k, scale), l.operand_offset(k, scale));
            }
            // One instruction for the pair: the size of the load plus the size of `dst`.
            assert_eq!(
                f.size(scale),
                l.size(scale) + OperandKind::RegOut.size(scale),
                "{} {scale:?}",
                f.name
            );
        }
    }
    // One-byte lengths 5, 3 and 5 (the late `dst` read of the T0 handlers).
    let one = |op: Opcode| op.info().size(OperandScale::Single);
    assert_eq!(
        (
            one(Opcode::GetNamedPropertyStar),
            one(Opcode::LdaCurrentContextSlotStar),
            one(Opcode::LdaContextSlotStar)
        ),
        (5, 3, 5)
    );
    // The named form keeps the LoadNamed slot (D2): operand 2, no new slot kind.
    assert_eq!(
        Opcode::GetNamedPropertyStar.info().operands[2].param,
        OperandParam::Slot(FeedbackSlotKind::LoadNamed)
    );
}

/// A unit whose function 1 has `regs` registers, a scope of 2 slots, and one fused
/// instruction of each opcode with `dst` = r(`dst`).
fn unit_with(regs: u32, dst: u32) -> VerifiedUnit {
    let src = "function f() {}";
    let mut u = UnitBuilder::new(src, common::script_options(false));
    let scope = u.add_scope(
        ScopeKind::Function,
        ScopeFlags::NEEDS_CONTEXT,
        None,
        vec![
            (UnitString::new("a"), VariableMode::Var),
            (UnitString::new("b"), VariableMode::Var),
        ],
    );
    let mut f = FunctionBuilder::new(FunctionMeta {
        kind: FunctionKind::Normal,
        flags: FunctionFlags::default(),
        name: Some(UnitString::new("f")),
        parent: Some(FunctionId(0)),
        outer_scope: Some(scope),
        source_start: 0,
        source_end: src.len() as u32,
        parameter_count: 0,
        length: 0,
    });
    f.new_register_list(regs);
    let k = f.string_constant("x");
    let s = f.feedback_slot(FeedbackSlotKind::LoadNamed);
    let d = Register::local(dst);
    f.get_named_property_star(Register::local(0), k, s, d)
        .lda_current_context_slot_star(1, d)
        .lda_context_slot_star(Register::local(0), 1, 0, d)
        .lda_undefined()
        .return_();
    let fcode = f.finish().unwrap();
    let mut b = FunctionBuilder::new(FunctionMeta::entry(
        CodeKind::Script,
        false,
        src.len() as u32,
    ));
    let kf = b.function_constant(FunctionId(1));
    let sc = b.feedback_slot(FeedbackSlotKind::CreateClosure);
    b.create_closure(kf, sc, 0).return_();
    u.set_function(FunctionId(0), b.finish().unwrap());
    u.set_function(FunctionId(1), fcode);
    u.finish().unwrap()
}

/// T7: a one-byte, a `Wide` and an `ExtraWide` form of each new row build, verify, round-trip
/// through the bytes, and decode with their operands.
#[test]
fn fused_rows_round_trip_at_every_scale() {
    for (regs, dst, scale) in [
        (3, 2, OperandScale::Single),
        (300, 200, OperandScale::Wide),
        (40_000, 33_000, OperandScale::ExtraWide),
    ] {
        let unit = unit_with(regs, dst);
        let bytes = unit.to_bytes();
        let back = CompiledUnit::from_bytes(&bytes).unwrap_or_else(|e| panic!("{scale:?}: {e}"));
        assert_eq!(*back, *unit.unit(), "{scale:?}");
        assert_eq!(back.to_bytes(), bytes, "{scale:?}");
        let insns: Vec<_> = Instructions::new(&back.functions[1].bytecode)
            .map(|i| i.unwrap())
            .collect();
        let want = [
            (
                Opcode::GetNamedPropertyStar,
                vec![0i64, 0, 0, i64::from(dst)],
            ),
            (Opcode::LdaCurrentContextSlotStar, vec![1, i64::from(dst)]),
            (Opcode::LdaContextSlotStar, vec![0, 1, 0, i64::from(dst)]),
        ];
        for (insn, (op, operands)) in insns.iter().zip(&want) {
            assert_eq!(insn.opcode, *op, "{scale:?}");
            assert_eq!(insn.scale, scale, "{scale:?} {op:?}");
            let got: Vec<i64> = (0..insn.operand_count()).map(|k| insn.operand(k)).collect();
            assert_eq!(&got, operands, "{scale:?} {op:?}");
            assert_eq!(
                insn.register(insn.operand_count() - 1),
                Register::local(dst)
            );
        }
        // The disassembler names each fused opcode.
        let text = disassemble(&back);
        for (fused, _) in FUSED {
            assert!(text.contains(fused.name()), "{scale:?}: {text}");
        }
    }
}

// ---- FunctionBuilder::fuse_star (rule O13, conditions B1-B4) ----

/// Builds function 1 with 300 registers, 300 LoadNamed slots, 300 string constants and a
/// scope of 2 slots: `body` emits its code (before `LdaUndefined`, `Return`) and gives what
/// `fuse_star` returned. The result: that value and the verified function.
fn fused(body: impl FnOnce(&mut FunctionBuilder) -> bool) -> (bool, Function) {
    let src = "function f() { return 0; }";
    let mut u = UnitBuilder::new(src, common::script_options(false));
    let scope = u.add_scope(
        ScopeKind::Function,
        ScopeFlags::NEEDS_CONTEXT,
        None,
        vec![
            (UnitString::new("a"), VariableMode::Var),
            (UnitString::new("b"), VariableMode::Var),
        ],
    );
    let mut f = FunctionBuilder::new(FunctionMeta {
        kind: FunctionKind::Normal,
        flags: FunctionFlags::default(),
        name: Some(UnitString::new("f")),
        parent: Some(FunctionId(0)),
        outer_scope: Some(scope),
        source_start: 0,
        source_end: src.len() as u32,
        parameter_count: 1,
        length: 1,
    });
    f.new_register_list(300);
    for i in 0..300 {
        f.string_constant(UnitString::new(&format!("p{i}")));
        f.feedback_slot(FeedbackSlotKind::LoadNamed);
    }
    let r = body(&mut f);
    f.lda_undefined().return_();
    let fcode = f.finish().unwrap();
    let mut b = FunctionBuilder::new(FunctionMeta::entry(
        CodeKind::Script,
        false,
        src.len() as u32,
    ));
    let kf = b.function_constant(FunctionId(1));
    let sc = b.feedback_slot(FeedbackSlotKind::CreateClosure);
    b.create_closure(kf, sc, 0).return_();
    u.set_function(FunctionId(0), b.finish().unwrap());
    u.set_function(FunctionId(1), fcode);
    let unit = u.finish().unwrap();
    (r, unit.unit().functions[1].clone())
}

/// The opcodes of `f`, with a `Wide` or `ExtraWide` prefix as "W" or "X" before the name.
fn ops(f: &Function) -> Vec<String> {
    Instructions::new(&f.bytecode)
        .map(|i| {
            let i = i.unwrap();
            let p = match i.scale {
                OperandScale::Single => "",
                OperandScale::Wide => "W ",
                OperandScale::ExtraWide => "X ",
            };
            format!("{p}{:?}", i.opcode)
        })
        .collect()
}

/// `fuse_star`, else `star` (the O13 call of the register optimizer).
fn star(b: &mut FunctionBuilder, dst: Register, family: u8) -> bool {
    let fused = b.fuse_star(dst, family);
    if !fused {
        b.star(dst);
    }
    fused
}

const K: ConstIndex = ConstIndex(0);
const S: FeedbackSlot = FeedbackSlot(0);

fn r(i: u32) -> Register {
    Register::local(i)
}

/// The positive case: the load becomes the fused opcode, keeps its position entry and its
/// instruction index; the instruction count does not change.
#[test]
fn fuse_star_fuses_the_last_load() {
    let (ok, f) = fused(|b| {
        b.set_statement_position(15);
        b.get_named_property(r(0), K, S);
        let n = b.instruction_count();
        let ok = b.fuse_star(r(1), 1);
        assert_eq!(b.instruction_count(), n);
        ok
    });
    assert!(ok);
    assert_eq!(ops(&f), ["GetNamedPropertyStar", "LdaUndefined", "Return"]);
    let first = Instructions::new(&f.bytecode).next().unwrap().unwrap();
    let operands: Vec<i64> = (0..4).map(|k| first.operand(k)).collect();
    assert_eq!(operands, [0, 0, 0, 1]);
    assert_eq!(
        f.positions,
        [PositionEntry {
            bytecode_offset: 0,
            source_offset: 15,
            is_statement: true,
        }]
    );
    // Family 2 fuses the 2 context loads too; dst may be a parameter or the receiver.
    for (load, want) in [
        (0, "LdaCurrentContextSlotStar"),
        (1, "LdaContextSlotStar"),
        (2, "GetNamedPropertyStar"),
    ] {
        let (ok, f) = fused(|b| {
            match load {
                0 => b.lda_current_context_slot(1),
                1 => b.lda_context_slot(r(0), 1, 0),
                _ => b.get_named_property(Register::RECEIVER, K, S),
            };
            star(b, Register::parameter(0), 2)
        });
        assert!(ok, "{want}");
        assert_eq!(ops(&f), [want, "LdaUndefined", "Return"]);
        let first = Instructions::new(&f.bytecode).next().unwrap().unwrap();
        assert_eq!(
            first.register(first.operand_count() - 1),
            Register::parameter(0)
        );
    }
}

/// B1: the last instruction must be a load of the family.
#[test]
fn fuse_star_b1_needs_a_load_of_the_family() {
    // No instruction yet.
    let (ok, f) = fused(|b| star(b, r(1), 2));
    assert!(!ok);
    assert_eq!(ops(&f), ["Star", "LdaUndefined", "Return"]);
    // Not a load of the set: LdaZero, an immutable context load, GetNamedPropertyFromSuper.
    for load in 0..3 {
        let (ok, f) = fused(|b| {
            match load {
                0 => b.lda_zero(),
                1 => b.lda_immutable_current_context_slot(1),
                _ => b.get_named_property_from_super(r(0), K, S),
            };
            star(b, r(1), 2)
        });
        assert!(!ok, "load {load}");
        assert_eq!(ops(&f)[1], "Star", "load {load}");
    }
    // Family 1 has no context form; family 0 fuses nothing.
    for (family, load) in [(1, 0), (1, 1), (0, 2)] {
        let (ok, f) = fused(|b| {
            match load {
                0 => b.lda_current_context_slot(1),
                1 => b.lda_context_slot(r(0), 1, 0),
                _ => b.get_named_property(r(0), K, S),
            };
            star(b, r(1), family)
        });
        assert!(!ok, "family {family} load {load}");
        assert_eq!(ops(&f).len(), 4, "family {family} load {load}");
        assert_eq!(ops(&f)[1], "Star");
    }
    // An `Ldar` (an owed load the optimizer emitted) between the load and the `Star`.
    let (ok, f) = fused(|b| {
        b.get_named_property(r(0), K, S).ldar(r(2));
        star(b, r(1), 2)
    });
    assert!(!ok);
    assert_eq!(
        ops(&f),
        ["GetNamedProperty", "Ldar", "Star", "LdaUndefined", "Return"]
    );
}

/// B2: a label bound after the load blocks the fuse, also when no jump uses it, and also
/// when it is the start or end of a handler range.
#[test]
fn fuse_star_b2_no_label_on_the_star() {
    // An unused label.
    let (ok, f) = fused(|b| {
        b.get_named_property(r(0), K, S);
        let l = b.new_label();
        b.bind(l);
        star(b, r(1), 2)
    });
    assert!(!ok);
    assert_eq!(
        ops(&f),
        ["GetNamedProperty", "Star", "LdaUndefined", "Return"]
    );
    // A jump target.
    let (ok, f) = fused(|b| {
        let l = b.new_label();
        b.jump_if_true(l).get_named_property(r(0), K, S);
        b.bind(l);
        star(b, r(1), 2)
    });
    assert!(!ok);
    assert_eq!(
        ops(&f),
        [
            "JumpIfTrue",
            "GetNamedProperty",
            "Star",
            "LdaUndefined",
            "Return"
        ]
    );
    // The start and the end of a handler range.
    for edge in [0, 1] {
        let (ok, f) = fused(|b| {
            let (start, end, target) = (b.new_label(), b.new_label(), b.new_label());
            if edge == 1 {
                b.bind(start);
            }
            b.lda_current_context_slot(1);
            b.bind(if edge == 0 { start } else { end });
            let ok = star(b, r(1), 2);
            if edge == 0 {
                b.lda_zero();
                b.bind(end);
            }
            b.lda_zero().jump(target);
            b.bind(target);
            b.add_handler(start, end, target, r(2), HandlerKind::Caught);
            ok
        });
        assert!(!ok, "edge {edge}");
        assert_eq!(
            ops(&f)[0..2],
            ["LdaCurrentContextSlot", "Star"],
            "edge {edge}"
        );
        assert_eq!(f.handlers.len(), 1);
    }
    // A label bound before the load does not block (it is on the load).
    let (ok, f) = fused(|b| {
        let l = b.new_label();
        b.jump_if_true(l);
        b.bind(l);
        b.get_named_property(r(0), K, S);
        star(b, r(1), 2)
    });
    assert!(ok);
    assert_eq!(
        ops(&f),
        [
            "JumpIfTrue",
            "GetNamedPropertyStar",
            "LdaUndefined",
            "Return"
        ]
    );
}

/// B3: a pending position blocks the fuse; the `Star` takes the entry (the carried statement
/// position of regopt O9 reaches the builder as a pending statement position).
#[test]
fn fuse_star_b3_no_pending_position() {
    for stmt in [true, false] {
        let (ok, f) = fused(|b| {
            b.get_named_property(r(0), K, S);
            if stmt {
                b.set_statement_position(20);
            } else {
                b.set_expression_position(20);
            }
            star(b, r(1), 2)
        });
        assert!(!ok, "statement {stmt}");
        assert_eq!(
            ops(&f),
            ["GetNamedProperty", "Star", "LdaUndefined", "Return"]
        );
        assert_eq!(
            f.positions,
            [PositionEntry {
                bytecode_offset: 4,
                source_offset: 20,
                is_statement: stmt,
            }]
        );
    }
}

/// B4: every scalable operand of the load and `dst` need scale `Single`.
#[test]
fn fuse_star_b4_one_byte_operands_only() {
    let wide = r(200);
    for (case, want) in [
        (0, ["GetNamedProperty", "W Star"]),
        (1, ["W GetNamedProperty", "Star"]),
        (2, ["W GetNamedProperty", "Star"]),
        (3, ["W GetNamedProperty", "Star"]),
        (4, ["LdaCurrentContextSlot", "W Star"]),
        (5, ["W LdaContextSlot", "Star"]),
    ] {
        let (ok, f) = fused(|b| {
            let dst = if matches!(case, 0 | 4) { wide } else { r(1) };
            match case {
                0 => b.get_named_property(r(0), K, S),
                1 => b.get_named_property(wide, K, S),
                2 => b.get_named_property(r(0), ConstIndex(299), S),
                3 => b.get_named_property(r(0), K, FeedbackSlot(299)),
                4 => b.lda_current_context_slot(1),
                _ => b.lda_context_slot(wide, 1, 0),
            };
            star(b, dst, 2)
        });
        assert!(!ok, "case {case}");
        assert_eq!(ops(&f)[0..2], want, "case {case}");
    }
    // The largest one-byte values fuse: r127, k255, s255.
    let (ok, f) = fused(|b| {
        b.get_named_property(r(127), ConstIndex(255), FeedbackSlot(255));
        star(b, r(127), 1)
    });
    assert!(ok);
    assert_eq!(ops(&f)[0], "GetNamedPropertyStar");
    // A parameter register is a negative operand: a0 fits one byte.
    let (ok, _) = fused(|b| {
        b.get_named_property(Register::parameter(0), K, S);
        star(b, Register::parameter(0), 1)
    });
    assert!(ok);
}
