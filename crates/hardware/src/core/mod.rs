//! Core processor implementation.
//!
//! This module contains the main CPU implementation including the instruction
//! pipeline, execution units, architecture-specific components, and the
//! orchestrator that coordinates all components.

/// Architecture-specific components (CSRs, register files, privilege modes, traps).
pub mod arch;

/// CPU core implementation and execution orchestration.
pub mod cpu;

/// Per-thread RISC-V architectural state.
pub mod hart;

/// Instruction pipeline implementation (10-stage, latches, signals).
pub mod pipeline;

/// Execution units (ALU, FPU, LSU, MMU, branch predictor, cache, prefetcher).
pub mod units;

pub use self::cpu::Cpu;
pub use self::hart::Hart;

use crate::common::CoreId;

/// A physical processor core hosting one or more harts.
///
/// Owns the hardware shared across the harts running on it: the pipeline,
/// SMT scheduler, branch predictor, L1 instruction and data caches, the L2,
/// load/store MSHRs, and the write-combining buffer. Hosts one or more
/// [`Hart`]s — one for a non-SMT core, two or more for SMT.
///
/// Currently a placeholder; fields are added as state migrates out of [`Cpu`].
#[derive(Debug)]
pub struct Core {
    /// Identifier for this physical core within the `SoC`.
    pub core_id: CoreId,
}

impl Core {
    /// Creates a new `Core` bound to the given core identifier.
    #[inline]
    pub const fn new(core_id: CoreId) -> Self {
        Self { core_id }
    }
}
