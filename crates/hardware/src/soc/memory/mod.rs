//! Physical System Memory (DRAM): backing buffer, mapping device, and latency controller.

/// DRAM buffer implementation (e.g., mmap or `Vec`) for raw byte storage.
pub mod buffer;

/// Memory controller implementations for access latency modeling.
pub mod controller;

use self::buffer::DramBuffer;
use crate::soc::devices::Device;
use std::sync::Arc;

/// System Memory structure.
#[derive(Debug)]
pub struct Memory {
    /// Shared reference to the underlying memory buffer.
    buffer: Arc<DramBuffer>,
    /// The base physical address where this memory is mapped.
    base_addr: u64,
}

impl Memory {
    /// Creates a new Memory instance backed by a shared DRAM buffer at `base_addr`.
    pub const fn new(buffer: Arc<DramBuffer>, base_addr: u64) -> Self {
        Self { buffer, base_addr }
    }

    /// Loads a byte slice into memory at a specific offset.
    pub fn load(&mut self, data: &[u8], offset: usize) {
        if offset + data.len() <= self.buffer.len() {
            self.buffer.write_slice(offset, data);
        }
    }

    /// Returns a raw mutable pointer to the underlying memory buffer.
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.buffer.as_mut_ptr()
    }
}

impl Device for Memory {
    fn name(&self) -> &'static str {
        "DRAM"
    }

    fn address_range(&self) -> (u64, u64) {
        (self.base_addr, self.buffer.len() as u64)
    }

    fn read_u8(&mut self, offset: u64) -> u8 {
        self.buffer.read_u8(offset as usize)
    }

    fn read_u16(&mut self, offset: u64) -> u16 {
        let i = offset as usize;
        let s = self.buffer.read_slice(i, 2);
        u16::from_le_bytes([s[0], s[1]])
    }

    fn read_u32(&mut self, offset: u64) -> u32 {
        let i = offset as usize;
        let s = self.buffer.read_slice(i, 4);
        u32::from_le_bytes([s[0], s[1], s[2], s[3]])
    }

    fn read_u64(&mut self, offset: u64) -> u64 {
        let i = offset as usize;
        let s = self.buffer.read_slice(i, 8);
        u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])
    }

    fn write_u8(&mut self, offset: u64, val: u8) {
        self.buffer.write_u8(offset as usize, val);
    }

    fn write_u16(&mut self, offset: u64, val: u16) {
        self.buffer.write_slice(offset as usize, &val.to_le_bytes());
    }

    fn write_u32(&mut self, offset: u64, val: u32) {
        self.buffer.write_slice(offset as usize, &val.to_le_bytes());
    }

    fn write_u64(&mut self, offset: u64, val: u64) {
        self.buffer.write_slice(offset as usize, &val.to_le_bytes());
    }

    fn write_bytes(&mut self, offset: u64, data: &[u8]) {
        self.load(data, offset as usize);
    }

    fn as_memory_mut(&mut self) -> Option<&mut Memory> {
        Some(self)
    }
}
