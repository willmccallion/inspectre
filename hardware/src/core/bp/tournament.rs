use super::{BranchPredictor, btb::Btb, ras::Ras};
use crate::config::TournamentConfig;

pub struct TournamentPredictor {
    btb: Btb,
    ras: Ras,
    ghr: u64,

    global_pht: Vec<u8>,
    global_mask: usize,

    local_history_table: Vec<u16>,
    local_hist_mask: usize,

    local_pht: Vec<u8>,
    local_pred_mask: usize,

    choice_pht: Vec<u8>,
}

impl TournamentPredictor {
    pub fn new(config: &TournamentConfig, btb_size: usize, ras_size: usize) -> Self {
        let global_size = 1 << config.global_size_bits;
        let local_hist_size = 1 << config.local_hist_bits;
        let local_pred_size = 1 << config.local_pred_bits;

        Self {
            btb: Btb::new(btb_size),
            ras: Ras::new(ras_size),
            ghr: 0,

            global_pht: vec![1; global_size],
            global_mask: global_size - 1,

            local_history_table: vec![0; local_hist_size],
            local_hist_mask: local_hist_size - 1,

            local_pht: vec![1; local_pred_size],
            local_pred_mask: local_pred_size - 1,

            choice_pht: vec![1; global_size],
        }
    }

    fn get_global_prediction(&self, idx: usize) -> bool {
        self.global_pht[idx] >= 2
    }

    fn get_local_prediction(&self, pc: u64) -> bool {
        let lh_idx = (pc as usize) & self.local_hist_mask;
        let pattern = self.local_history_table[lh_idx];
        let pred_idx = (pattern as usize) & self.local_pred_mask;
        self.local_pht[pred_idx] >= 2
    }
}

impl BranchPredictor for TournamentPredictor {
    fn predict_branch(&self, pc: u64) -> (bool, Option<u64>) {
        let g_idx = ((self.ghr ^ pc) as usize) & self.global_mask;

        let global_taken = self.get_global_prediction(g_idx);
        let local_taken = self.get_local_prediction(pc);

        let use_global = self.choice_pht[g_idx] >= 2;
        let taken = if use_global {
            global_taken
        } else {
            local_taken
        };

        if taken {
            (true, self.btb.lookup(pc))
        } else {
            (false, None)
        }
    }

    fn update_branch(&mut self, pc: u64, taken: bool, target: Option<u64>) {
        let g_idx = ((self.ghr ^ pc) as usize) & self.global_mask;

        let global_correct = self.get_global_prediction(g_idx) == taken;
        let local_correct = self.get_local_prediction(pc) == taken;

        if global_correct != local_correct {
            let choice = &mut self.choice_pht[g_idx];
            if global_correct {
                if *choice < 3 {
                    *choice += 1;
                }
            } else if *choice > 0 {
                *choice -= 1;
            }
        }

        // Update Global
        let g_cnt = &mut self.global_pht[g_idx];
        if taken {
            if *g_cnt < 3 {
                *g_cnt += 1;
            }
        } else if *g_cnt > 0 {
            *g_cnt -= 1;
        }
        self.ghr = ((self.ghr << 1) | (taken as u64)) & (self.global_mask as u64);

        // Update Local
        let lh_idx = (pc as usize) & self.local_hist_mask;
        let pattern = self.local_history_table[lh_idx];
        let pred_idx = (pattern as usize) & self.local_pred_mask;

        let l_cnt = &mut self.local_pht[pred_idx];
        if taken {
            if *l_cnt < 3 {
                *l_cnt += 1;
            }
        } else if *l_cnt > 0 {
            *l_cnt -= 1;
        }

        // Update local history pattern
        self.local_history_table[lh_idx] =
            ((pattern << 1) | (taken as u16)) & (self.local_pred_mask as u16);

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
