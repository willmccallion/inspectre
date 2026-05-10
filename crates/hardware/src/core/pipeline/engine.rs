//! Execution engine traits and pipeline type erasure.
//!
//! This module defines the trait hierarchy for pluggable backends:
//! 1. **`IssueUnit`** — stage-level trait for instruction issue (FIFO vs O3).
//! 2. **`ExecuteUnit`** — stage-level trait for instruction execution.
//! 3. **`ExecutionEngine`** — high-level trait covering the entire backend.
//! 4. **`PipelineDispatch`** — enum dispatch for type-erased pipeline storage.

use crate::core::pipeline::checkpoint::CheckpointTable;
use crate::core::pipeline::free_list::FreeList;
use crate::core::pipeline::latches::RenameIssueEntry;
use crate::core::pipeline::load_queue::LoadQueue;
use crate::core::pipeline::prf::PhysReg;
use crate::core::pipeline::prf::PhysRegFile;
use crate::core::pipeline::rename_map::RenameMap;
use crate::core::pipeline::rob::Rob;
use crate::core::pipeline::scoreboard::Scoreboard;
use crate::core::pipeline::snapshot::PipelineSnapshot;
use crate::core::pipeline::store_buffer::StoreBuffer;
use crate::core::pipeline::vec_prf::VecPhysRegFile;
use crate::core::units::vpu::types::VecPhysReg;
use serde::Deserialize;

/// Backend type selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum BackendType {
    /// In-order pipeline (default).
    #[default]
    InOrder,
    /// Out-of-order pipeline (future).
    OutOfOrder,
}

/// The execution engine trait — implemented by `InOrderEngine` (and `O3Engine` in the future).
///
/// Covers the backend pipeline: Issue -> Execute -> Memory1 -> Memory2 -> Writeback -> Commit.
pub trait ExecutionEngine {
    /// Run one cycle of all backend stages (reverse order internally).
    /// `redirect_pending` is set to `true` by the engine when an instruction
    /// flushes the frontend (branch misprediction, trap, FENCE.I, MRET/SRET).
    fn tick(
        &mut self,
        cpu: &mut crate::core::Cpu,
        rename_output: &mut Vec<RenameIssueEntry>,
        redirect_pending: &mut bool,
    );

    /// How many instructions can the engine accept from rename this cycle?
    fn can_accept(&self) -> usize;

    /// Flush all speculative state. Committed stores in the store buffer remain.
    fn flush(&mut self, cpu: &mut crate::core::Cpu);

    /// Read a CSR, checking in-flight `CsrUpdate` entries in the ROB.
    fn read_csr_speculative(&self, cpu: &crate::core::Cpu, addr: crate::common::CsrAddr) -> u64;

    /// Access the scoreboard (for rename to mark producers, issue to check readiness).
    fn scoreboard(&self) -> &Scoreboard;
    /// Access the scoreboard mutably (for rename to mark producers).
    fn scoreboard_mut(&mut self) -> &mut Scoreboard;

    /// Access the ROB (for rename to allocate entries, forwarding, etc.).
    fn rob(&self) -> &Rob;
    /// Access the ROB mutably (for rename to allocate entries).
    fn rob_mut(&mut self) -> &mut Rob;

    /// Access the store buffer (for rename to allocate, memory2 for forwarding).
    fn store_buffer(&self) -> &StoreBuffer;
    /// Access the store buffer mutably (for rename to allocate entries).
    fn store_buffer_mut(&mut self) -> &mut StoreBuffer;

    /// Access the speculative rename map (O3 only).
    fn rename_map(&self) -> &RenameMap {
        panic!("rename_map only available for O3 backend")
    }
    /// Access the speculative rename map mutably (O3 only).
    fn rename_map_mut(&mut self) -> &mut RenameMap {
        panic!("rename_map_mut only available for O3 backend")
    }

    /// Access the physical register file (O3 only).
    fn prf(&self) -> &PhysRegFile {
        panic!("prf only available for O3 backend")
    }
    /// Access the physical register file mutably (O3 only).
    fn prf_mut(&mut self) -> &mut PhysRegFile {
        panic!("prf_mut only available for O3 backend")
    }

    /// Access the free list (O3 only).
    fn free_list_mut(&mut self) -> &mut FreeList<PhysReg> {
        panic!("free_list_mut only available for O3 backend")
    }

    /// Access the load queue (O3 only). Returns None for in-order backend.
    fn load_queue_mut(&mut self) -> Option<&mut LoadQueue> {
        None
    }

    /// Returns true if this backend uses physical register renaming.
    fn has_prf(&self) -> bool {
        false
    }

    /// Returns true if this backend resolves intra-bundle RAW hazards via
    /// register renaming, so decode can skip the bundle-write hazard check.
    /// Default `false` (in-order); O3 overrides.
    fn has_register_renaming(&self) -> bool {
        false
    }

    /// Access the checkpoint table (O3 only).
    fn checkpoint_table(&self) -> &CheckpointTable {
        panic!("checkpoint_table only available for O3 backend")
    }
    /// Access the checkpoint table mutably (O3 only).
    fn checkpoint_table_mut(&mut self) -> &mut CheckpointTable {
        panic!("checkpoint_table_mut only available for O3 backend")
    }
    /// Returns the configured checkpoint count (0 = disabled).
    fn checkpoint_count(&self) -> usize {
        0
    }

    /// Access the vector physical register file (O3 only).
    fn vec_prf(&self) -> &VecPhysRegFile {
        panic!("vec_prf only available for O3 backend")
    }
    /// Access the vector physical register file mutably (O3 only).
    fn vec_prf_mut(&mut self) -> &mut VecPhysRegFile {
        panic!("vec_prf_mut only available for O3 backend")
    }
    /// Access the vector free list mutably (O3 only).
    fn vec_free_list_mut(&mut self) -> &mut FreeList<VecPhysReg> {
        panic!("vec_free_list_mut only available for O3 backend")
    }
}

