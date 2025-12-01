pub mod controller;
use crate::system::devices::Device;

pub struct Memory {
    bytes: Vec<u8>,
    base_addr: u64,
}

impl Memory {
    pub fn new(size: usize, base_addr: u64) -> Self {
        Self {
            bytes: vec![0; size],
            base_addr,
        }
    }

    /// Efficiently copies a slice of data into memory.
    pub fn load(&mut self, data: &[u8], offset: usize) {
        if offset + data.len() <= self.bytes.len() {
            self.bytes[offset..offset + data.len()].copy_from_slice(data);
        } else {
            eprintln!("Memory::load: Data too large for RAM segment!");
        }
    }

    #[inline]
    fn check_bounds(&self, offset: usize, size: usize) -> usize {
        if offset + size > self.bytes.len() {
            panic!(
                "Memory OOB: Offset {:#x} exceeds RAM size {}",
                offset,
                self.bytes.len()
            );
        }
        offset
    }
}

impl Device for Memory {
    fn name(&self) -> &str {
        "DRAM"
    }

    fn address_range(&self) -> (u64, u64) {
        (self.base_addr, self.bytes.len() as u64)
    }

    fn read_u8(&mut self, offset: u64) -> u8 {
        let i = self.check_bounds(offset as usize, 1);
        self.bytes[i]
    }

    fn read_u16(&mut self, offset: u64) -> u16 {
        let i = self.check_bounds(offset as usize, 2);
        u16::from_le_bytes(self.bytes[i..i + 2].try_into().unwrap())
    }

    fn read_u32(&mut self, offset: u64) -> u32 {
        let i = self.check_bounds(offset as usize, 4);
        u32::from_le_bytes(self.bytes[i..i + 4].try_into().unwrap())
    }

    fn read_u64(&mut self, offset: u64) -> u64 {
        let i = self.check_bounds(offset as usize, 8);
        u64::from_le_bytes(self.bytes[i..i + 8].try_into().unwrap())
    }

    fn write_u8(&mut self, offset: u64, val: u8) {
        let i = self.check_bounds(offset as usize, 1);
        self.bytes[i] = val;
    }

    fn write_u16(&mut self, offset: u64, val: u16) {
        let i = self.check_bounds(offset as usize, 2);
        self.bytes[i..i + 2].copy_from_slice(&val.to_le_bytes());
    }

    fn write_u32(&mut self, offset: u64, val: u32) {
        let i = self.check_bounds(offset as usize, 4);
        self.bytes[i..i + 4].copy_from_slice(&val.to_le_bytes());
    }

    fn write_u64(&mut self, offset: u64, val: u64) {
        let i = self.check_bounds(offset as usize, 8);
        self.bytes[i..i + 8].copy_from_slice(&val.to_le_bytes());
    }

    // Override default to use efficient memcpy
    fn write_bytes(&mut self, offset: u64, data: &[u8]) {
        self.load(data, offset as usize);
    }
}
