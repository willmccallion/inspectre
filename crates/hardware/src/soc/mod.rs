//! System-on-Chip (SoC) Components.
//!
//! This module organizes the components that make up the simulated system,
//! including the system bus, memory controllers, devices, and the builder
//! for assembling the system.

/// System builder for assembling SoC components.
pub mod builder;

/// Memory-mapped I/O device implementations.
pub mod devices;

/// System bus interconnect and routing.
pub mod interconnect;

/// Memory controller implementations.
pub mod memory;

/// Device trait definitions for MMIO access.
pub mod traits;

pub use builder::System;

/// The simulated System-on-Chip.
///
/// Owns the cores, shared last-level cache(s), coherence controller, memory
/// subsystem, and the IO bus with its memory-mapped devices. Carries the
/// master cycle counter that every subsystem reads from for time-correlated
/// state (e.g. CLINT computes `mtime = cycle / divider`).
///
/// Currently a placeholder; fields are added as state migrates out of the
/// CPU and bus.
#[derive(Debug, Default)]
pub struct Soc {
    /// Master clock; every subsystem reads from this.
    pub cycle: u64,
}

impl Soc {
    /// Creates a new `Soc` with the cycle counter initialised to zero.
    #[inline]
    pub const fn new() -> Self {
        Self { cycle: 0 }
    }
}
