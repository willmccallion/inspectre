use crate::system::devices::Device;

const MSIP_OFFSET: u64 = 0x0000;
const MTIMECMP_OFFSET: u64 = 0x4000;
const MTIME_OFFSET: u64 = 0xBFF8;

pub struct Clint {
    base_addr: u64,
    mtime: u64,
    mtimecmp: u64,
    msip: u32,
    divider: u64,
    counter: u64,
}

impl Clint {
    pub fn new(base_addr: u64, divider: u64) -> Self {
        Self {
            base_addr,
            mtime: 0,
            mtimecmp: u64::MAX,
            msip: 0,
            divider: if divider == 0 { 1 } else { divider },
            counter: 0,
        }
    }
}

impl Device for Clint {
    fn name(&self) -> &str {
        "CLINT"
    }

    fn address_range(&self) -> (u64, u64) {
        (self.base_addr, 0x10000)
    }

    fn read_u8(&mut self, offset: u64) -> u8 {
        let val = self.read_u64(offset & !7);
        let shift = (offset & 7) * 8;
        ((val >> shift) & 0xFF) as u8
    }

    fn read_u16(&mut self, _offset: u64) -> u16 {
        0
    }

    fn read_u32(&mut self, offset: u64) -> u32 {
        match offset {
            MSIP_OFFSET => self.msip,
            MTIMECMP_OFFSET => self.mtimecmp as u32,
            val if val == MTIMECMP_OFFSET + 4 => (self.mtimecmp >> 32) as u32,
            MTIME_OFFSET => self.mtime as u32,
            val if val == MTIME_OFFSET + 4 => (self.mtime >> 32) as u32,
            _ => 0,
        }
    }

    fn read_u64(&mut self, offset: u64) -> u64 {
        match offset {
            MSIP_OFFSET => self.msip as u64,
            MTIMECMP_OFFSET => self.mtimecmp,
            MTIME_OFFSET => self.mtime,
            _ => 0,
        }
    }

    fn write_u8(&mut self, _offset: u64, _val: u8) {}
    fn write_u16(&mut self, _offset: u64, _val: u16) {}

    fn write_u32(&mut self, offset: u64, val: u32) {
        match offset {
            MSIP_OFFSET => self.msip = val & 1,
            MTIMECMP_OFFSET => {
                self.mtimecmp = (self.mtimecmp & 0xFFFF_FFFF_0000_0000) | (val as u64)
            }
            val if val == MTIMECMP_OFFSET + 4 => {
                self.mtimecmp = (self.mtimecmp & 0x0000_0000_FFFF_FFFF) | (val << 32)
            }
            MTIME_OFFSET => self.mtime = (self.mtime & 0xFFFF_FFFF_0000_0000) | (val as u64),
            val if val == MTIME_OFFSET + 4 => {
                self.mtime = (self.mtime & 0x0000_0000_FFFF_FFFF) | (val << 32)
            }
            _ => {}
        }
    }

    fn write_u64(&mut self, offset: u64, val: u64) {
        match offset {
            MSIP_OFFSET => self.msip = (val as u32) & 1,
            MTIMECMP_OFFSET => self.mtimecmp = val,
            MTIME_OFFSET => self.mtime = val,
            _ => {}
        }
    }

    fn tick(&mut self) -> bool {
        self.counter += 1;
        if self.counter >= self.divider {
            self.mtime = self.mtime.wrapping_add(1);
            self.counter = 0;
        }
        // Assert interrupt if mtime >= mtimecmp
        self.mtime >= self.mtimecmp || (self.msip & 1) != 0
    }
}
