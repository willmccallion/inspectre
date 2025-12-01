pub trait MemoryController {
    fn access_latency(&mut self, addr: u64) -> u64;
}

pub struct SimpleController {
    latency: u64,
}

impl SimpleController {
    pub fn new(latency: u64) -> Self {
        Self { latency }
    }
}

impl MemoryController for SimpleController {
    fn access_latency(&mut self, _addr: u64) -> u64 {
        self.latency
    }
}

/// Models a single-bank DRAM with Row Buffer management.
pub struct DramController {
    last_row: Option<u64>,
    t_cas: u64,
    t_ras: u64,
    t_pre: u64,
    row_mask: u64,
}

impl DramController {
    pub fn new(t_cas: u64, t_ras: u64, t_pre: u64) -> Self {
        // Assume 2KB Row Size (11 bits offset)
        Self {
            last_row: None,
            t_cas,
            t_ras,
            t_pre,
            row_mask: !2047,
        }
    }
}

impl MemoryController for DramController {
    fn access_latency(&mut self, addr: u64) -> u64 {
        let row = addr & self.row_mask;

        match self.last_row {
            Some(open_row) if open_row == row => {
                // Row Buffer Hit: Just CAS
                self.t_cas
            }
            Some(_) => {
                // Row Buffer Conflict (Miss): Precharge Old + Activate New + CAS
                self.last_row = Some(row);
                self.t_pre + self.t_ras + self.t_cas
            }
            None => {
                // Bank Idle: Activate New + CAS
                self.last_row = Some(row);
                self.t_ras + self.t_cas
            }
        }
    }
}
