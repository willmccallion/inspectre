//! Simulator: owns the CPU, the pipeline, and the global event queue.
//!
//! Each `tick()`:
//! 1. Increments the cycle in `pre_tick`.
//! 2. Drains events scheduled for the new cycle, delivering packets to
//!    pipelines / caches / bus / memory controllers.
//! 3. Runs one cycle of the pipeline (mailbox-drain at the top, then
//!    engine.tick, then frontend.tick).
//! 4. Drains again so packets the pipeline just emitted reach their
//!    targets this cycle. The cache / bus / mem-controller handlers
//!    schedule their responses for future cycles; those land in the
//!    pipeline's mailbox via the next cycle's start-of-tick drain.
//! 5. Runs `post_tick` for mode tracing.
//!
//! Memory traffic (instruction fetch, load, store, page-table walk) flows
//! exclusively through scheduled `MemReq` / `MemResp` packets.

use crate::common::SimError;
use crate::config::Config;
use crate::core::Cpu;
use crate::core::pipeline::backend::inorder::InOrderEngine;
use crate::core::pipeline::backend::o3::O3Engine;
use crate::core::pipeline::engine::{BackendType, Pipeline, PipelineDispatch};
use crate::core::pipeline::frontend::Frontend;
use crate::sim::components::{CacheId, ComponentId, MemCtrlId, PipelineId};
use crate::sim::events::Event;
use crate::sim::handle::{Handle, HandleCtx};
use crate::sim::packet::Packet;
use crate::soc::Soc;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

/// Default `CacheId` for the L1 instruction cache in a single-core config.
const L1I_CACHE_ID: CacheId = CacheId::new(0);
/// Default `CacheId` for the L1 data cache in a single-core config.
const L1D_CACHE_ID: CacheId = CacheId::new(1);
/// Default `PipelineId` for the single-core pipeline.
const PIPELINE_ID: PipelineId = PipelineId::new(0);

/// Top-level simulator: CPU architectural state + pipeline + scheduler.
#[derive(Debug)]
pub struct Simulator {
    /// CPU architectural state (registers, caches, MMU, `SoC`, stats).
    pub cpu: Cpu,
    /// Pipeline implementation (frontend + backend engine).
    pub pipeline: PipelineDispatch,
}

unsafe impl Send for Simulator {}
unsafe impl Sync for Simulator {}

impl Simulator {
    /// Creates a new simulator with the given `SoC` and configuration.
    /// `exit_signal` must be the same `Arc` cloned into bus-resident
    /// devices when `Soc` was constructed; HTIF / `SysCon` writes propagate
    /// via this slot.
    pub fn new(soc: Soc, config: &Config, exit_signal: Arc<AtomicU64>) -> Self {
        let cpu = Cpu::new(soc, config, exit_signal);
        let pipeline = match config.pipeline.backend {
            BackendType::InOrder => PipelineDispatch::InOrder(Box::new(Pipeline {
                frontend: Frontend::new(config.pipeline.width),
                engine: InOrderEngine::new(config, PIPELINE_ID, L1I_CACHE_ID, L1D_CACHE_ID),
                rename_output: Vec::with_capacity(config.pipeline.width),
                redirect_pending: false,
            })),
            BackendType::OutOfOrder => PipelineDispatch::OutOfOrder(Box::new(Pipeline {
                frontend: Frontend::new(config.pipeline.width),
                engine: O3Engine::new(config, PIPELINE_ID, L1I_CACHE_ID, L1D_CACHE_ID),
                rename_output: Vec::with_capacity(config.pipeline.width),
                redirect_pending: false,
            })),
        };
        Self { cpu, pipeline }
    }

    /// Synchronize the architectural register file into the O3 PRF.
    ///
    /// Must be called after all register initialization (loader setup, etc.)
    /// but before the first pipeline tick. For the in-order backend this is a no-op.
    pub fn sync_arch_regs(&mut self) {
        if let PipelineDispatch::OutOfOrder(ref mut p) = self.pipeline {
            p.engine.sync_arch_regs(&self.cpu);
        }
    }

