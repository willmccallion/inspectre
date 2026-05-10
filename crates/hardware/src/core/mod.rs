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
use crate::config::{Config, InclusionPolicy};
use crate::core::pipeline::write_buffer::WriteCombiningBuffer;
use crate::core::units::bru::BranchPredictorWrapper;
use crate::core::units::cache::CacheSim;
use crate::core::units::cache::mshr::MshrFile;
use crate::core::units::prefetch::PrefetchFilter;

/// A physical processor core hosting one or more harts.
///
/// Owns the hardware shared across the harts running on it: pipeline-private
/// caches (L1 instruction, L1 data, L2), load/store MSHRs, write-combining
/// buffer, branch predictor, and prefetch filter. Hosts one or more
/// [`Hart`]s — one for a non-SMT core, two or more for SMT.
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
    /// Cache inclusion policy (Inclusive / Exclusive / NINE).
    pub inclusion_policy: InclusionPolicy,
    /// Write Combining Buffer for store coalescing.
    pub wcb: WriteCombiningBuffer,
    /// Shared prefetch filter to deduplicate prefetch requests across cache levels.
    pub prefetch_filter: PrefetchFilter,
    /// Branch Predictor Unit.
    pub branch_predictor: BranchPredictorWrapper,
    /// Pipeline width (superscalar degree).
    pub pipeline_width: usize,
    /// Maximum element width in bits (`ELEN`). Configurable for now;
    /// polish commit at end of Phase A will hardcode to the implementation
    /// max since the simulator does not actually downgrade behaviour.
    pub elen: usize,
    /// Whether the Zvfh (half-precision vector FP) extension is enabled.
    /// See note on `elen`.
    pub zvfh: bool,
    /// Whether this core's pipeline uses register renaming. Polish commit
    /// will move to a method on the pipeline backend.
    pub has_register_renaming: bool,
    /// L1I line size in bytes (cached for fetch alignment). Polish commit
    /// will derive from `l1_i_cache.line_bytes()` instead.
    pub i_cache_line_bytes: usize,
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
            inclusion_policy: config.cache.inclusion_policy,
            wcb: WriteCombiningBuffer::new(config.cache.wcb_entries, config.cache.l1_d.line_bytes),
            prefetch_filter: PrefetchFilter::new(prefetch_window, config.cache.l1_d.line_bytes),
            branch_predictor: BranchPredictorWrapper::new(config),
            pipeline_width: config.pipeline.width,
            elen: config.pipeline.elen,
            zvfh: config.pipeline.zvfh,
            has_register_renaming: config.pipeline.backend
                == crate::core::pipeline::engine::BackendType::OutOfOrder,
            i_cache_line_bytes: config.cache.l1_i.line_bytes.max(1),
        }
    }
}

