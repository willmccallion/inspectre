//! Goldfish Real-Time Clock (RTC).
//!
//! A virtual RTC device commonly used in Android emulators (QEMU).
//! It provides the current system time in nanoseconds.
//!
//! # Memory Map
//!
//! * `0x00`: Time (Low 32 bits)
//! * `0x04`: Time (High 32 bits)

use crate::common::IrqId;
use crate::soc::devices::Device;
use std::time::{SystemTime, UNIX_EPOCH};

/// Goldfish RTC device structure.
#[derive(Debug)]
pub struct GoldfishRtc {
    /// Base physical address of the device.
    base_addr: u64,
}

impl GoldfishRtc {
    /// Creates a new Goldfish RTC device.
    pub const fn new(base_addr: u64) -> Self {
        Self { base_addr }
    }

    /// Retrieves the current system time in nanoseconds.
    #[allow(clippy::unused_self)]
    fn get_time_ns(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64
    }
}

impl Device for GoldfishRtc {
    fn name(&self) -> &'static str {
        "GoldfishRTC"
    }

    fn address_range(&self) -> (u64, u64) {
        (self.base_addr, 0x1000)
    }

    fn read_u8(&mut self, _offset: u64) -> u8 {
        0
    }
    fn read_u16(&mut self, _offset: u64) -> u16 {
        0
    }

    fn read_u32(&mut self, offset: u64) -> u32 {
        let time = self.get_time_ns();
        match offset {
            0x00 => time as u32,
            0x04 => (time >> 32) as u32,
            _ => 0,
        }
    }

    fn read_u64(&mut self, offset: u64) -> u64 {
        let time = self.get_time_ns();
        match offset {
            0x00 => time,
            _ => 0,
        }
    }

    fn write_u8(&mut self, _offset: u64, _val: u8) {}
    fn write_u16(&mut self, _offset: u64, _val: u16) {}
    fn write_u32(&mut self, _offset: u64, _val: u32) {}
    fn write_u64(&mut self, _offset: u64, _val: u64) {}

    fn get_irq_id(&self) -> Option<IrqId> {
        Some(IrqId::new(11))
    }
}
