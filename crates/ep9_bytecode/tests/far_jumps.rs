//! ABI 8, the WIDE-SET probe (T4): units with `Far` jump twins. The builder makes a twin only
//! in `finish` with `far_jumps` on; such a unit round-trips, verifies, disassembles and builds
//! to the same bytes twice, and its instruction list is the `far_jumps`-off list after
//! `near_form`. F-a: units with the prefix `Wide2` (`wide2` on), the same checks.

mod common;

use ep9_bytecode::*;

/// The 11 one-operand forward jumps, in opcode order.
fn bases() -> Vec<Opcode> {
    (Opcode::Jump as u8..=Opcode::JumpIfJSReceiver as u8)
        .map(|b| Opcode::from_u8(b).unwrap())
        .collect()
}

/// One script function: for each base, a jump over `body` one-byte instructions, then a
/// `Jump` over 65,536 bytes (ExtraWide in both forms) and a `Return`.
fn far_unit(far: bool, body: u32) -> CompiledUnit {
    let src = "x";
    let mut u = UnitBuilder::new(src, common::script_options(false));
    let mut b = FunctionBuilder::new(FunctionMeta::entry(CodeKind::Script, false, 1));
    b.set_far_jumps(far);
    for op in bases() {
        let l = b.new_label();
        b.set_statement_position(0);
        b.emit(op, &[l.into_pending()]);
        for _ in 0..body {
            b.lda_zero();
        }
        b.bind(l);
    }
    let l = b.new_label();
    b.jump(l);
    for _ in 0..65_536 {
        b.lda_zero();
    }
    b.bind(l);
    b.return_();
    u.set_function(FunctionId(0), b.finish().unwrap());
    u.finish().unwrap().into_inner()
}

/// (opcode after `near_form`, scale, target instruction index) of each instruction.
fn shape(f: &Function) -> Vec<(Opcode, Option<usize>)> {
    let insns: Vec<Instruction<'_>> = Instructions::new(&f.bytecode).map(|i| i.unwrap()).collect();
    let index = |t: u32| insns.iter().position(|i| i.offset == t).unwrap();
    insns
        .iter()
        .map(|i| (i.opcode.near_form(), i.jump_target().map(index)))
        .collect()
}

/// The instruction index of each position entry.
fn position_indices(f: &Function) -> Vec<(usize, u32)> {
    let starts: Vec<u32> = Instructions::new(&f.bytecode)
        .map(|i| i.unwrap().offset)
        .collect();
    f.positions
        .iter()
        .map(|p| {
            let i = starts.binary_search(&p.bytecode_offset).unwrap();
            (i, p.source_offset)
        })
        .collect()
}

#[test]
fn far_unit_has_every_twin_and_round_trips() {
    let unit = far_unit(true, 300);
    verify(unit.clone()).unwrap();
    let f = &unit.functions[0];
    let ops: Vec<Opcode> = Instructions::new(&f.bytecode)
        .map(|i| i.unwrap())
        .filter(|i| i.opcode.near_form() != Opcode::LdaZero)
        .map(|i| i.opcode)
        .collect();
    let mut want: Vec<Opcode> = bases().iter().map(|b| b.far_twin().unwrap()).collect();
    want.extend([Opcode::Jump, Opcode::Return]);
    assert_eq!(ops, want);
    let bytes = unit.to_bytes();
    let back = CompiledUnit::from_bytes(&bytes).unwrap();
    assert_eq!(*back, unit);
    assert_eq!(back.to_bytes(), bytes);
    let text = disassemble(&back);
    assert_eq!(text, disassemble(&unit));
    // The twin prints its name and its target; the ExtraWide jump keeps its prefix.
    assert!(text.contains("JumpFar @"), "{text}");
    assert!(text.contains("JumpIfJSReceiverFar @"), "{text}");
    assert!(text.contains("Jump.ExtraWide @"), "{text}");
    assert!(!text.contains("Far.Wide"), "{text}");
}

#[test]
fn far_unit_builds_to_identical_bytes_twice() {
    let a = far_unit(true, 300);
    let b = far_unit(true, 300);
    assert_eq!(a.to_bytes(), b.to_bytes());
    assert_eq!(a.content_hash(), b.content_hash());
    // The two forms are two programs with two hashes.
    assert_ne!(a.content_hash(), far_unit(false, 300).content_hash());
}

