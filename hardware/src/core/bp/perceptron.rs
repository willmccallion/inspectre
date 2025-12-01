use super::{BranchPredictor, btb::Btb, ras::Ras};
use crate::config::PerceptronConfig;

const THETA_COEFF: f64 = 1.93;
const THETA_BIAS: f64 = 14.0;

pub struct PerceptronPredictor {
    ghr: u64,
    table: Vec<i8>,
    history_length: usize,
    table_mask: usize,
    row_size: usize,
    threshold: i32,
    btb: Btb,
    ras: Ras,
}

impl PerceptronPredictor {
    pub fn new(config: &PerceptronConfig, btb_size: usize, ras_size: usize) -> Self {
        let table_entries = 1 << config.table_bits;
        let hist_len = config.history_length;
        let threshold = (THETA_COEFF * (hist_len as f64) + THETA_BIAS) as i32;
        let row_size = hist_len + 1;

        Self {
            ghr: 0,
            table: vec![0; table_entries * row_size],
            history_length: hist_len,
            table_mask: table_entries - 1,
            row_size,
            threshold,
            btb: Btb::new(btb_size),
            ras: Ras::new(ras_size),
        }
    }

    fn index(&self, pc: u64) -> usize {
        let pc_idx = (pc >> 2) as usize & self.table_mask;
        let hist_idx = (self.ghr as usize) & self.table_mask;
        pc_idx ^ hist_idx
    }

    fn output(&self, row_idx: usize) -> i32 {
        let base = row_idx * self.row_size;
        let mut y = self.table[base] as i32;

        for i in 0..self.history_length {
            let bit = if (self.ghr >> i) & 1 != 0 { 1 } else { -1 };
            y += (self.table[base + 1 + i] as i32) * bit;
        }
        y
    }
}

fn clamp_weight(v: i32) -> i8 {
    if v > 127 {
        127
    } else if v < -128 {
        -128
    } else {
        v as i8
    }
}

impl BranchPredictor for PerceptronPredictor {
    fn predict_branch(&self, pc: u64) -> (bool, Option<u64>) {
        let idx = self.index(pc);
        let y = self.output(idx);
        let taken = y >= 0;
        if taken {
            (true, self.btb.lookup(pc))
        } else {
            (false, None)
        }
    }

    fn update_branch(&mut self, pc: u64, taken: bool, target: Option<u64>) {
        let idx = self.index(pc);
        let y = self.output(idx);
        let t = if taken { 1 } else { -1 };

        if y.abs() <= self.threshold || (y >= 0) != taken {
            let base = idx * self.row_size;
            // Update Bias
            let v = self.table[base] as i32 + t;
            self.table[base] = clamp_weight(v);

            // Update Weights
            for i in 0..self.history_length {
                let x = if (self.ghr >> i) & 1 != 0 { 1 } else { -1 };
                let w_idx = base + 1 + i;
                let v = self.table[w_idx] as i32 + t * x;
                self.table[w_idx] = clamp_weight(v);
            }
        }

        // Update GHR
        self.ghr =
            ((self.ghr << 1) | if taken { 1 } else { 0 }) & ((1u64 << self.history_length) - 1);

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
