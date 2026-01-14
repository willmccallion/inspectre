use crate::system::devices::Device;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::mpsc::{Receiver, channel};
use std::thread;

pub struct Uart {
    base_addr: u64,
    rx_queue: VecDeque<u8>,
    rx_receiver: Receiver<u8>,
    ier: u8,
    lcr: u8,
    mcr: u8,
    scr: u8,
    div: u16,
    tick_count: u8, // Throttle counter
}

impl Uart {
    pub fn new(base_addr: u64) -> Self {
        let (tx, rx) = channel();

        thread::spawn(move || {
            let mut buffer = [0u8; 1];
            let stdin = io::stdin();
            let mut handle = stdin.lock();
            while handle.read_exact(&mut buffer).is_ok() {
                let _ = tx.send(buffer[0]);
            }
        });

        Self {
            base_addr,
            rx_queue: VecDeque::new(),
            rx_receiver: rx,
            ier: 0,
            lcr: 0,
            mcr: 0,
            scr: 0,
            div: 0,
            tick_count: 0,
        }
    }

    fn check_stdin(&mut self) {
        while let Ok(byte) = self.rx_receiver.try_recv() {
            self.rx_queue.push_back(byte);
        }
    }
}

impl Device for Uart {
    fn name(&self) -> &str {
        "UART0"
    }
    fn address_range(&self) -> (u64, u64) {
        (self.base_addr, 0x100)
    }

    fn read_u8(&mut self, offset: u64) -> u8 {
        match offset {
            0 => {
                if (self.lcr & 0x80) != 0 {
                    (self.div & 0xFF) as u8
                } else {
                    self.rx_queue.pop_front().unwrap_or(0)
                }
            }
            1 => {
                if (self.lcr & 0x80) != 0 {
                    (self.div >> 8) as u8
                } else {
                    self.ier
                }
            }
            2 => 0xC1,
            3 => self.lcr,
            4 => self.mcr,
            5 => {
                let mut lsr = 0x60;
                if !self.rx_queue.is_empty() {
                    lsr |= 0x01;
                }
                lsr
            }
            6 => 0,
            7 => self.scr,
            _ => 0,
        }
    }

    fn read_u16(&mut self, offset: u64) -> u16 {
        self.read_u8(offset) as u16
    }
    fn read_u32(&mut self, offset: u64) -> u32 {
        self.read_u8(offset) as u32
    }
    fn read_u64(&mut self, offset: u64) -> u64 {
        self.read_u8(offset) as u64
    }

    fn write_u8(&mut self, offset: u64, val: u8) {
        match offset {
            0 => {
                if (self.lcr & 0x80) != 0 {
                    self.div = (self.div & 0xFF00) | (val as u16);
                } else {
                    print!("{}", val as char);
                    if val == b'\n' {
                        io::stdout().flush().ok();
                    }
                }
            }
            1 => {
                if (self.lcr & 0x80) != 0 {
                    self.div = (self.div & 0x00FF) | ((val as u16) << 8);
                } else {
                    self.ier = val;
                }
            }
            3 => self.lcr = val,
            4 => self.mcr = val,
            7 => self.scr = val,
            _ => {}
        }
    }

    fn write_u16(&mut self, offset: u64, val: u16) {
        self.write_u8(offset, val as u8);
    }
    fn write_u32(&mut self, offset: u64, val: u32) {
        self.write_u8(offset, val as u8);
    }
    fn write_u64(&mut self, offset: u64, val: u64) {
        self.write_u8(offset, val as u8);
    }

    fn tick(&mut self) -> bool {
        // Optimization: Only check stdin every 255 cycles to reduce mutex contention
        self.tick_count = self.tick_count.wrapping_add(1);
        if self.tick_count == 0 {
            self.check_stdin();
        }
        (self.ier & 1) != 0 && !self.rx_queue.is_empty()
    }

    fn get_irq_id(&self) -> Option<u32> {
        Some(10)
    }
}