/// The full pipeline combines a frontend and an engine.
///
/// The frontend is generic over the engine type, so the full pipeline
/// maintains both together.
#[derive(Debug)]
pub struct Pipeline<E: ExecutionEngine> {
    /// Frontend stages: fetch, decode, rename.
    pub frontend: crate::core::pipeline::frontend::Frontend<E>,
    /// Backend execution engine (in-order or out-of-order).
    pub engine: E,
    /// Buffer for rename stage output, consumed by the engine each cycle.
    pub rename_output: Vec<RenameIssueEntry>,
    /// Set by the backend (execute / commit) when a PC redirect occurs
    /// (branch misprediction, trap, FENCE.I, MRET/SRET). Read at the top
    /// of `tick` to decide whether to flush the frontend, then cleared.
    pub redirect_pending: bool,
}

impl<E: ExecutionEngine> Pipeline<E> {
    /// Run one cycle of the entire pipeline.
    pub fn tick(&mut self, cpu: &mut crate::core::Cpu) {
        let pc_before = cpu.hart.pc;

        self.engine.tick(cpu, &mut self.rename_output, &mut self.redirect_pending);

        // PC compare catches commit-stage redirects (MRET/SRET) that bypass execute's flush path.
        let needs_frontend_flush = self.redirect_pending || cpu.hart.pc != pc_before;
        self.redirect_pending = false;
        if needs_frontend_flush {
            self.frontend.flush();
            self.rename_output.clear();
        }

        if cpu.check_exit().is_none() && !cpu.hart.wfi_waiting {
            self.frontend.tick(cpu, &mut self.engine, &mut self.rename_output);
        }
    }

    /// Flush the entire pipeline.
    pub fn flush(&mut self, cpu: &mut crate::core::Cpu) {
        self.frontend.flush();
        self.rename_output.clear();
        self.engine.flush(cpu);
    }
}

/// Type-erased pipeline for storage in the non-generic Cpu struct.
#[derive(Debug)]
pub enum PipelineDispatch {
    /// In-order pipeline.
    InOrder(Box<Pipeline<crate::core::pipeline::backend::inorder::InOrderEngine>>),
    /// Out-of-order pipeline.
    OutOfOrder(Box<Pipeline<crate::core::pipeline::backend::o3::O3Engine>>),
}

impl PipelineDispatch {
    /// Run one cycle.
    pub fn tick(&mut self, cpu: &mut crate::core::Cpu) {
        match self {
            Self::InOrder(p) => p.tick(cpu),
            Self::OutOfOrder(p) => p.tick(cpu),
        }
    }

    /// Flush.
    pub fn flush(&mut self, cpu: &mut crate::core::Cpu) {
        match self {
            Self::InOrder(p) => p.flush(cpu),
            Self::OutOfOrder(p) => p.flush(cpu),
        }
    }

    /// Capture a point-in-time snapshot of all inter-stage latch contents.
    pub fn snapshot(&self, width: usize) -> PipelineSnapshot {
        match self {
            Self::InOrder(p) => PipelineSnapshot {
                fetch1_fetch2: p.frontend.fetch1_fetch2.clone(),
                fetch2_decode: p.frontend.fetch2_decode.clone(),
                decode_rename: p.frontend.decode_rename.clone(),
                rename_issue: p.rename_output.clone(),
                issue_queue: p.engine.issuer.queue_snapshot(),
                execute_mem1: p.engine.execute_mem1.clone(),
                mem1_mem2: p.engine.mem1_mem2.clone(),
                mem2_wb: p.engine.mem2_wb.clone(),
                fetch1_stall: p.frontend.fetch1_stall,
                fetch2_stall: p.frontend.fetch2_stall,
                mem1_stall: p.engine.mem1_stall,
                width,
            },
            Self::OutOfOrder(p) => PipelineSnapshot {
                fetch1_fetch2: p.frontend.fetch1_fetch2.clone(),
                fetch2_decode: p.frontend.fetch2_decode.clone(),
                decode_rename: p.frontend.decode_rename.clone(),
                rename_issue: p.rename_output.clone(),
                issue_queue: p.engine.issue_queue.queue_snapshot(),
                execute_mem1: p.engine.execute_mem1.clone(),
                mem1_mem2: p.engine.mem1_mem2.clone(),
                mem2_wb: p.engine.mem2_wb.clone(),
                fetch1_stall: p.frontend.fetch1_stall,
                fetch2_stall: p.frontend.fetch2_stall,
                mem1_stall: 0, // O3 uses per-entry complete_cycle, no global stall
                width,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::pipeline::backend::inorder::InOrderEngine;
    use crate::core::pipeline::frontend::Frontend;

    #[test]
    fn test_backend_type_default() {
        assert_eq!(BackendType::default(), BackendType::InOrder);
    }

    #[test]
    fn test_pipeline_dispatch_inorder_tick_flush_snapshot() {
        let config = crate::config::Config::default();
        let mut cpu = crate::core::Cpu::build(&config, "");

        let frontend = Frontend::new(config.pipeline.width);
        let engine = InOrderEngine::new(&config);
        let pipeline = Pipeline {
            frontend,
            engine,
            rename_output: Vec::new(),
            redirect_pending: false,
        };
        let mut dispatch = PipelineDispatch::InOrder(Box::new(pipeline));

        dispatch.tick(&mut cpu);
        dispatch.flush(&mut cpu);
        let snapshot = dispatch.snapshot(1);
        assert_eq!(snapshot.width, 1);
    }
}
