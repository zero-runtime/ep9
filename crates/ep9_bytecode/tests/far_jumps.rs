//! ABI 8, the WIDE-SET probe (T4): units with `Far` jump twins. The builder makes a twin only
//! in `finish` with `far_jumps` on; such a unit round-trips, verifies, disassembles and builds
//! to the same bytes twice, and its instruction list is the `far_jumps`-off list after
//! `near_form`.

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
