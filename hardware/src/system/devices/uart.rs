use crate::system::devices::Device;
use std::io::{self, Read, Write};

pub struct Uart {
    base_addr: u64,
}

impl Uart {
    pub fn new(base_addr: u64) -> Self {
        Self { base_addr }
    }
}

impl Device for Uart {
    fn name(&self) -> &str {
        "UART0"
    }

    fn address_range(&self) -> (u64, u64) {
        (self.base_addr, 0x100)
    }

    fn read_u8(&mut self, _offset: u64) -> u8 {
        let mut buf = [0u8; 1];
        match io::stdin().read(&mut buf) {
            Ok(1) => buf[0],
            _ => 0,
        }
    }

    fn read_u16(&mut self, _offset: u64) -> u16 {
        0
    }

    fn read_u32(&mut self, _offset: u64) -> u32 {
        0
    }

    fn read_u64(&mut self, _offset: u64) -> u64 {
        0
    }

    fn write_u8(&mut self, _offset: u64, val: u8) {
        print!("{}", val as char);
        io::stdout().flush().ok();
    }

    fn write_u16(&mut self, _offset: u64, _val: u16) {}

    fn write_u32(&mut self, _offset: u64, _val: u32) {}

    fn write_u64(&mut self, _offset: u64, _val: u64) {}
}
