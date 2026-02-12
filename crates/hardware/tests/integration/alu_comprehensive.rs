//! Comprehensive ALU and execution tests using actual CPU execution.

use crate::common::harness::TestHarness;
use crate::common::builder::instruction::InstructionBuilder;

#[test]
fn test_alu_add_operations() {
    let mut h = TestHarness::boot_default();

    // ADD x1, x0, x0 (0 + 0 = 0)
    h.cpu.regs.write(10, 5);
    h.cpu.regs.write(11, 3);
    h.execute_inst(InstructionBuilder::add(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 8);

    // Test negative numbers
    h.cpu.regs.write(10, (-5i64) as u64);
    h.cpu.regs.write(11, 3);
    h.execute_inst(InstructionBuilder::add(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12) as i64, -2);
}

#[test]
fn test_alu_sub_operations() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 10);
    h.cpu.regs.write(11, 3);
    h.execute_inst(InstructionBuilder::sub(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 7);

    // Test underflow
    h.cpu.regs.write(10, 3);
    h.cpu.regs.write(11, 10);
    h.execute_inst(InstructionBuilder::sub(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12) as i64, -7);
}

#[test]
fn test_alu_logical_operations() {
    let mut h = TestHarness::boot_default();

    // AND
    h.cpu.regs.write(10, 0xFF);
    h.cpu.regs.write(11, 0x0F);
    h.execute_inst(InstructionBuilder::and(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 0x0F);

    // OR
    h.cpu.regs.write(10, 0xF0);
    h.cpu.regs.write(11, 0x0F);
    h.execute_inst(InstructionBuilder::or(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 0xFF);

    // XOR
    h.cpu.regs.write(10, 0xFF);
    h.cpu.regs.write(11, 0x0F);
    h.execute_inst(InstructionBuilder::xor(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 0xF0);
}

#[test]
fn test_alu_shift_left() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 1);
    h.cpu.regs.write(11, 4);
    h.execute_inst(InstructionBuilder::sll(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 16);

    // Shift by 0
    h.cpu.regs.write(10, 0xFF);
    h.cpu.regs.write(11, 0);
    h.execute_inst(InstructionBuilder::sll(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 0xFF);
}

#[test]
fn test_alu_shift_right_logical() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 16);
    h.cpu.regs.write(11, 4);
    h.execute_inst(InstructionBuilder::srl(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 1);

    // High bit should shift in zeros
    h.cpu.regs.write(10, 0x8000_0000_0000_0000);
    h.cpu.regs.write(11, 1);
    h.execute_inst(InstructionBuilder::srl(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 0x4000_0000_0000_0000);
}

#[test]
fn test_alu_shift_right_arithmetic() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 16);
    h.cpu.regs.write(11, 4);
    h.execute_inst(InstructionBuilder::sra(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 1);

    // High bit should shift in ones for negative numbers
    h.cpu.regs.write(10, 0x8000_0000_0000_0000);
    h.cpu.regs.write(11, 1);
    h.execute_inst(InstructionBuilder::sra(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 0xC000_0000_0000_0000);
}

#[test]
fn test_alu_slt_signed() {
    let mut h = TestHarness::boot_default();

    // Positive < Positive
    h.cpu.regs.write(10, 5);
    h.cpu.regs.write(11, 10);
    h.execute_inst(InstructionBuilder::slt(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 1);

    // Positive >= Positive
    h.cpu.regs.write(10, 10);
    h.cpu.regs.write(11, 5);
    h.execute_inst(InstructionBuilder::slt(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 0);

    // Negative < Positive
    h.cpu.regs.write(10, (-5i64) as u64);
    h.cpu.regs.write(11, 5);
    h.execute_inst(InstructionBuilder::slt(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 1);
}

#[test]
fn test_alu_sltu_unsigned() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 5);
    h.cpu.regs.write(11, 10);
    h.execute_inst(InstructionBuilder::sltu(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 1);

    // Large unsigned vs small
    h.cpu.regs.write(10, 0xFFFF_FFFF_FFFF_FFFF);
    h.cpu.regs.write(11, 5);
    h.execute_inst(InstructionBuilder::sltu(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 0); // Max unsigned is not less than 5
}

#[test]
fn test_alu_immediate_operations() {
    let mut h = TestHarness::boot_default();

    // ADDI
    h.cpu.regs.write(10, 5);
    h.execute_inst(InstructionBuilder::addi(12, 10, 10));
    assert_eq!(h.cpu.regs.read(12), 15);

    // XORI
    h.cpu.regs.write(10, 0xFF);
    h.execute_inst(InstructionBuilder::xori(12, 10, 0x0F));
    assert_eq!(h.cpu.regs.read(12), 0xF0);

    // ORI
    h.cpu.regs.write(10, 0xF0);
    h.execute_inst(InstructionBuilder::ori(12, 10, 0x0F));
    assert_eq!(h.cpu.regs.read(12), 0xFF);

    // ANDI
    h.cpu.regs.write(10, 0xFF);
    h.execute_inst(InstructionBuilder::andi(12, 10, 0x0F));
    assert_eq!(h.cpu.regs.read(12), 0x0F);
}

#[test]
fn test_alu_32bit_operations() {
    let mut h = TestHarness::boot_default();

    // ADDW
    h.cpu.regs.write(10, 0xFFFF_FFFF);
    h.cpu.regs.write(11, 1);
    h.execute_inst(InstructionBuilder::addw(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12) as i64, 0); // Wraps to 0, sign-extended

    // SUBW
    h.cpu.regs.write(10, 1);
    h.cpu.regs.write(11, 2);
    h.execute_inst(InstructionBuilder::subw(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 0xFFFF_FFFF_FFFF_FFFF); // -1 sign-extended
}

#[test]
fn test_alu_shift_immediate() {
    let mut h = TestHarness::boot_default();

    // SLLI
    h.cpu.regs.write(10, 1);
    h.execute_inst(InstructionBuilder::slli(12, 10, 5));
    assert_eq!(h.cpu.regs.read(12), 32);

    // SRLI
    h.cpu.regs.write(10, 32);
    h.execute_inst(InstructionBuilder::srli(12, 10, 5));
    assert_eq!(h.cpu.regs.read(12), 1);

    // SRAI
    h.cpu.regs.write(10, 0x8000_0000_0000_0000);
    h.execute_inst(InstructionBuilder::srai(12, 10, 4));
    assert_eq!(h.cpu.regs.read(12), 0xF800_0000_0000_0000);
}

#[test]
fn test_alu_zero_register_preserved() {
    let mut h = TestHarness::boot_default();

    // Try to write to x0
    h.cpu.regs.write(10, 42);
    h.execute_inst(InstructionBuilder::add(0, 10, 10));
    assert_eq!(h.cpu.regs.read(0), 0); // x0 must always be 0
}

#[test]
fn test_multiply_operations() {
    let mut h = TestHarness::boot_default();

    // MUL
    h.cpu.regs.write(10, 6);
    h.cpu.regs.write(11, 7);
    h.execute_inst(InstructionBuilder::mul(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 42);

    // MULW
    h.cpu.regs.write(10, 1000);
    h.cpu.regs.write(11, 1000);
    h.execute_inst(InstructionBuilder::mulw(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12) as i32, 1_000_000);
}

#[test]
fn test_divide_operations() {
    let mut h = TestHarness::boot_default();

    // DIV
    h.cpu.regs.write(10, 42);
    h.cpu.regs.write(11, 6);
    h.execute_inst(InstructionBuilder::div(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 7);

    // DIVU
    h.cpu.regs.write(10, 100);
    h.cpu.regs.write(11, 10);
    h.execute_inst(InstructionBuilder::divu(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 10);

    // REM
    h.cpu.regs.write(10, 43);
    h.cpu.regs.write(11, 6);
    h.execute_inst(InstructionBuilder::rem(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 1);

    // REMU
    h.cpu.regs.write(10, 101);
    h.cpu.regs.write(11, 10);
    h.execute_inst(InstructionBuilder::remu(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), 1);
}

#[test]
fn test_divide_by_zero() {
    let mut h = TestHarness::boot_default();

    // Division by zero should return -1 per RISC-V spec
    h.cpu.regs.write(10, 42);
    h.cpu.regs.write(11, 0);
    h.execute_inst(InstructionBuilder::div(12, 10, 11));
    assert_eq!(h.cpu.regs.read(12), u64::MAX); // -1
}

#[test]
fn test_branch_not_taken() {
    let mut h = TestHarness::boot_default();
    let initial_pc = h.cpu.pc;

    h.cpu.regs.write(10, 5);
    h.cpu.regs.write(11, 10);
    h.execute_inst(InstructionBuilder::beq(10, 11, 8)); // Not equal, don't branch

    // PC should advance by 4 (instruction size), not branch
    assert_eq!(h.cpu.pc, initial_pc + 4);
}

#[test]
fn test_branch_taken() {
    let mut h = TestHarness::boot_default();
    let initial_pc = h.cpu.pc;

    h.cpu.regs.write(10, 5);
    h.cpu.regs.write(11, 5);
    h.execute_inst(InstructionBuilder::beq(10, 11, 8)); // Equal, branch

    // PC should branch
    assert_eq!(h.cpu.pc, initial_pc + 8);
}

#[test]
fn test_bne_branch() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 5);
    h.cpu.regs.write(11, 10);
    let initial_pc = h.cpu.pc;
    h.execute_inst(InstructionBuilder::bne(10, 11, 8));
    assert_eq!(h.cpu.pc, initial_pc + 8); // Should branch
}

#[test]
fn test_blt_branch() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 5);
    h.cpu.regs.write(11, 10);
    let initial_pc = h.cpu.pc;
    h.execute_inst(InstructionBuilder::blt(10, 11, 8));
    assert_eq!(h.cpu.pc, initial_pc + 8); // Should branch (5 < 10)
}

#[test]
fn test_bge_branch() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 10);
    h.cpu.regs.write(11, 5);
    let initial_pc = h.cpu.pc;
    h.execute_inst(InstructionBuilder::bge(10, 11, 8));
    assert_eq!(h.cpu.pc, initial_pc + 8); // Should branch (10 >= 5)
}

#[test]
fn test_bltu_branch() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 5);
    h.cpu.regs.write(11, 10);
    let initial_pc = h.cpu.pc;
    h.execute_inst(InstructionBuilder::bltu(10, 11, 8));
    assert_eq!(h.cpu.pc, initial_pc + 8); // Should branch
}

#[test]
fn test_bgeu_branch() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 10);
    h.cpu.regs.write(11, 10);
    let initial_pc = h.cpu.pc;
    h.execute_inst(InstructionBuilder::bgeu(10, 11, 8));
    assert_eq!(h.cpu.pc, initial_pc + 8); // Should branch (equal)
}
