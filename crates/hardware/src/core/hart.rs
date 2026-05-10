//! A RISC-V hardware thread.
//!
//! `Hart` owns the per-thread architectural state: registers, CSRs, program
//! counter, privilege mode, MMU, and load reservation. On a non-SMT core
//! there is exactly one `Hart`; with SMT, sibling threads share the parent
//! [`Core`](super::Core)'s pipeline and L1 caches but each retains its own
//! `Hart`.
//!
//! Currently a placeholder; fields are added as state migrates out of
//! [`Cpu`](super::Cpu).

use crate::common::HartId;

/// Per-thread RISC-V architectural state.
///
/// Constructed with a [`HartId`] that the `mhartid` CSR will report.
#[derive(Debug)]
pub struct Hart {
    /// Globally unique hardware-thread identifier; reported by `mhartid`.
    pub hart_id: HartId,
}

impl Hart {
    /// Creates a new `Hart` bound to the given hardware-thread identifier.
    #[inline]
    pub const fn new(hart_id: HartId) -> Self {
        Self { hart_id }
    }
}
