//! Simulator: owns the CPU, the pipeline, and the global event queue.
//!
//! Each `tick()` drains the event queue first — delivering every packet whose
//! `fire_at <= cycle` to its target component — and then runs the pipeline.
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
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

/// Default `CacheId` assigned to the L1 instruction cache for a single core.
const L1I_CACHE_ID: CacheId = CacheId::new(0);
/// Default `CacheId` assigned to the L1 data cache for a single core.
const L1D_CACHE_ID: CacheId = CacheId::new(1);

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
        let pipeline_id = PipelineId::new(0);
        let pipeline = match config.pipeline.backend {
            BackendType::InOrder => PipelineDispatch::InOrder(Box::new(Pipeline {
                frontend: Frontend::new(config.pipeline.width),
                engine: InOrderEngine::new(config),
                rename_output: Vec::with_capacity(config.pipeline.width),
                redirect_pending: false,
                pipeline_id,
                mailbox: Vec::new(),
                outstanding_fetches: HashMap::new(),
                outstanding_loads: HashMap::new(),
                outstanding_stores: HashMap::new(),
                outstanding_walks: HashMap::new(),
                next_req_id: 0,
                l1_i_id: L1I_CACHE_ID,
                l1_d_id: L1D_CACHE_ID,
            })),
            BackendType::OutOfOrder => PipelineDispatch::OutOfOrder(Box::new(Pipeline {
                frontend: Frontend::new(config.pipeline.width),
                engine: O3Engine::new(config),
                rename_output: Vec::with_capacity(config.pipeline.width),
                redirect_pending: false,
                pipeline_id,
                mailbox: Vec::new(),
                outstanding_fetches: HashMap::new(),
                outstanding_loads: HashMap::new(),
                outstanding_stores: HashMap::new(),
                outstanding_walks: HashMap::new(),
                next_req_id: 0,
                l1_i_id: L1I_CACHE_ID,
                l1_d_id: L1D_CACHE_ID,
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
    /// 1. Drains the event queue: every event with `fire_at <= cycle` is
    ///    dispatched to its target component. Packets aimed at the pipeline
    ///    land in its mailbox; packets aimed at caches, memory controllers,
    ///    devices, or the bus invoke their `Handle::handle` impl.
    /// 2. Runs the CPU pre-tick (interrupt updates, hang detection).
    /// 3. Runs one cycle of the pipeline.
    /// 4. Runs the CPU post-tick (privilege tracing, status).
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
        self.drain_events();
        if !skip {
            self.pipeline.tick(&mut self.cpu);
        }
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
                // Hart / Core targeting is unused in Phase 2; device targeting
                // is always routed via Bus rather than direct.
            }
        }
    }

    /// Retrieves the exit code if the simulation has finished.
    pub fn take_exit(&self) -> Option<u64> {
        self.cpu.take_exit()
    }
}

/// Dispatches a packet to one of the caches lookup by `CacheId`.
fn dispatch_to_cache(cpu: &mut Cpu, id: CacheId, packet: Packet, source: ComponentId) {
    let self_id = ComponentId::Cache(id);
    match id {
        id if id == CacheId::new(0) => {
            let mut ctx = build_ctx_with_self(&mut cpu.event_queue, &mut cpu.stats_hier, &cpu.config, cpu.soc.cycle, self_id);
            cpu.core.l1_i_cache.handle(packet, source, &mut ctx);
        }
        id if id == CacheId::new(1) => {
            let mut ctx = build_ctx_with_self(&mut cpu.event_queue, &mut cpu.stats_hier, &cpu.config, cpu.soc.cycle, self_id);
            cpu.core.l1_d_cache.handle(packet, source, &mut ctx);
        }
        id if id == CacheId::new(2) => {
            let mut ctx = build_ctx_with_self(&mut cpu.event_queue, &mut cpu.stats_hier, &cpu.config, cpu.soc.cycle, self_id);
            cpu.core.l2_cache.handle(packet, source, &mut ctx);
        }
        id if id == CacheId::new(3) => {
            let mut ctx = build_ctx_with_self(&mut cpu.event_queue, &mut cpu.stats_hier, &cpu.config, cpu.soc.cycle, self_id);
            cpu.soc.l3_cache.handle(packet, source, &mut ctx);
        }
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

fn build_ctx_with_self<'a>(
    queue: &'a mut crate::sim::events::EventQueue,
    stats: &'a mut crate::sim::stats::Stats,
    config: &'a Config,
    cycle: u64,
    self_id: ComponentId,
) -> HandleCtx<'a> {
    HandleCtx { scheduler: queue, stats, config, cycle, self_id }
}

/// Suppress an unused-import lint until the `MemCtrlId` constant is used
/// for routing in a follow-up.
const _: fn() = || {
    let _ = MemCtrlId::new(0);
};
