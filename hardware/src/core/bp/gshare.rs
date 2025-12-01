use super::{BranchPredictor, btb::Btb, ras::Ras};

const TABLE_BITS: usize = 12; // 4096 entries
const TABLE_SIZE: usize = 1 << TABLE_BITS;

pub struct GSharePredictor {
    ghr: u64,
    // 2-bit saturating counters:
    // 0 = Strongly Not Taken, 1 = Weakly Not Taken,
    // 2 = Weakly Taken, 3 = Strongly Taken
    pht: Vec<u8>,
    btb: Btb,
    ras: Ras,
}

impl GSharePredictor {
    pub fn new(btb_size: usize, ras_size: usize) -> Self {
        Self {
            ghr: 0,
            pht: vec![1; TABLE_SIZE], // Initialize to Weakly Not Taken
            btb: Btb::new(btb_size),
            ras: Ras::new(ras_size),
        }
    }

    fn index(&self, pc: u64) -> usize {
        // GShare Index = (PC ^ GlobalHistory) % TableSize
        let pc_part = (pc >> 2) & ((TABLE_SIZE as u64) - 1);
        let ghr_part = self.ghr & ((TABLE_SIZE as u64) - 1);
        (pc_part ^ ghr_part) as usize
    }
}

impl BranchPredictor for GSharePredictor {
    fn predict_branch(&self, pc: u64) -> (bool, Option<u64>) {
        let idx = self.index(pc);
        let counter = self.pht[idx];
        let taken = counter >= 2;

        if taken {
            (true, self.btb.lookup(pc))
        } else {
            (false, None)
        }
    }

    fn update_branch(&mut self, pc: u64, taken: bool, target: Option<u64>) {
        let idx = self.index(pc);
        let counter = self.pht[idx];

        // Update 2-bit saturating counter
        if taken && counter < 3 {
            self.pht[idx] += 1;
        } else if counter > 0 {
            self.pht[idx] -= 1;
        }

        // Update Global History Register
        self.ghr = ((self.ghr << 1) | if taken { 1 } else { 0 }) & ((TABLE_SIZE as u64) - 1);

        if let Some(tgt) = target {
            self.btb.update(pc, tgt);
        }
    }

    fn predict_btb(&self, pc: u64) -> Option<u64> {
        self.btb.lookup(pc)
    }

    fn on_call(&mut self, pc: u64, ret_addr: u64, target: u64) {
        self.ras.push(ret_addr);
        self.btb.update(pc, target);
    }

    fn predict_return(&self) -> Option<u64> {
        self.ras.top()
    }

    fn on_return(&mut self) {
        self.ras.pop();
    }
}
