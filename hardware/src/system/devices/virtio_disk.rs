use crate::system::devices::Device;

pub struct VirtualDisk {
    data: Vec<u8>,
    base_addr: u64,
}

impl VirtualDisk {
    pub fn new(base_addr: u64) -> Self {
        Self {
            data: Vec::new(),
            base_addr,
        }
    }

    pub fn load(&mut self, bytes: Vec<u8>) {
        self.data = bytes;
    }

    fn size_le(&self) -> [u8; 8] {
        (self.data.len() as u64).to_le_bytes()
    }
}

impl Device for VirtualDisk {
    fn name(&self) -> &str {
        "VirtIO Disk"
    }

    fn address_range(&self) -> (u64, u64) {
        // We expose the disk data + 8 bytes for the size register
        (self.base_addr, (self.data.len() as u64) + 8)
    }

    fn read_u8(&mut self, offset: u64) -> u8 {
        let len = self.data.len() as u64;
        if offset < len {
            self.data[offset as usize]
        } else if offset >= len && offset < len + 8 {
            let idx = (offset - len) as usize;
            self.size_le()[idx]
        } else {
            0
        }
    }

    fn read_u16(&mut self, offset: u64) -> u16 {
        let len = self.data.len() as u64;
        if offset < len - 1 {
            let o = offset as usize;
            u16::from_le_bytes(self.data[o..o + 2].try_into().unwrap())
        } else if offset >= len && offset < len + 7 {
            let idx = (offset - len) as usize;
            let s = self.size_le();
            u16::from_le_bytes([s[idx], s[idx + 1]])
        } else {
            0
        }
    }

    fn read_u32(&mut self, offset: u64) -> u32 {
        let len = self.data.len() as u64;
        if offset < len - 3 {
            let o = offset as usize;
            u32::from_le_bytes(self.data[o..o + 4].try_into().unwrap())
        } else if offset >= len && offset < len + 5 {
            let idx = (offset - len) as usize;
            let s = self.size_le();
            u32::from_le_bytes(s[idx..idx + 4].try_into().unwrap())
        } else {
            0
        }
    }

    fn read_u64(&mut self, offset: u64) -> u64 {
        let len = self.data.len() as u64;
        if offset < len - 7 {
            let o = offset as usize;
            u64::from_le_bytes(self.data[o..o + 8].try_into().unwrap())
        } else if offset == len {
            u64::from_le_bytes(self.size_le())
        } else {
            0
        }
    }

    fn write_u8(&mut self, offset: u64, val: u8) {
        if offset < self.data.len() as u64 {
            self.data[offset as usize] = val;
        }
    }

    fn write_u16(&mut self, offset: u64, val: u16) {
        if offset < (self.data.len() as u64) - 1 {
            let o = offset as usize;
            let bytes = val.to_le_bytes();
            self.data[o] = bytes[0];
            self.data[o + 1] = bytes[1];
        }
    }

    fn write_u32(&mut self, offset: u64, val: u32) {
        if offset < (self.data.len() as u64) - 3 {
            let o = offset as usize;
            let bytes = val.to_le_bytes();
            self.data[o..o + 4].copy_from_slice(&bytes);
        }
    }

    fn write_u64(&mut self, offset: u64, val: u64) {
        if offset < (self.data.len() as u64) - 7 {
            let o = offset as usize;
            let bytes = val.to_le_bytes();
            self.data[o..o + 8].copy_from_slice(&bytes);
        }
    }
}
