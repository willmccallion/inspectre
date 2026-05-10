//! Load reservation (LR/SC) state and operations.
//!
//! RISC-V LR/SC pairs reserve a cache-line-aligned region of memory; a
//! subsequent SC succeeds only if no other write has invalidated the
//! reservation. The simulator tracks one reservation per hart at cache-line
//! granularity (the SC-success granule is implementation-defined; we pick
//! 64 bytes to match a typical L1D line).

use super::Hart;
use crate::common::PhysAddr;

impl Hart {
    /// Cache line size for reservation granularity (64 bytes).
    const RESERVATION_GRANULE: u64 = 64;

    /// Aligns an address to the reservation granule (cache line boundary).
    #[inline]
    const fn align_reservation_address(addr: PhysAddr) -> PhysAddr {
        PhysAddr(addr.0 & !(Self::RESERVATION_GRANULE - 1))
    }

    /// Sets a load reservation at the given address (cache-line aligned).
    #[inline]
    pub(crate) const fn set_reservation(&mut self, addr: PhysAddr) {
        self.load_reservation = Some(Self::align_reservation_address(addr));
    }

    /// Returns `true` when a reservation covers `addr` (same cache line).
    #[inline]
    pub(crate) const fn check_reservation(&self, addr: PhysAddr) -> bool {
        if let Some(reserved_addr) = self.load_reservation {
            reserved_addr.0 == Self::align_reservation_address(addr).0
        } else {
            false
        }
    }

    /// Clears the load reservation.
    #[inline]
    pub(crate) const fn clear_reservation(&mut self) {
        self.load_reservation = None;
    }
}
