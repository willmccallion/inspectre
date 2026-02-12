//! Comprehensive memory access tests including loads, stores, and alignment.

use crate::common::harness::TestHarness;
use crate::common::builder::instruction::InstructionBuilder;

#[test]
fn test_load_byte_signed() {
    let mut h = TestHarness::boot_default();

    // Write test data to memory
    h.bus_write_u8(0x8000_1000, 0xFF); // -1 as signed byte
    h.bus_write_u8(0x8000_1001, 0x7F); // 127 as signed byte

    // LB from 0x8000_1000
    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::lb(11, 10, 0));
    assert_eq!(h.cpu.regs.read(11) as i64, -1); // Sign-extended

    // LB from 0x8000_1001
    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::lb(11, 10, 1));
    assert_eq!(h.cpu.regs.read(11) as i64, 127);
}

#[test]
fn test_load_byte_unsigned() {
    let mut h = TestHarness::boot_default();

    h.bus_write_u8(0x8000_1000, 0xFF);

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::lbu(11, 10, 0));
    assert_eq!(h.cpu.regs.read(11), 0xFF); // Zero-extended, not sign-extended
}

#[test]
fn test_load_halfword_signed() {
    let mut h = TestHarness::boot_default();

    h.bus_write_u16(0x8000_1000, 0xFFFF); // -1 as signed halfword

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::lh(11, 10, 0));
    assert_eq!(h.cpu.regs.read(11) as i64, -1); // Sign-extended
}

#[test]
fn test_load_halfword_unsigned() {
    let mut h = TestHarness::boot_default();

    h.bus_write_u16(0x8000_1000, 0xFFFF);

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::lhu(11, 10, 0));
    assert_eq!(h.cpu.regs.read(11), 0xFFFF); // Zero-extended
}

#[test]
fn test_load_word_signed() {
    let mut h = TestHarness::boot_default();

    h.bus_write_u32(0x8000_1000, 0xFFFF_FFFF); // -1 as signed word

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::lw(11, 10, 0));
    assert_eq!(h.cpu.regs.read(11) as i64, -1); // Sign-extended
}

#[test]
fn test_load_word_unsigned() {
    let mut h = TestHarness::boot_default();

    h.bus_write_u32(0x8000_1000, 0xFFFF_FFFF);

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::lwu(11, 10, 0));
    assert_eq!(h.cpu.regs.read(11), 0xFFFF_FFFF); // Zero-extended
}

#[test]
fn test_load_doubleword() {
    let mut h = TestHarness::boot_default();

    h.bus_write_u64(0x8000_1000, 0x1234_5678_9ABC_DEF0);

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::ld(11, 10, 0));
    assert_eq!(h.cpu.regs.read(11), 0x1234_5678_9ABC_DEF0);
}

#[test]
fn test_store_byte() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 0x8000_1000);
    h.cpu.regs.write(11, 0x42);
    h.execute_inst(InstructionBuilder::sb(11, 10, 0));

    assert_eq!(h.bus_read_u8(0x8000_1000), 0x42);
}

#[test]
fn test_store_halfword() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 0x8000_1000);
    h.cpu.regs.write(11, 0x1234);
    h.execute_inst(InstructionBuilder::sh(11, 10, 0));

    assert_eq!(h.bus_read_u16(0x8000_1000), 0x1234);
}

#[test]
fn test_store_word() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 0x8000_1000);
    h.cpu.regs.write(11, 0x1234_5678);
    h.execute_inst(InstructionBuilder::sw(11, 10, 0));

    assert_eq!(h.bus_read_u32(0x8000_1000), 0x1234_5678);
}

#[test]
fn test_store_doubleword() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 0x8000_1000);
    h.cpu.regs.write(11, 0x1234_5678_9ABC_DEF0);
    h.execute_inst(InstructionBuilder::sd(11, 10, 0));

    assert_eq!(h.bus_read_u64(0x8000_1000), 0x1234_5678_9ABC_DEF0);
}

#[test]
fn test_load_with_positive_offset() {
    let mut h = TestHarness::boot_default();

    h.bus_write_u64(0x8000_1010, 0xDEAD_BEEF);

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::ld(11, 10, 0x10));
    assert_eq!(h.cpu.regs.read(11), 0xDEAD_BEEF);
}

#[test]
fn test_load_with_negative_offset() {
    let mut h = TestHarness::boot_default();

    h.bus_write_u64(0x8000_0FF0, 0xCAFE_BABE);

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::ld(11, 10, -16));
    assert_eq!(h.cpu.regs.read(11), 0xCAFE_BABE);
}

#[test]
fn test_store_with_offset() {
    let mut h = TestHarness::boot_default();

    h.cpu.regs.write(10, 0x8000_1000);
    h.cpu.regs.write(11, 0x1234_5678);
    h.execute_inst(InstructionBuilder::sw(11, 10, 0x20));

    assert_eq!(h.bus_read_u32(0x8000_1020), 0x1234_5678);
}

