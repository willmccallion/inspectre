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
use crate::core::units::cache::CacheSim;
use crate::core::units::mmu::Mmu;
use crate::core::units::mmu::pmp::Pmp;
use crate::core::{Core, Hart};
use crate::soc::Soc;
use crate::stats::SimStats;

/// CPU architectural state: registers, caches, MMU, bus, and statistics.
///
/// The pipeline is owned by `Simulator`, not by `Cpu`. This struct holds only
/// the architectural state that the pipeline reads and writes.
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cpu {
    /// Per-thread architectural state (registers, CSRs, PC, MMU, PMP, ...).
    pub hart: Hart,
    /// Pipeline-private state shared by harts on this core (caches, MSHRs,
    /// branch predictor, prefetch filter, write-combining buffer).
    pub core: Core,

    /// System-on-Chip: bus, memory controller, devices, exit signal.
    pub soc: Soc,
    /// L3 Unified Cache.
    pub l3_cache: CacheSim,
    /// Base address of RAM — addresses at or above this go through the
    /// cache hierarchy for latency simulation; addresses below are MMIO.
    pub cache_base: u64,

    /// Enable instruction tracing.
    pub trace: bool,
    /// Exit code if simulation finished.
    pub exit_code: Option<u64>,
    /// Performance statistics.
    pub stats: SimStats,
    /// Direct mode (no translation, flat memory).
    pub direct_mode: bool,
    /// CLINT time divider.
    pub clint_divider: u64,
    /// Raw pointer to the start of simulated RAM.
    ///
    /// # Safety
    ///
    /// Must point to a valid, mutable memory region of size `ram_end - ram_start`
    /// bytes that remains live for the lifetime of the `Cpu`. All accesses must
    /// verify `ram_start <= address < ram_end` before dereferencing.
    pub ram_ptr: *mut u8,
    /// Physical address where RAM starts.
    pub ram_start: u64,
    /// Physical address where RAM ends (exclusive).
    pub ram_end: u64,

    /// HTIF tohost address range (start, end). Stores in this range bypass the
    /// RAM fast-path and go through the bus so the HTIF device can intercept them.
    pub htif_range: Option<(u64, u64)>,

    /// Set by the backend when a PC redirect occurs (branch misprediction,
    /// trap, FENCE.I, etc.). The pipeline uses this to flush the frontend,
    /// rather than relying solely on `cpu.hart.pc != pc_before` which can miss
    /// redirects when the target happens to equal the current fetch PC.
    pub redirect_pending: bool,

    /// Optional buffered writer for the commit log (enabled by the `commit-log` feature).
    #[cfg(feature = "commit-log")]
    pub commit_log: Option<std::io::BufWriter<std::fs::File>>,
}

/// Maximum number of (pc, inst) entries kept for invalid-PC debug trace.
pub const PC_TRACE_MAX: usize = 32;

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
    pub fn new(mut soc: Soc, config: &Config) -> Self {
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

        let (ram_ptr, ram_start, ram_end) =
            soc.bus.get_ram_info().unwrap_or((std::ptr::null_mut(), 0, 0));
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
            software_ad_bits: config.memory.software_ad_bits,
            misaligned_access_trap: config.memory.misaligned_access_trap,
        });
        hart.committed_next_pc = config.general.start_pc;

        Self {
            hart,
            core: Core::new(CoreId::new(0), config),
            trace: config.general.trace_instructions,
            soc,
            exit_code: None,
            direct_mode,
            cache_base: config.system.ram_base,
            stats: SimStats::default(),
            l3_cache: CacheSim::new(&config.cache.l3),
            clint_divider: config.system.clint_divider,
            ram_ptr,
            ram_start,
            ram_end,
            htif_range: None,
            redirect_pending: false,
            #[cfg(feature = "commit-log")]
            commit_log: None,
        }
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

    /// Retrieves the exit code if the simulation has finished.
    pub const fn take_exit(&mut self) -> Option<u64> {
        self.exit_code.take()
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
    use crate::soc::builder::Soc;

    #[test]
    fn test_cpu_reservation() {
        let config = Config::default();
        let soc = Soc::new(&config, "");
        let mut cpu = Cpu::new(soc, &config);

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
        let soc = Soc::new(&config, "");
        let cpu = Cpu::new(soc, &config);
        cpu.dump_state();
    }

    #[test]
    fn test_cpu_take_exit() {
        let config = Config::default();
        let soc = Soc::new(&config, "");
        let mut cpu = Cpu::new(soc, &config);

        assert_eq!(cpu.take_exit(), None);
        cpu.exit_code = Some(42);
        assert_eq!(cpu.take_exit(), Some(42));
        assert_eq!(cpu.take_exit(), None);
    }
}
