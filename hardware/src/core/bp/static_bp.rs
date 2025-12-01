use super::{BranchPredictor, btb::Btb, ras::Ras};

pub struct StaticPredictor {
    btb: Btb,
    ras: Ras,
}

impl StaticPredictor {
    pub fn new(btb_size: usize, ras_size: usize) -> Self {
        Self {
            btb: Btb::new(btb_size),
            ras: Ras::new(ras_size),
        }
    }
}

impl BranchPredictor for StaticPredictor {
    fn predict_branch(&self, _pc: u64) -> (bool, Option<u64>) {
        (false, None)
    }

    fn update_branch(&mut self, pc: u64, _taken: bool, target: Option<u64>) {
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