    /// Advances the simulator by one clock cycle.
    ///
    /// # Errors
    ///
    /// Returns [`SimError::HangDetected`] if the PC has not advanced for too many
    /// consecutive cycles (and is not stuck in a WFI spin-wait).
    ///
    /// Returns [`SimError::KernelPanic`] if the guest OS panic sentinel fires.
    pub fn tick(&mut self) -> Result<(), SimError> {
        let prev_priv = self.cpu.hart.privilege;
        let skip = self.cpu.pre_tick()?;
        // First drain: deliver events scheduled for cycles <= now into
        // their targets (filling pipeline.mailbox with responses from
        // previous cycles' emissions).
        self.drain_events();
        if !skip {
            self.pipeline.tick(&mut self.cpu);
        }
        // Second drain: events the pipeline just scheduled (MemReqs to L1)
        // reach their target component handlers this cycle so the next
        // cycle's start-of-tick drain delivers their responses.
        self.drain_events();
        self.cpu.post_tick(prev_priv);
        Ok(())
    }

    /// Dispatches every event with `fire_at <= self.cpu.soc.cycle`.
    fn drain_events(&mut self) {
        let cycle = self.cpu.soc.cycle;
        while let Some(event) = self.cpu.event_queue.pop_ready(cycle) {
            self.dispatch(event);
        }
    }

    /// Routes a single event to its target component.
    fn dispatch(&mut self, event: Event) {
        let Event { fire_at: _, seq: _, target, source, packet } = event;
        match target {
            ComponentId::Pipeline(_) => {
                self.pipeline.deliver(source, packet);
            }
            ComponentId::Cache(id) => {
                dispatch_to_cache(&mut self.cpu, id, packet, source);
            }
            ComponentId::Bus => {
                let mut ctx = build_ctx(&mut self.cpu, ComponentId::Bus);
                self.cpu.soc.bus.handle(packet, source, &mut ctx);
            }
            ComponentId::MemCtrl(id) => {
                let mut ctx = build_ctx(&mut self.cpu, ComponentId::MemCtrl(id));
                self.cpu.soc.mem_controller.handle(packet, source, &mut ctx);
            }
            ComponentId::Device(_) | ComponentId::Hart(_) | ComponentId::Core(_) => {
                // Devices are routed via Bus; Hart / Core targeting is
                // reserved for future multi-core / coherence packets.
            }
        }
    }

    /// Retrieves the exit code if the simulation has finished.
    pub fn take_exit(&self) -> Option<u64> {
        self.cpu.take_exit()
    }
}

/// Dispatches a packet to the cache identified by `id`.
fn dispatch_to_cache(cpu: &mut Cpu, id: CacheId, packet: Packet, source: ComponentId) {
    let self_id = ComponentId::Cache(id);
    let cycle = cpu.soc.cycle;
    let mut ctx = HandleCtx {
        scheduler: &mut cpu.event_queue,
        stats: &mut cpu.stats_hier,
        config: &cpu.config,
        cycle,
        self_id,
    };
    match id {
        id if id == CacheId::new(0) => cpu.core.l1_i_cache.handle(packet, source, &mut ctx),
        id if id == CacheId::new(1) => cpu.core.l1_d_cache.handle(packet, source, &mut ctx),
        id if id == CacheId::new(2) => cpu.core.l2_cache.handle(packet, source, &mut ctx),
        id if id == CacheId::new(3) => cpu.soc.l3_cache.handle(packet, source, &mut ctx),
        _ => {}
    }
}

fn build_ctx(cpu: &mut Cpu, self_id: ComponentId) -> HandleCtx<'_> {
    HandleCtx {
        scheduler: &mut cpu.event_queue,
        stats: &mut cpu.stats_hier,
        config: &cpu.config,
        cycle: cpu.soc.cycle,
        self_id,
    }
}

/// Suppress unused-import warnings until the `MemCtrlId` constant is used
/// for routing in a follow-up.
const _: fn() = || {
    let _ = MemCtrlId::new(0);
};
