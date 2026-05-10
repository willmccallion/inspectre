//! RISC-V Zicboz (cache-block zero) extension constants.
//!
//! Zicboz adds a single instruction, `cbo.zero rs1`, encoded in the MISC-MEM
//! group with funct3 = CBO. The 12-bit immediate selects the specific CBO
//! operation; `cbo.zero` uses 0x004.
//!
//! Zicbom (cache-block management: `cbo.inval` / `cbo.clean` / `cbo.flush`)
//! shares the funct3=CBO encoding. We don't simulate cache coherence at a
//! granularity that lets Zicbom do anything observable, so those instructions
//! are accepted by decode and retire as no-ops. This keeps Linux DMA paths
//! that issue cbo.flush around their buffers from trapping.

/// `imm[11:0]` value that selects `cbo.zero` within the MISC-MEM CBO group.
pub const CBO_ZERO_IMM: i64 = 0x004;

/// `imm[11:0]` for `cbo.inval` (Zicbom). No-op in this simulator.
pub const CBO_INVAL_IMM: i64 = 0x000;

/// `imm[11:0]` for `cbo.clean` (Zicbom). No-op in this simulator.
pub const CBO_CLEAN_IMM: i64 = 0x001;

/// `imm[11:0]` for `cbo.flush` (Zicbom). No-op in this simulator.
pub const CBO_FLUSH_IMM: i64 = 0x002;

/// Cache-block size in bytes operated on by `cbo.zero`.
///
/// Implementation-defined per the spec; we pick 64 to match the L1D line.
/// Software discovers the value via the `riscv,cboz-block-size` device-tree
/// property — no CSR exposes it directly.
pub const CBOZ_BLOCK_SIZE: u64 = 64;
