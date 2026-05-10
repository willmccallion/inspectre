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
use crate::config::Config;
use crate::core::pipeline::write_buffer::WriteCombiningBuffer;
use crate::core::units::bru::BranchPredictorWrapper;
use crate::core::units::cache::CacheSim;
use crate::core::units::cache::mshr::MshrFile;
use crate::core::units::prefetch::PrefetchFilter;

/// A physical processor core hosting one or more harts.
///
/// Owns the runtime hardware shared across the harts running on it:
/// pipeline-private caches (L1 instruction, L1 data, L2), load/store MSHRs,
/// write-combining buffer, branch predictor, and prefetch filter. Hosts one
/// or more [`Hart`]s — one for a non-SMT core, two or more for SMT.
///
/// Configuration-derived constants (`pipeline_width`, ELEN/Zvfh, inclusion
/// policy, …) are NOT cached here — they're read on demand from
/// `soc.config`. Pipeline properties like register-renaming are queried via
/// the [`ExecutionEngine`](crate::core::pipeline::engine::ExecutionEngine)
/// trait. `i_cache_line_bytes` is read via [`CacheSim::line_bytes()`].
#[derive(Debug)]
pub struct Core {
    /// Identifier for this physical core within the `SoC`.
    pub core_id: CoreId,
    /// L1 Instruction Cache.
    pub l1_i_cache: CacheSim,
    /// L1 Data Cache.
    pub l1_d_cache: CacheSim,
    /// L2 Unified Cache.
    pub l2_cache: CacheSim,
    /// L1D MSHR file for non-blocking cache access (O3 backend only).
    pub l1d_mshrs: MshrFile,
    /// Write Combining Buffer for store coalescing.
    pub wcb: WriteCombiningBuffer,
    /// Shared prefetch filter to deduplicate prefetch requests across cache levels.
    pub prefetch_filter: PrefetchFilter,
    /// Branch Predictor Unit.
    pub branch_predictor: BranchPredictorWrapper,
}

impl Core {
    /// Creates a new `Core` from configuration.
    pub fn new(core_id: CoreId, config: &Config) -> Self {
        let prefetch_window = if config.cache.l1_d.prefetcher != crate::config::Prefetcher::None
            || config.cache.l2.prefetcher != crate::config::Prefetcher::None
        {
            64
        } else {
            0
        };
        Self {
            core_id,
            l1_i_cache: CacheSim::new(&config.cache.l1_i),
            l1_d_cache: CacheSim::new(&config.cache.l1_d),
            l2_cache: CacheSim::new(&config.cache.l2),
            l1d_mshrs: MshrFile::new(config.cache.l1_d.mshr_count, config.cache.l1_d.line_bytes),
            wcb: WriteCombiningBuffer::new(config.cache.wcb_entries, config.cache.l1_d.line_bytes),
            prefetch_filter: PrefetchFilter::new(prefetch_window, config.cache.l1_d.line_bytes),
            branch_predictor: BranchPredictorWrapper::new(config),
        }
    }
}

