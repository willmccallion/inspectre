//! Core Local Interruptor (CLINT).
//!
//! The CLINT block holds memory-mapped control and status registers associated
//! with software and timer interrupts. It complies with the RISC-V Privileged
//! Specification.
//!
//! # Memory Map
//!
//! * `0x0000`: MSIP (Machine Software Interrupt Pending)
//! * `0x4000`: MTIMECMP (Machine Time Compare)
//! * `0xBFF8`: MTIME (Machine Time)

use crate::common::LineAddr;
use crate::sim::components::ComponentId;
use crate::sim::handle::{Handle, HandleCtx};
use crate::sim::packet::{AccessSize, HitLevel, MemOp, MemRespData, Packet, WriteData};
use crate::soc::devices::Device;

/// Offset for the Machine Software Interrupt Pending register.
const MSIP_OFFSET: u64 = 0x0000;
/// Offset for the Machine Time Compare register.
const MTIMECMP_OFFSET: u64 = 0x4000;
/// Offset for the Machine Time register.
const MTIME_OFFSET: u64 = 0xBFF8;

/// CLINT device structure.
#[derive(Debug)]
pub struct Clint {
    /// Base physical address of the device.
    base_addr: u64,
    /// Current machine time counter.
    mtime: u64,
    /// Machine time compare register.
    mtimecmp: u64,
    /// Machine software interrupt pending register.
    msip: u32,
    /// Divider to scale CPU cycles to timer ticks.
    divider: u64,
    /// Internal counter for the divider.
    counter: u64,
}

impl Clint {
    /// Creates a new CLINT device.
    ///
    /// # Arguments
    ///
    /// * `base_addr` - The base physical address.
    /// * `divider` - The ratio of CPU cycles to timer ticks (e.g., 10 means timer increments every 10 cycles).
    pub const fn new(base_addr: u64, divider: u64) -> Self {
        Self {
            base_addr,
            mtime: 0,
            mtimecmp: u64::MAX,
            msip: 0,
            divider: if divider == 0 { 1 } else { divider },
            counter: 0,
        }
    }

    /// Returns `true` if the machine software interrupt pending bit is set.
    pub const fn msip_pending(&self) -> bool {
        (self.msip & 1) != 0
    }

    fn read_register(&self, offset: u64, size: AccessSize) -> u64 {
        match size {
            AccessSize::B8 => match offset {
                MSIP_OFFSET => u64::from(self.msip),
                MTIMECMP_OFFSET => self.mtimecmp,
                MTIME_OFFSET => self.mtime,
                _ => 0,
            },
            AccessSize::B4 => match offset {
                MSIP_OFFSET => u64::from(self.msip),
                MTIMECMP_OFFSET => u64::from(self.mtimecmp as u32),
                o if o == MTIMECMP_OFFSET + 4 => u64::from((self.mtimecmp >> 32) as u32),
                MTIME_OFFSET => u64::from(self.mtime as u32),
                o if o == MTIME_OFFSET + 4 => u64::from((self.mtime >> 32) as u32),
                _ => 0,
            },
            AccessSize::B1 => {
                let aligned = offset & !7;
                let shift = (offset & 7) * 8;
                let val = self.read_register(aligned, AccessSize::B8);
                (val >> shift) & 0xFF
            }
            _ => 0,
        }
    }

    fn write_register(&mut self, offset: u64, size: AccessSize, val: u64) {
        match size {
            AccessSize::B8 => match offset {
                MSIP_OFFSET => self.msip = (val as u32) & 1,
                MTIMECMP_OFFSET => self.mtimecmp = val,
                MTIME_OFFSET => self.mtime = val,
                _ => {}
            },
            AccessSize::B4 => {
                let v32 = val as u32;
                match offset {
                    MSIP_OFFSET => self.msip = v32 & 1,
                    MTIMECMP_OFFSET => {
                        self.mtimecmp =
                            (self.mtimecmp & 0xFFFF_FFFF_0000_0000) | u64::from(v32);
                    }
                    o if o == MTIMECMP_OFFSET + 4 => {
                        self.mtimecmp = (self.mtimecmp & 0x0000_0000_FFFF_FFFF)
                            | (u64::from(v32) << 32);
                    }
                    MTIME_OFFSET => {
                        self.mtime = (self.mtime & 0xFFFF_FFFF_0000_0000) | u64::from(v32);
                    }
                    o if o == MTIME_OFFSET + 4 => {
                        self.mtime =
                            (self.mtime & 0x0000_0000_FFFF_FFFF) | (u64::from(v32) << 32);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Handle for Clint {
    fn handle(&mut self, packet: Packet, source: ComponentId, ctx: &mut HandleCtx<'_>) {
        if let Packet::MemReq { req_id, paddr, size, op, .. } = packet {
            let offset = paddr.val().saturating_sub(self.base_addr);
            let value = match op {
                MemOp::Read | MemOp::Fetch | MemOp::Atomic { .. } => {
                    self.read_register(offset, size)
                }
                MemOp::Write { data: WriteData::Small(val) } => {
                    self.write_register(offset, size, val);
                    0
                }
                MemOp::Write { .. } => 0,
            };
            ctx.scheduler.schedule(
                ctx.cycle + 1,
                source,
                ctx.self_id,
                Packet::MemResp {
                    req_id,
                    line_addr: LineAddr::from_phys(paddr, 64),
                    data: MemRespData::Small(value),
                    hit_level: HitLevel::Mmio,
                },
            );
        }
    }
}

impl Device for Clint {
    fn name(&self) -> &'static str {
        "CLINT"
    }

    fn address_range(&self) -> (u64, u64) {
        (self.base_addr, 0x10000)
    }

    /// Advances the device state by one cycle.
    /// Returns `true` if the machine timer interrupt is pending.
    fn tick(&mut self) -> bool {
        self.counter += 1;
        if self.counter >= self.divider {
            self.mtime = self.mtime.wrapping_add(1);
            self.counter = 0;
        }

        self.mtime >= self.mtimecmp
    }

    fn as_clint_mut(&mut self) -> Option<&mut Clint> {
        Some(self)
    }
}