#[test]
fn test_load_store_sequence() {
    let mut h = TestHarness::boot_default();

    // Store value
    h.cpu.regs.write(10, 0x8000_1000);
    h.cpu.regs.write(11, 42);
    h.execute_inst(InstructionBuilder::sd(11, 10, 0));

    // Load it back
    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::ld(12, 10, 0));

    assert_eq!(h.cpu.regs.read(12), 42);
}

#[test]
fn test_load_multiple_bytes() {
    let mut h = TestHarness::boot_default();

    // Write pattern
    for i in 0..16 {
        h.bus_write_u8(0x8000_1000 + i, (i & 0xFF) as u8);
    }

    // Read them back
    for i in 0..16 {
        h.cpu.regs.write(10, 0x8000_1000);
        h.execute_inst(InstructionBuilder::lbu(11, 10, i as i64));
        assert_eq!(h.cpu.regs.read(11), i as u64);
    }
}

#[test]
fn test_overlapping_stores() {
    let mut h = TestHarness::boot_default();

    // Store word
    h.cpu.regs.write(10, 0x8000_1000);
    h.cpu.regs.write(11, 0x1234_5678);
    h.execute_inst(InstructionBuilder::sw(11, 10, 0));

    // Store byte that overlaps
    h.cpu.regs.write(11, 0xFF);
    h.execute_inst(InstructionBuilder::sb(11, 10, 0));

    // First byte should be 0xFF, rest should be from original word
    assert_eq!(h.bus_read_u8(0x8000_1000), 0xFF);
}

#[test]
fn test_load_zero_to_register() {
    let mut h = TestHarness::boot_default();

    h.bus_write_u64(0x8000_1000, 0);

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::ld(11, 10, 0));
    assert_eq!(h.cpu.regs.read(11), 0);
}

#[test]
fn test_store_from_zero_register() {
    let mut h = TestHarness::boot_default();

    // Store from x0 (always 0)
    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::sd(0, 10, 0));

    assert_eq!(h.bus_read_u64(0x8000_1000), 0);
}

#[test]
fn test_memory_endianness() {
    let mut h = TestHarness::boot_default();

    // Store 0x12345678 and verify byte order (little-endian)
    h.cpu.regs.write(10, 0x8000_1000);
    h.cpu.regs.write(11, 0x12345678);
    h.execute_inst(InstructionBuilder::sw(11, 10, 0));

    assert_eq!(h.bus_read_u8(0x8000_1000), 0x78);
    assert_eq!(h.bus_read_u8(0x8000_1001), 0x56);
    assert_eq!(h.bus_read_u8(0x8000_1002), 0x34);
    assert_eq!(h.bus_read_u8(0x8000_1003), 0x12);
}

#[test]
fn test_load_across_cache_line() {
    let mut h = TestHarness::boot_default();

    // Assuming 64-byte cache lines, write across boundary
    h.bus_write_u64(0x8000_103C, 0xDEADBEEF_CAFEBABE);

    h.cpu.regs.write(10, 0x8000_103C);
    h.execute_inst(InstructionBuilder::ld(11, 10, 0));
    assert_eq!(h.cpu.regs.read(11), 0xDEADBEEF_CAFEBABE);
}

#[test]
fn test_adjacent_memory_locations() {
    let mut h = TestHarness::boot_default();

    // Write to adjacent locations
    h.cpu.regs.write(10, 0x8000_1000);
    h.cpu.regs.write(11, 1);
    h.execute_inst(InstructionBuilder::sd(11, 10, 0));

    h.cpu.regs.write(11, 2);
    h.execute_inst(InstructionBuilder::sd(11, 10, 8));

    h.cpu.regs.write(11, 3);
    h.execute_inst(InstructionBuilder::sd(11, 10, 16));

    // Read them back
    h.execute_inst(InstructionBuilder::ld(12, 10, 0));
    assert_eq!(h.cpu.regs.read(12), 1);

    h.execute_inst(InstructionBuilder::ld(12, 10, 8));
    assert_eq!(h.cpu.regs.read(12), 2);

    h.execute_inst(InstructionBuilder::ld(12, 10, 16));
    assert_eq!(h.cpu.regs.read(12), 3);
}

#[test]
fn test_maximum_offset() {
    let mut h = TestHarness::boot_default();

    // Maximum positive 12-bit signed offset is 2047
    h.bus_write_u64(0x8000_17FF, 0xAAAA_BBBB);

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::ld(11, 10, 2047));
    assert_eq!(h.cpu.regs.read(11), 0xAAAA_BBBB);
}

#[test]
fn test_minimum_offset() {
    let mut h = TestHarness::boot_default();

    // Minimum 12-bit signed offset is -2048
    h.bus_write_u64(0x8000_0800, 0xCCCC_DDDD);

    h.cpu.regs.write(10, 0x8000_1000);
    h.execute_inst(InstructionBuilder::ld(11, 10, -2048));
    assert_eq!(h.cpu.regs.read(11), 0xCCCC_DDDD);
}
