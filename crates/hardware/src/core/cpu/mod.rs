//! CPU Core Definition and Initialization.
//!
//! Defines the central `Cpu` structure holding all architectural processor
//! state. The pipeline lives separately in `Simulator`; this struct owns
//! registers, MMU, caches, and the system bus.

/// Control and Status Register access and management.
pub mod csr;

/// Instruction execution orchestration and pipeline coordination.
pub mod execution;

/// Memory access handling and load/store operations.
pub mod memory;

/// Trap and exception handling logic.
pub mod trap;

use crate::common::{CoreId, HartId, PhysAddr, RegisterFile};
use crate::config::Config;
use crate::core::arch::csr::Csrs;
use crate::core::arch::mode::PrivilegeMode;
use crate::core::hart::HartInit;
use crate::core::units::mmu::Mmu;
use crate::core::units::mmu::pmp::Pmp;
use crate::core::{Core, Hart};
use crate::sim::per_hart_debug::HartDebug;
use crate::soc::Soc;
use crate::stats::SimStats;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// CPU architectural state: registers, caches, MMU, bus, and statistics.
///
/// The pipeline is owned by `Simulator`, not by `Cpu`. This struct holds only
/// the architectural state that the pipeline reads and writes.
#[derive(Debug)]
pub struct Cpu {
    /// Per-thread architectural state (registers, CSRs, PC, MMU, PMP, ...).
    pub hart: Hart,
    /// Pipeline-private state shared by harts on this core (caches, MSHRs,
    /// branch predictor, prefetch filter, write-combining buffer).
    pub core: Core,

    /// System-on-Chip: config, bus, memory controller, shared L3, master clock.
    pub soc: Soc,

    /// Sim-side per-hart debug bookkeeping (hang detection, panic timing,
    /// retire trace). Indexed by `HartId`; transitionally lives here until
    /// the bench view replaces direct `Cpu` access.
    pub per_hart_debug: Vec<HartDebug>,

    /// Sim-side perf observability counters.
    pub stats: SimStats,
    /// Optional buffered writer for the commit log (enabled by the
    /// `commit-log` feature).
    #[cfg(feature = "commit-log")]
    pub commit_log: Option<std::io::BufWriter<std::fs::File>>,
    /// Atomic slot bus-resident devices (HTIF, `SysCon`) write the harness
    /// termination value into. `u64::MAX` means "no exit pending".
    pub exit_signal: Arc<AtomicU64>,
    /// Latched termination value the harness returns from `take_exit`.
    pub exit_code: Option<u64>,

    /// Direct mode (no translation, flat memory). Initialised from
    /// `config.general.direct_mode` and runtime-mutable (the ELF loader
    /// writes it after init).
    pub direct_mode: bool,

    /// Set by the backend when a PC redirect occurs (branch misprediction,
    /// trap, FENCE.I, etc.). The pipeline uses this to flush the frontend,
    /// rather than relying solely on `cpu.hart.pc != pc_before` which can miss
    /// redirects when the target happens to equal the current fetch PC.
    pub redirect_pending: bool,
}

unsafe impl Send for Cpu {}
unsafe impl Sync for Cpu {}

impl Cpu {
    /// Cache line size for reservation granularity (64 bytes)
    const RESERVATION_GRANULE: u64 = 64;

    /// Aligns an address to the reservation granule (cache line boundary)
    #[inline]
    const fn align_reservation_address(addr: PhysAddr) -> PhysAddr {
        PhysAddr(addr.0 & !(Self::RESERVATION_GRANULE - 1))
    }

    /// Sets a load reservation at the given address (cache-line aligned)
    #[inline]
    pub(crate) const fn set_reservation(&mut self, addr: PhysAddr) {
        self.hart.load_reservation = Some(Self::align_reservation_address(addr));
    }

    /// Checks if a reservation exists for the given address
    #[inline]
    pub(crate) const fn check_reservation(&self, addr: PhysAddr) -> bool {
        if let Some(reserved_addr) = self.hart.load_reservation {
            reserved_addr.0 == Self::align_reservation_address(addr).0
        } else {
            false
        }
    }

