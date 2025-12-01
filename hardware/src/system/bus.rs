use super::devices::Device;

pub struct Bus {
    devices: Vec<Box<dyn Device>>,
    pub width_bytes: u64,
    pub latency_cycles: u64,
}

impl Bus {
    pub fn new(width_bytes: u64, latency_cycles: u64) -> Self {
        Self {
            devices: Vec::new(),
            width_bytes,
            latency_cycles,
        }
    }

    pub fn add_device(&mut self, dev: Box<dyn Device>) {
        let (base, size) = dev.address_range();
        println!(
            "[Bus] Registered device: {:<12} @ {:#010x} - {:#010x} ({} bytes)",
            dev.name(),
            base,
            base + size,
            size
        );
        self.devices.push(dev);
        // Sort by base address for consistent lookup
        self.devices.sort_by_key(|d| d.address_range().0);
    }

    /// Calculates the transit time for a packet of `bytes` size over the bus.
    pub fn calculate_transit_time(&self, bytes: usize) -> u64 {
        let transfers = (bytes as u64 + self.width_bytes - 1) / self.width_bytes;
        self.latency_cycles + transfers
    }

    pub fn load_binary_at(&mut self, data: &[u8], addr: u64) {
        if let Some((dev, offset)) = self.find_device(addr) {
            let (_, size) = dev.address_range();
            if offset + (data.len() as u64) <= size {
                dev.write_bytes(offset, data);
                return;
            }
        }

        for (i, byte) in data.iter().enumerate() {
            self.write_u8(addr + i as u64, *byte);
        }
    }

    pub fn is_valid_address(&self, paddr: u64) -> bool {
        for dev in &self.devices {
            let (start, size) = dev.address_range();
            if paddr >= start && paddr < start + size {
                return true;
            }
        }
        false
    }

    pub fn tick(&mut self) -> bool {
        let mut interrupt_pending = false;
        for dev in &mut self.devices {
            if dev.tick() {
                interrupt_pending = true;
            }
        }
        interrupt_pending
    }

    fn find_device(&mut self, paddr: u64) -> Option<(&mut Box<dyn Device>, u64)> {
        for dev in &mut self.devices {
            let (start, size) = dev.address_range();
            if paddr >= start && paddr < start + size {
                return Some((dev, paddr - start));
            }
        }
        None
    }

    pub fn read_u8(&mut self, paddr: u64) -> u8 {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.read_u8(offset)
        } else {
            eprintln!("Bus Error: Read Fault @ {:#x}", paddr);
            0
        }
    }

    pub fn read_u16(&mut self, paddr: u64) -> u16 {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.read_u16(offset)
        } else {
            eprintln!("Bus Error: Read Fault @ {:#x}", paddr);
            0
        }
    }

    pub fn read_u32(&mut self, paddr: u64) -> u32 {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.read_u32(offset)
        } else {
            eprintln!("Bus Error: Read Fault @ {:#x}", paddr);
            0
        }
    }

    pub fn read_u64(&mut self, paddr: u64) -> u64 {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.read_u64(offset)
        } else {
            eprintln!("Bus Error: Read Fault @ {:#x}", paddr);
            0
        }
    }

    pub fn write_u8(&mut self, paddr: u64, val: u8) {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.write_u8(offset, val);
        } else {
            eprintln!("Bus Error: Write Fault @ {:#x}", paddr);
        }
    }

    pub fn write_u16(&mut self, paddr: u64, val: u16) {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.write_u16(offset, val);
        } else {
            eprintln!("Bus Error: Write Fault @ {:#x}", paddr);
        }
    }

    pub fn write_u32(&mut self, paddr: u64, val: u32) {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.write_u32(offset, val);
        } else {
            eprintln!("Bus Error: Write Fault @ {:#x}", paddr);
        }
    }

    pub fn write_u64(&mut self, paddr: u64, val: u64) {
        if let Some((dev, offset)) = self.find_device(paddr) {
            dev.write_u64(offset, val);
        } else {
            eprintln!("Bus Error: Write Fault @ {:#x}", paddr);
        }
    }
}
