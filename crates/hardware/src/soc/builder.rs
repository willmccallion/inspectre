//! `Soc` construction.
//!
//! Builds the simulated System-on-Chip from a [`Config`]: registers the IO
//! interconnect, memory controller, and devices (CLINT, PLIC, UART, virtio,
//! HTIF, syscon, RTC). The pipeline-side state (cores, shared caches,
//! coherence) is added in later phases.

use crate::config::{Config, MemoryController as MemControllerType};
use crate::core::units::cache::CacheSim;
use crate::soc::devices::{Clint, GoldfishRtc, Htif, Plic, SysCon, Uart, VirtioBlock};
use crate::soc::interconnect::Bus;
use crate::soc::memory::buffer::DramBuffer;
use crate::soc::memory::controller::{
    DramConfig, DramController, MemoryController, SimpleController,
};
use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

/// The simulated System-on-Chip.
///
/// Owns the IO interconnect, the memory controller, the shared last-level
/// cache, and the master cycle counter that every subsystem reads from for
/// time-correlated state (e.g. CLINT computes `mtime = cycle / divider`).
///
/// The simulator [`Config`] lives on `Cpu` (transitionally, then on
/// `Simulator`); `Soc` does not hold it because configuration is bench
/// metadata, not hardware state.
///
/// Cores and coherence are added in later phases of the multi-core migration.
pub struct Soc {
    /// Master clock; every subsystem reads from this.
    pub cycle: u64,
    /// IO interconnect; routes accesses to RAM and MMIO devices.
    pub bus: Bus,
    /// Main memory controller.
    pub mem_controller: MemoryController,
    /// Shared L3 cache (last-level cache; future shared LLC for multi-core).
    pub l3_cache: CacheSim,
}

impl std::fmt::Debug for Soc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Soc")
            .field("cycle", &self.cycle)
            .field("bus", &self.bus)
            .field("l3_cache", &self.l3_cache)
            .finish_non_exhaustive()
    }
}

impl Soc {
    /// Builds a new `Soc` from configuration and optional disk image path.
    /// `exit_signal` is cloned into bus-resident devices (`SysCon`, HTIF) so
    /// they can write the harness termination value when triggered.
    pub fn new(config: &Config, disk_path: &str, exit_signal: &Arc<AtomicU64>) -> Self {
        let mut bus = Bus::new(config.system.bus_width, config.system.bus_latency);

        let ram_base = config.system.ram_base;
        let ram_size = config.memory.ram_size;
        let ram_buffer = Arc::new(DramBuffer::new(ram_size));

        let uart_base = config.system.uart_base;
        let uart = Uart::new(uart_base, config.system.uart_to_stderr, config.system.uart_quiet);

        let clint_addr = config.system.clint_base;
        let clint = Clint::new(clint_addr, config.system.clint_divider);

        let plic_addr = 0x0c00_0000;
        let plic = Plic::new(plic_addr);

        let disk_base = config.system.disk_base;
        let mut disk = VirtioBlock::new(disk_base, ram_base, ram_buffer.clone());
        if !disk_path.is_empty()
            && let Ok(disk_data) = fs::read(disk_path)
            && !disk_data.is_empty()
        {
            disk.load(disk_data);
        }

        let syscon_addr = config.system.syscon_base;
        let syscon = SysCon::new(syscon_addr, exit_signal.clone());

        let rtc = GoldfishRtc::new(0x101000);

        bus.add_device(Box::new(uart));
        bus.add_device(Box::new(disk));
        bus.add_device(Box::new(clint));
        bus.add_device(Box::new(plic));
        bus.add_device(Box::new(syscon));
        bus.add_device(Box::new(rtc));

        if config.system.tohost_addr != 0 {
            let htif = Htif::new(config.system.tohost_addr, exit_signal.clone());
            bus.add_device(Box::new(htif));
        }

        let mem_controller = match config.memory.controller {
            MemControllerType::Dram => MemoryController::Dram(DramController::new(
                ram_buffer.clone(),
                crate::common::PhysAddr::new(ram_base),
                DramConfig {
                    t_cas: config.memory.t_cas,
                    t_ras: config.memory.t_ras,
                    t_pre: config.memory.t_pre,
                    t_rrd: config.memory.t_rrd,
                    num_banks: config.memory.num_banks,
                    row_size_bytes: config.memory.row_size_bytes,
                    t_refi: config.memory.t_refi,
                    t_rfc: config.memory.t_rfc,
                },
            )),
            MemControllerType::Simple => MemoryController::Simple(SimpleController::new(
                ram_buffer.clone(),
                crate::common::PhysAddr::new(ram_base),
                config.memory.row_miss_latency,
            )),
        };

        let l3_cache = CacheSim::new(&config.cache.l3);

        Self { cycle: 0, bus, mem_controller, l3_cache }
    }

    /// Loads a binary into memory at the given physical address.
    pub fn load_binary_at(&mut self, data: &[u8], addr: crate::common::PhysAddr) {
        self.bus.load_binary_at(data, addr);
    }

    /// Advances all devices by one tick and returns this cycle's interrupt
    /// snapshot.
    pub fn tick(&mut self) -> crate::soc::interconnect::BusIrqs {
        self.bus.tick()
    }

    /// Checks whether the kernel has signaled panic via UART.
    pub fn check_kernel_panic(&mut self) -> bool {
        self.bus.check_kernel_panic()
    }

    /// Registers an HTIF device at the given tohost address. `exit_signal`
    /// is cloned into the device so HTIF tohost writes propagate up to the
    /// harness.
    pub fn add_htif(&mut self, tohost_addr: u64, exit_signal: &Arc<AtomicU64>) {
        let htif = Htif::new(tohost_addr, exit_signal.clone());
        self.bus.add_device(Box::new(htif));
    }
}
