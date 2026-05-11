//! Outstanding-request tracking for the event-driven pipeline.
//!
//! Each in-flight memory operation issued by the pipeline (instruction fetch,
//! demand load, store, page-table walk PTE read) is recorded here keyed by
//! its [`ReqId`].  When the matching [`Packet::MemResp`](crate::sim::packet::Packet::MemResp)
//! lands in the pipeline's mailbox, the corresponding entry is woken up and
//! the stage that was waiting on it advances.

use crate::common::{AccessType, PhysAddr, VirtAddr};
use crate::core::pipeline::latches::Mem1Mem2Entry;
use crate::core::pipeline::rob::RobTag;
use crate::core::pipeline::signals::MemWidth;
use crate::core::units::bru::Ghr;
use crate::sim::components::ReqId;

/// An instruction-fetch request awaiting its `MemResp`.
///
/// The frontend issued a [`MemReq`](crate::sim::packet::Packet::MemReq) with
/// `op = MemOp::Fetch` for the line containing `pc`; the response arrives via
/// the pipeline's mailbox and reactivates the entry so the fetch can drain
/// into the Fetch1→Fetch2 latch.
#[derive(Clone, Debug)]
pub struct OutstandingFetch {
    /// Program counter being fetched.
    pub pc: u64,
    /// Pre-translation address (kept so faults can report the original VA).
    pub vaddr: VirtAddr,
    /// Physical address used to issue the `MemReq`.
    pub paddr: PhysAddr,
    /// Branch-predictor history snapshot captured at fetch time.
    pub ghr_snapshot: Ghr,
    /// Return-address-stack snapshot captured at fetch time.
    pub ras_snapshot: u64,
}

/// A demand load awaiting its `MemResp`.
///
/// Memory1 issued a load request; the parked entry holds the [`Mem1Mem2Entry`]
/// that would otherwise flow into Memory2.  When the response arrives, the
/// entry is restored to the mem1→mem2 latch with `complete_cycle` set to the
/// arrival cycle.
#[derive(Clone, Debug)]
pub struct OutstandingLoad {
    /// ROB tag of the load.
    pub rob_tag: RobTag,
    /// Translated physical address.
    pub paddr: PhysAddr,
    /// Pipeline entry parked until the response arrives.
    pub parked: Mem1Mem2Entry,
}

/// A store awaiting its `MemResp` (write-allocate completion or MMIO ack).
///
/// Stores normally retire through the store buffer; the outstanding entry
/// exists so the LSU can keep an accurate MSHR-like view of inflight write
/// allocations and so MMIO writes can be correlated with their device acks.
#[derive(Clone, Debug)]
pub struct OutstandingStore {
    /// ROB tag of the store.
    pub rob_tag: RobTag,
    /// Translated physical address.
    pub paddr: PhysAddr,
    /// Store data (raw u64; sub-word stores zero-extend).
    pub data: u64,
    /// Width of the store.
    pub width: MemWidth,
}

/// State of an in-progress page-table walk embedded in the LSU's
/// outstanding-request tracking.
///
/// Each PTE read fires a [`MemReq`](crate::sim::packet::Packet::MemReq) for
/// 8 bytes at the computed PTE address.  When the response arrives the
/// walker advances: a pointer PTE moves down a level (decrementing `level`),
/// a leaf PTE completes the walk with a translated `PhysAddr`, and a fault
/// surfaces back through the parked operation's continuation.
#[derive(Clone, Debug)]
pub enum WalkStage {
    /// Awaiting the PTE response for the given level.
    Pending {
        /// Current level (Sv39: 2→0, Sv48: 3→0, Sv57: 4→0).
        level: u32,
        /// PPN of the page table at this level.
        ppn_raw: u64,
    },
    /// Walk completed successfully; the translated physical address is ready.
    Complete {
        /// Translated physical address.
        paddr: PhysAddr,
    },
    /// Walk failed; the trap will be raised on the originating operation.
    Fault {
        /// Faulting virtual address.
        vaddr: VirtAddr,
        /// Access type to choose the correct fault flavour.
        access: AccessType,
    },
}

/// What an in-progress walk is feeding once it completes.
///
/// The walk doesn't know about the originating operation; it only knows
/// how to advance the page table tree.  This enum records what to resume
/// once the walk lands.
#[derive(Clone, Debug)]
pub enum WalkContinuation {
    /// Instruction fetch waiting for translation.
    Fetch(OutstandingFetch),
    /// Demand load waiting for translation.
    Load(OutstandingLoad),
    /// Store waiting for translation.
    Store(OutstandingStore),
}

/// A page-table walk in flight.
#[derive(Clone, Debug)]
pub struct OutstandingWalk {
    /// Virtual address being translated.
    pub vaddr: VirtAddr,
    /// Type of access being attempted.
    pub access: AccessType,
    /// Current stage of the walk.
    pub stage: WalkStage,
    /// What to do once the walk completes.
    pub continuation: WalkContinuation,
}