/// The two forms hold the same instructions and targets by instruction index, and the same
/// number of position entries at the same instructions; each twin is 1 byte shorter than
/// `Wide` + its base.
#[test]
fn near_form_of_the_on_unit_is_the_off_unit() {
    for body in [100, 300, 70_000] {
        let on = far_unit(true, body);
        let off = far_unit(false, body);
        let (fon, foff) = (&on.functions[0], &off.functions[0]);
        assert_eq!(shape(fon), shape(foff), "body {body}");
        assert_eq!(position_indices(fon), position_indices(foff), "body {body}");
        assert_eq!(fon.positions.len(), 11);
        let twins = Instructions::new(&fon.bytecode)
            .filter(|i| i.as_ref().unwrap().opcode.near_form() != i.as_ref().unwrap().opcode)
            .count();
        assert_eq!(twins, if body == 300 { 11 } else { 0 }, "body {body}");
        assert_eq!(
            foff.bytecode.len() - fon.bytecode.len(),
            twins,
            "body {body}"
        );
        // Off: today's form, no twin at all.
        assert!(
            Instructions::new(&foff.bytecode)
                .all(|i| i.as_ref().unwrap().opcode.near_form() == i.as_ref().unwrap().opcode)
        );
    }
}

// ---- F-a: the second scale-2 prefix Wide2 (ABI 8, the WIDE-SET probe) ----

/// One script function with a prefixed `GetNamedProperty`, `SetNamedProperty` and `JumpLoop`
/// (a name constant past 255, a loop over 300 bytes) and a prefixed `LdaSmi`, built with
/// `wide2` on or off (off: the default, no setter call).
fn wide2_unit(wide2: bool) -> CompiledUnit {
    let src = "x";
    let mut u = UnitBuilder::new(src, common::script_options(false));
    let mut b = FunctionBuilder::new(FunctionMeta::entry(CodeKind::Script, false, 1));
    if wide2 {
        b.set_wide2(true);
    }
    let r = b.new_register();
    for j in 0..300 {
        b.string_constant(format!("k{j}").as_str());
    }
    let k = b.string_constant("far");
    let (ls, ss) = (
        b.feedback_slot(FeedbackSlotKind::LoadNamed),
        b.feedback_slot(FeedbackSlotKind::StoreNamed),
    );
    let jl = b.feedback_slot(FeedbackSlotKind::JumpLoop);
    b.set_statement_position(0);
    b.lda_undefined().star(r);
    let done = b.new_label();
    let header = b.bound_label();
    b.lda_true().jump_if_true(done);
    b.get_named_property(r, k, ls).set_named_property(r, k, ss);
    b.lda_smi(1000);
    for _ in 0..300 {
        b.lda_zero();
    }
    b.jump_loop(header, 0, jl);
    b.bind(done);
    b.return_();
    u.set_function(FunctionId(0), b.finish().unwrap());
    u.finish().unwrap().into_inner()
}

/// F-a (T4 form): the `wide2` unit verifies, round-trips, disassembles the same, builds to the
/// same bytes twice, and differs from the off unit only in the 3 prefix bytes of the wide list.
#[test]
fn wide2_unit_round_trips_and_is_deterministic() {
    let (on, off) = (wide2_unit(true), wide2_unit(false));
    verify(on.clone()).unwrap();
    verify(off.clone()).unwrap();
    let bytes = on.to_bytes();
    let back = CompiledUnit::from_bytes(&bytes).unwrap();
    assert_eq!(*back, on);
    assert_eq!(back.to_bytes(), bytes);
    assert_eq!(disassemble(&back), disassemble(&on));
    assert_eq!(wide2_unit(true).to_bytes(), bytes);
    assert_eq!(wide2_unit(true).content_hash(), on.content_hash());
    assert_ne!(on.content_hash(), off.content_hash());
    let (fon, foff) = (&on.functions[0].bytecode, &off.functions[0].bytecode);
    assert_eq!(fon.len(), foff.len());
    let diff: Vec<usize> = (0..fon.len()).filter(|&i| fon[i] != foff[i]).collect();
    assert_eq!(diff.len(), 3);
    for i in diff {
        assert_eq!((foff[i], fon[i]), (Opcode::Wide as u8, Opcode::Wide2 as u8));
        let insn = decode_at(fon, i as u32).unwrap();
        assert!(insn.opcode.takes_wide2(), "{}", insn.opcode);
    }
    assert_eq!(on.functions[0].positions, off.functions[0].positions);
    // The off unit has no Wide2 byte as a prefix.
    assert!(Instructions::new(foff).all(|i| i.unwrap().bytes[0] != Opcode::Wide2 as u8));
}
