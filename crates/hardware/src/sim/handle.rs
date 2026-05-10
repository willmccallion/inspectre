//! The `Handle` trait every event-receiving component implements, and the
//! `HandleCtx` borrow bundle passed to it on dispatch.
//!
//! A component reacts to incoming packets by mutating its own state and
//! scheduling outgoing events through `ctx.scheduler`. It does not reach
//! into other components — cross-component effects are always packet sends.

use crate::config::Config;
use crate::sim::components::ComponentId;
use crate::sim::events::EventQueue;
use crate::sim::packet::Packet;
use crate::sim::stats::Stats;

/// Borrow bundle passed to every `Handle::handle` invocation.
///
/// Carries the bench-side scaffolding a component might need: the event
/// scheduler (to schedule outgoing packets), the stats sink, the simulation
/// config (read-only), and the current cycle.
#[derive(Debug)]
pub struct HandleCtx<'a> {
    /// Outgoing event scheduler.
    pub scheduler: &'a mut EventQueue,
    /// Component-rooted stats sink.
    pub stats: &'a mut Stats,
    /// Simulator configuration (read-only).
    pub config: &'a Config,
    /// The cycle at which this event is being delivered.
    pub cycle: u64,
}

/// Trait every event-driven component implements.
pub trait Handle {
    /// React to `packet` arriving from `source`. May mutate `self` and schedule
    /// further events through `ctx.scheduler`.
    fn handle(&mut self, packet: Packet, source: ComponentId, ctx: &mut HandleCtx<'_>);
}
