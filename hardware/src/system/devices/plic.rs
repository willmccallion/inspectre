use crate::system::devices::Device;

const PLIC_PRIORITY_BASE: u64 = 0x000000;
const PLIC_PENDING_BASE: u64 = 0x001000;
const PLIC_ENABLE_BASE: u64 = 0x002000;
const PLIC_CONTEXT_BASE: u64 = 0x200000;

pub struct Plic {
    base_addr: u64,
    priorities: Vec<u32>,
    pending: Vec<u32>, // Bitmap (32 x 32 = 1024 IRQs)
    enables: Vec<u32>, // Bitmap
    thresholds: Vec<u32>,
    claims: Vec<u32>,
}

impl Plic {
    pub fn new(base_addr: u64) -> Self {
        Self {
            base_addr,
            priorities: vec![0; 1024],
            pending: vec![0; 32],
            enables: vec![0; 32],
            thresholds: vec![0; 2],
            claims: vec![0; 2],
        }
    }

    // Fast update from Bus bitmask (supports IRQs 0-63)
    pub fn update_irqs(&mut self, mask: u64) {
        // Word 0 (IRQs 0-31)
        self.pending[0] = (mask & 0xFFFFFFFF) as u32;
        // Word 1 (IRQs 32-63)
        self.pending[1] = (mask >> 32) as u32;
    }

    pub fn set_irq(&mut self, irq: usize, active: bool) {
        let idx = irq / 32;
        let bit = 1 << (irq % 32);
        if idx < self.pending.len() {
            if active {
                self.pending[idx] |= bit;
            } else {
                self.pending[idx] &= !bit;
            }
        }
    }
}

impl Device for Plic {
    fn name(&self) -> &str {
        "PLIC"
    }
    fn address_range(&self) -> (u64, u64) {
        (self.base_addr, 0x4000000)
    }

    fn read_u32(&mut self, offset: u64) -> u32 {
        if offset >= PLIC_PRIORITY_BASE && offset < PLIC_PENDING_BASE {
            let idx = (offset - PLIC_PRIORITY_BASE) as usize / 4;
            if idx < self.priorities.len() {
                return self.priorities[idx];
            }
        } else if offset >= PLIC_PENDING_BASE && offset < PLIC_ENABLE_BASE {
            let idx = (offset - PLIC_PENDING_BASE) as usize / 4;
            if idx < self.pending.len() {
                return self.pending[idx];
            }
        } else if offset >= PLIC_ENABLE_BASE && offset < PLIC_CONTEXT_BASE {
            let ctx = (offset - PLIC_ENABLE_BASE) as usize / 0x80;
            if ctx < 2 {
                return self.enables[ctx];
            }
        } else if offset >= PLIC_CONTEXT_BASE {
            let ctx = (offset - PLIC_CONTEXT_BASE) as usize / 0x1000;
            let reg = offset & 0xFFF;
            if ctx < 2 {
                if reg == 0 {
                    return self.thresholds[ctx];
                }
                if reg == 4 {
                    let claim = self.claims[ctx];
                    if claim > 0 {
                        // Clear pending bit on claim
                        let idx = claim as usize / 32;
                        let bit = 1 << (claim % 32);
                        self.pending[idx] &= !bit;
                    }
                    return claim;
                }
            }
        }
        0
    }

    fn write_u32(&mut self, offset: u64, val: u32) {
        if offset >= PLIC_PRIORITY_BASE && offset < PLIC_PENDING_BASE {
            let idx = (offset - PLIC_PRIORITY_BASE) as usize / 4;
            if idx < self.priorities.len() {
                self.priorities[idx] = val;
            }
        } else if offset >= PLIC_ENABLE_BASE && offset < PLIC_CONTEXT_BASE {
            let ctx = (offset - PLIC_ENABLE_BASE) as usize / 0x80;
            if ctx < 2 {
                self.enables[ctx] = val;
            }
        } else if offset >= PLIC_CONTEXT_BASE {
            let ctx = (offset - PLIC_CONTEXT_BASE) as usize / 0x1000;
            let reg = offset & 0xFFF;
            if ctx < 2 {
                if reg == 0 {
                    self.thresholds[ctx] = val;
                }
                if reg == 4 {
                    self.claims[ctx] = 0;
                } // Completion
            }
        }
    }

    fn read_u8(&mut self, offset: u64) -> u8 {
        (self.read_u32(offset & !3) >> ((offset & 3) * 8)) as u8
    }
    fn read_u16(&mut self, offset: u64) -> u16 {
        (self.read_u32(offset & !3) >> ((offset & 3) * 8)) as u16
    }
    fn read_u64(&mut self, offset: u64) -> u64 {
        self.read_u32(offset) as u64
    }

    fn write_u8(&mut self, offset: u64, val: u8) {
        self.write_u32(offset & !3, val as u32);
    }
    fn write_u16(&mut self, offset: u64, val: u16) {
        self.write_u32(offset & !3, val as u32);
    }
    fn write_u64(&mut self, offset: u64, val: u64) {
        self.write_u32(offset, val as u32);
    }

    fn tick(&mut self) -> bool {
        let ctx = 1; // S-mode
        let mut max_prio = 0;
        let mut max_id = 0;

        // Optimization: Only check words that have pending interrupts
        // We only check the first 2 words (64 IRQs) for speed, as that's all we use.
        for w in 0..2 {
            let pending = self.pending[w];
            let enable = self.enables[ctx]; // Simplified: assuming enable reg 0 matches pending reg 0

            let active = pending & enable;

            if active != 0 {
                // Iterate bits in this word
                for b in 0..32 {
                    if (active & (1 << b)) != 0 {
                        let id = (w * 32) + b;
                        if id == 0 {
                            continue;
                        }
                        let prio = self.priorities[id];
                        if prio > max_prio && prio > self.thresholds[ctx] {
                            max_prio = prio;
                            max_id = id as u32;
                        }
                    }
                }
            }
        }

        if max_id > 0 {
            self.claims[ctx] = max_id;
            return true;
        }
        false
    }

    fn as_plic_mut(&mut self) -> Option<&mut Plic> {
        Some(self)
    }
}
