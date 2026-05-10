//! A RISC-V hardware thread.
//!
//! `Hart` owns the per-thread architectural state: registers, CSRs, program
//! counter, privilege mode, MMU, PMP, and load reservation. On a non-SMT core
//! there is exactly one `Hart`; with SMT, sibling threads share the parent
//! [`Core`](super::Core)'s pipeline and L1 caches but each retains its own
//! `Hart`.
//!
//! Constructed with a [`HartId`] that the `mhartid` CSR reports.

use crate::common::{HartId, PhysAddr, RegisterFile};
use crate::core::arch::csr::Csrs;
use crate::core::arch::mode::PrivilegeMode;
use crate::core::units::mmu::Mmu;
use crate::core::units::mmu::pmp::Pmp;

/// Per-thread RISC-V architectural state.
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct Hart {
    /// Globally unique hardware-thread identifier; reported by `mhartid`.
    pub hart_id: HartId,
    /// General Purpose and Floating Point Registers.
    pub regs: RegisterFile,
    /// Program Counter.
    pub pc: u64,
    /// Control and Status Registers.
    pub csrs: Csrs,
    /// Current Privilege Mode (M, S, U).
    pub privilege: PrivilegeMode,
    /// Load Reservation address (for LR/SC).
    pub load_reservation: Option<PhysAddr>,
    /// Memory Management Unit.
    pub mmu: Mmu,
    /// Physical Memory Protection unit.
    pub pmp: Pmp,
    /// Last PC observed by the hang detector.
    pub last_pc: u64,
    /// Number of consecutive cycles `pc` has been equal to `last_pc`.
    pub same_pc_count: u64,
    /// True when the hart has executed `WFI` and is waiting for an interrupt.
    pub wfi_waiting: bool,
    /// PC at which `WFI` was entered.
    pub wfi_pc: u64,
    /// PC the next committed instruction should start at.
    ///
    /// Updated after every commit to `entry.pc + entry.inst_size`. Used as
    /// the EPC for interrupts when the ROB is empty, because `pc` is the
    /// fetch PC and may be far ahead of the commit point.
    pub committed_next_pc: u64,
    /// Ring buffer of `(pc, inst)` for the last N retired instructions; used
    /// by the invalid-PC debug trace.
    pub pc_trace: Vec<(u64, u32)>,
    /// Last invalid PC the debug trace printed (deduplicates dumps).
    pub last_invalid_pc_debug: Option<u64>,
    /// Cycle at which a kernel panic was first detected (`None` if not yet).
    /// The simulator runs for some additional cycles after detection so the
    /// full panic message can print before exit.
    pub panic_detected_at_cycle: Option<u64>,
    /// Software-written SEIP bit. The `mip` SEIP bit is the OR of this and
    /// the PLIC hardware signal, so the software component is tracked here.
    pub sw_seip: bool,
}

/// Initial values for constructing a [`Hart`].
///
/// Keeps the [`Hart::new`] signature short while making the inputs explicit at
/// the call site.
#[derive(Debug)]
pub struct HartInit {
    /// Globally unique hardware-thread identifier.
    pub hart_id: HartId,
    /// Initial register file (with stack pointer set if running bare-metal).
    pub regs: RegisterFile,
    /// Initial program counter.
    pub pc: u64,
    /// Initial CSR state.
    pub csrs: Csrs,
    /// Initial privilege mode.
    pub privilege: PrivilegeMode,
    /// MMU instance (TLBs + page-table walker).
    pub mmu: Mmu,
    /// Physical memory protection unit.
    pub pmp: Pmp,
}

impl Hart {
    /// Creates a new `Hart` from its initial configuration.
    pub fn new(init: HartInit) -> Self {
        Self {
            hart_id: init.hart_id,
            regs: init.regs,
            pc: init.pc,
            csrs: init.csrs,
            privilege: init.privilege,
            load_reservation: None,
            mmu: init.mmu,
            pmp: init.pmp,
            last_pc: 0,
            same_pc_count: 0,
            wfi_waiting: false,
            wfi_pc: 0,
            committed_next_pc: 0,
            pc_trace: Vec::with_capacity(super::cpu::PC_TRACE_MAX),
            last_invalid_pc_debug: None,
            panic_detected_at_cycle: None,
            sw_seip: false,
        }
    }
}