    /// Clears the load reservation
    #[inline]
    pub(crate) const fn clear_reservation(&mut self) {
        self.hart.load_reservation = None;
    }

    /// Creates a new CPU instance with the specified `SoC` and configuration.
    /// The `Soc` must have been built with the same `exit_signal` Arc so
    /// HTIF / `SysCon` writes propagate to the harness.
    pub fn new(soc: Soc, config: &Config, exit_signal: Arc<AtomicU64>) -> Self {
        use crate::core::arch::csr::{
            MISA_DEFAULT_RV64IMAFDC, MISA_EXT_A, MISA_EXT_C, MISA_EXT_D, MISA_EXT_F, MISA_EXT_I,
            MISA_EXT_M, MISA_EXT_S, MISA_EXT_U, MISA_XLEN_64, MSTATUS_DEFAULT_RV64, MSTATUS_FS,
            MSTATUS_FS_INIT, MSTATUS_MXR, MSTATUS_SIE, MSTATUS_SPIE, MSTATUS_SPP, MSTATUS_SUM,
            MSTATUS_UXL, MSTATUS_VS_INIT,
        };
        use crate::isa::abi;

        let configured_misa = config.pipeline.misa_override.as_ref().map_or_else(
            || {
                MISA_XLEN_64
                    | MISA_EXT_A
                    | MISA_EXT_C
                    | MISA_EXT_D
                    | MISA_EXT_F
                    | MISA_EXT_I
                    | MISA_EXT_M
                    | MISA_EXT_S
                    | MISA_EXT_U
            },
            |override_str| {
                let s = override_str.trim_start_matches("0x");
                u64::from_str_radix(s, 16).unwrap_or(MISA_DEFAULT_RV64IMAFDC)
            },
        );

        let direct_mode = config.general.direct_mode;

        // In direct (SE) mode, enable FP state so user programs can use
        // floating-point instructions without an OS to set mstatus.FS/VS.
        // In full-system mode, firmware/OS is responsible for enabling FP/V.
        let mstatus = if direct_mode {
            MSTATUS_DEFAULT_RV64 | MSTATUS_FS_INIT | MSTATUS_VS_INIT
        } else {
            MSTATUS_DEFAULT_RV64
        };

        // Initialize sstatus as a view of mstatus (spec: sstatus is not a
        // separate register, it's a restricted view of mstatus).
        let sstatus_mask = MSTATUS_SIE
            | MSTATUS_SPIE
            | MSTATUS_SPP
            | MSTATUS_FS
            | MSTATUS_SUM
            | MSTATUS_MXR
            | MSTATUS_UXL;
        let sstatus = mstatus & sstatus_mask;
        let vlenb = config.pipeline.vlen / 8;
        let csrs = Csrs {
            mstatus,
            sstatus,
            misa: configured_misa,
            stimecmp: u64::MAX,
            vlenb: vlenb as u64,
            ..Default::default()
        };

        let mut regs = if direct_mode {
            let sp = config.general.initial_sp.unwrap_or(config.system.ram_base + 0x100_0000);
            let mut r = RegisterFile::new();
            r.write(abi::REG_SP, sp);
            r
        } else {
            RegisterFile::new()
        };
        // Initialize vector register file if VLEN > 0
        if config.pipeline.vlen > 0
            && let Ok(vlen) = crate::core::units::vpu::types::Vlen::new(config.pipeline.vlen)
        {
            regs.init_vpr(vlen);
        }

        // Always start in Machine mode. The riscv-tests switch to lower modes
        // via their own trap handlers; bare-metal binaries need M-mode too.
        let privilege = PrivilegeMode::Machine;

        let mmu = Mmu::new(
            config.memory.tlb_size,
            config.memory.l2_tlb_size,
            config.memory.l2_tlb_ways,
            config.memory.l2_tlb_latency,
            config.memory.software_ad_bits,
            config.memory.paging_mode_max,
        );

        let mut hart = Hart::new(HartInit {
            hart_id: HartId::new(0),
            regs,
            pc: config.general.start_pc,
            csrs,
            privilege,
            mmu,
            pmp: Pmp::new(),
        });
        hart.committed_next_pc = config.general.start_pc;

        Self {
            hart,
            core: Core::new(CoreId::new(0), config),
            soc,
            per_hart_debug: vec![HartDebug::default()],
            stats: SimStats::default(),
            #[cfg(feature = "commit-log")]
            commit_log: None,
            exit_signal,
            exit_code: None,
            direct_mode,
            redirect_pending: false,
        }
    }

