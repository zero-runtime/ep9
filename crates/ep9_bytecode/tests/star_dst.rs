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
    assert_eq!(OPCODE_COUNT, 0xa1);
    assert_eq!(ABI_VERSION, 7);
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
