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

    pub fn load(&mut self, data: &[u8], offset: usize) {
        if offset + data.len() <= self.bytes.len() {
            self.bytes[offset..offset + data.len()].copy_from_slice(data);
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.bytes.as_mut_ptr()
    }

    fn check_bounds(&self, offset: usize, size: usize) -> usize {
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
        self.bytes[offset as usize]
    }
    fn read_u16(&mut self, offset: u64) -> u16 {
        let i = offset as usize;
        u16::from_le_bytes(self.bytes[i..i + 2].try_into().unwrap())
    }
    fn read_u32(&mut self, offset: u64) -> u32 {
        let i = offset as usize;
        u32::from_le_bytes(self.bytes[i..i + 4].try_into().unwrap())
    }
    fn read_u64(&mut self, offset: u64) -> u64 {
        let i = offset as usize;
        u64::from_le_bytes(self.bytes[i..i + 8].try_into().unwrap())
    }
    fn write_u8(&mut self, offset: u64, val: u8) {
        self.bytes[offset as usize] = val;
    }
    fn write_u16(&mut self, offset: u64, val: u16) {
        let i = offset as usize;
        self.bytes[i..i + 2].copy_from_slice(&val.to_le_bytes());
    }
    fn write_u32(&mut self, offset: u64, val: u32) {
        let i = offset as usize;
        self.bytes[i..i + 4].copy_from_slice(&val.to_le_bytes());
    }
    fn write_u64(&mut self, offset: u64, val: u64) {
        let i = offset as usize;
        self.bytes[i..i + 8].copy_from_slice(&val.to_le_bytes());
    }
    fn write_bytes(&mut self, offset: u64, data: &[u8]) {
        self.load(data, offset as usize);
    }
}