    /// Atomically takes the exit code if a bus device has signalled termination.
    pub fn take_exit(&self) -> Option<u64> {
        let val = self.exit_signal.swap(u64::MAX, Ordering::Relaxed);
        if val == u64::MAX { None } else { Some(val) }
    }

    /// Returns the exit code if a device has requested shutdown without
    /// clearing the signal slot.
    pub fn check_exit(&self) -> Option<u64> {
        let val = self.exit_signal.load(Ordering::Relaxed);
        if val == u64::MAX { None } else { Some(val) }
    }

    /// Manually signals exit (used by tests and the binding `step` loop).
    pub fn signal_exit(&self, code: u64) {
        self.exit_signal.store(code, Ordering::Relaxed);
    }

    /// Convenience constructor: builds the exit-signal `Arc`, the `Soc`, and
    /// the `Cpu` together. Use this when you don't need to register additional
    /// bus devices between `Soc::new` and `Cpu::new`.
    pub fn build(config: &Config, disk_path: &str) -> Self {
        let exit_signal = Arc::new(AtomicU64::new(u64::MAX));
        let soc = Soc::new(config, disk_path, &exit_signal);
        Self::new(soc, config, exit_signal)
    }

    /// Opens a commit log file for writing retired instruction traces.
    ///
    /// Each retired instruction is logged as `core   0: 0x<pc> (0x<inst>)\n`.
    /// Only available when the `commit-log` Cargo feature is enabled.
    ///
    /// # Errors
    ///
    /// Returns [`SimError::FileRead`] if the file cannot be created.
    #[cfg(feature = "commit-log")]
    pub fn open_commit_log(&mut self, path: &str) -> Result<(), crate::common::SimError> {
        use std::fs::File;
        use std::io::BufWriter;
        let file = File::create(path).map_err(|source| crate::common::SimError::FileRead {
            path: path.to_owned(),
            source,
        })?;
        self.commit_log = Some(BufWriter::with_capacity(1 << 20, file));
        Ok(())
    }

    /// Dumps the current CPU state (PC and registers) to stdout.
    pub fn dump_state(&self) {
        println!("PC = {:#018x}", self.hart.pc);
        self.hart.regs.dump();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_cpu_reservation() {
        let config = Config::default();
        let mut cpu = Cpu::build(&config, "");

        cpu.set_reservation(PhysAddr::new(0x1000));
        assert!(cpu.check_reservation(PhysAddr::new(0x1000)));
        assert!(cpu.check_reservation(PhysAddr::new(0x1008)));
        assert!(!cpu.check_reservation(PhysAddr::new(0x2000)));

        cpu.clear_reservation();
        assert!(!cpu.check_reservation(PhysAddr::new(0x1000)));
    }

    #[test]
    fn test_cpu_dump_state_no_panic() {
        let config = Config::default();
        let cpu = Cpu::build(&config, "");
        cpu.dump_state();
    }

    #[test]
    fn test_cpu_take_exit() {
        let config = Config::default();
        let cpu = Cpu::build(&config, "");

        assert_eq!(cpu.take_exit(), None);
        cpu.signal_exit(42);
        assert_eq!(cpu.take_exit(), Some(42));
        assert_eq!(cpu.take_exit(), None);
    }
}
