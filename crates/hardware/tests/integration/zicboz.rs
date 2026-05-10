//! End-to-end tests for the Zicboz `cbo.zero` instruction.

use crate::common::harness::TestContext;
use rvsim_core::common::PhysAddr;
use rvsim_core::config::Config;
use rvsim_core::isa::rv64i::{funct3 as i_f3, opcodes as i_op};
use rvsim_core::isa::zicboz::{CBO_ZERO_IMM, CBOZ_BLOCK_SIZE};

const RAM_BASE: u64 = 0x8000_0000;
const RAM_SIZE: usize = 0x4000;
const NOP: u32 = 0x0000_0013;
const X10: u32 = 10;

/// Encode `cbo.zero rs1` (Zicboz). I-type with imm=`CBO_ZERO_IMM`, rd=x0,
/// funct3=CBO, opcode=MISC-MEM.
fn cbo_zero(rs1: u32) -> u32 {
    ((CBO_ZERO_IMM as u32) << 20) | ((rs1 & 0x1F) << 15) | (i_f3::CBO << 12) | i_op::OP_MISC_MEM
}

fn fill_pattern(ctx: &mut TestContext, base: u64, len: u64) {
    for off in (0..len).step_by(8) {
        let val = 0xDEAD_BEEF_CAFE_F00Du64.wrapping_add(off);
        ctx.cpu_mut().bus.bus.write_u64(PhysAddr::new(base + off), val);
    }
}

fn read_u64(ctx: &mut TestContext, addr: u64) -> u64 {
    ctx.cpu_mut().bus.bus.read_u64(PhysAddr::new(addr))
}

/// Run `cbo.zero x10; NOP*N` after seeding x10 with `addr_in_x10`.
fn run_cbo_zero(ctx: &mut TestContext, addr_in_x10: u64) {
    let program: Vec<u32> = std::iter::once(cbo_zero(X10))
        .chain(std::iter::repeat_n(NOP, 64))
        .collect();
    for (i, inst) in program.iter().enumerate() {
        let addr = RAM_BASE + (i as u64) * 4;
        ctx.cpu_mut().bus.bus.write_u32(PhysAddr::new(addr), *inst);
    }
    ctx.set_reg(X10 as usize, addr_in_x10);
    ctx.cpu_mut().pc = RAM_BASE;
    ctx.run(128);
}

/// 64-byte aligned address: cbo.zero zeros exactly that block, leaving the
/// adjacent block untouched.
#[test]
fn cbo_zero_aligned_zeroes_one_block_only() {
    let data_addr = RAM_BASE + 0x1000;
    let mut ctx = TestContext::new_with_config(&Config::default()).with_memory(RAM_SIZE, RAM_BASE);
    fill_pattern(&mut ctx, data_addr, 2 * CBOZ_BLOCK_SIZE);

    run_cbo_zero(&mut ctx, data_addr);

    for off in (0..CBOZ_BLOCK_SIZE).step_by(8) {
        assert_eq!(
            read_u64(&mut ctx, data_addr + off),
            0,
            "block must be zeroed at +{off}"
        );
    }
    for off in (CBOZ_BLOCK_SIZE..2 * CBOZ_BLOCK_SIZE).step_by(8) {
        let expected = 0xDEAD_BEEF_CAFE_F00Du64.wrapping_add(off);
        assert_eq!(
            read_u64(&mut ctx, data_addr + off),
            expected,
            "neighbour block must be untouched at +{off}"
        );
    }
}

/// Spec: the low bits of rs1 below `CBOZ_BLOCK_SIZE` are ignored. Passing
/// an unaligned address still zeros the block at `rs1 & !(BLOCK_SIZE - 1)`,
/// not the bytes from rs1 onward.
#[test]
fn cbo_zero_ignores_low_address_bits() {
    let block_addr = RAM_BASE + 0x1000;
    let unaligned = block_addr + 17; // mid-block
    let mut ctx = TestContext::new_with_config(&Config::default()).with_memory(RAM_SIZE, RAM_BASE);
    fill_pattern(&mut ctx, block_addr, 2 * CBOZ_BLOCK_SIZE);

    run_cbo_zero(&mut ctx, unaligned);

    for off in (0..CBOZ_BLOCK_SIZE).step_by(8) {
        assert_eq!(
            read_u64(&mut ctx, block_addr + off),
            0,
            "block at rs1 & !63 must be zeroed at +{off}"
        );
    }
    for off in (CBOZ_BLOCK_SIZE..2 * CBOZ_BLOCK_SIZE).step_by(8) {
        let expected = 0xDEAD_BEEF_CAFE_F00Du64.wrapping_add(off);
        assert_eq!(
            read_u64(&mut ctx, block_addr + off),
            expected,
            "next block must be untouched at +{off}"
        );
    }
}

/// Default M-mode access succeeds without setting menvcfg.CBZE.
#[test]
fn cbo_zero_in_machine_mode_ignores_menvcfg_cbze() {
    let data_addr = RAM_BASE + 0x1000;
    let mut ctx = TestContext::new_with_config(&Config::default()).with_memory(RAM_SIZE, RAM_BASE);
    assert_eq!(
        ctx.cpu().csrs.menvcfg & rvsim_core::core::arch::csr::MENVCFG_CBZE,
        0
    );
    fill_pattern(&mut ctx, data_addr, CBOZ_BLOCK_SIZE);

    run_cbo_zero(&mut ctx, data_addr);

    for off in (0..CBOZ_BLOCK_SIZE).step_by(8) {
        assert_eq!(read_u64(&mut ctx, data_addr + off), 0);
    }
}

/// Trap path: in U-mode with menvcfg.CBZE=0, cbo.zero must NOT zero the block.
#[test]
fn cbo_zero_traps_in_user_mode_when_cbze_clear() {
    use rvsim_core::core::arch::mode::PrivilegeMode;

    let data_addr = RAM_BASE + 0x1000;
    let mut ctx = TestContext::new_with_config(&Config::default()).with_memory(RAM_SIZE, RAM_BASE);
    fill_pattern(&mut ctx, data_addr, CBOZ_BLOCK_SIZE);
    let baseline: Vec<u64> = (0..CBOZ_BLOCK_SIZE)
        .step_by(8)
        .map(|off| read_u64(&mut ctx, data_addr + off))
        .collect();

    ctx.cpu_mut().privilege = PrivilegeMode::User;
    // Defaults: menvcfg.CBZE = 0, senvcfg.CBZE = 0 → trap.

    run_cbo_zero(&mut ctx, data_addr);

    for (i, off) in (0..CBOZ_BLOCK_SIZE).step_by(8).enumerate() {
        assert_eq!(
            read_u64(&mut ctx, data_addr + off),
            baseline[i],
            "block must NOT be zeroed when CBZE is clear in U-mode (off=+{off})"
        );
    }
}
