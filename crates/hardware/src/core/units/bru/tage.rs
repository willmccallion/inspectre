//! TAGE (Tagged Geometric History Length) Branch Predictor.
//!
//! TAGE uses a base bimodal predictor and multiple tagged banks indexed with
//! geometrically increasing history lengths. It provides high accuracy by
//! matching long history patterns while falling back to shorter histories
//! or the base predictor when necessary.
//!
//! # Performance
//!
//! - **Time Complexity:**
//!   - `predict()`: O(B) where B is the number of banks (typically 4-6)
//!   - `update()`: O(B)
//! - **Space Complexity:** O(T × B) where T is table size per bank
//! - **Hardware Cost:** High - multiple table lookups, priority selection
//! - **Best Case:** Complex history-correlated patterns with varying lengths
//! - **Worst Case:** Random or completely uncorrelated branches (~50% accuracy)

use super::{BranchPredictor, btb::Btb, ras::Ras};
use crate::config::TageConfig;

/// An entry in a TAGE bank.
#[derive(Clone, Default)]
struct TageEntry {
    /// Tag for matching the history/PC hash.
    tag: u16,
    /// 3-bit saturating counter for prediction.
    ctr: i8,
    /// 2-bit useful counter for replacement policy.
    u: u8,
}

/// TAGE Predictor structure.
pub struct TagePredictor {
    /// Branch Target Buffer.
    btb: Btb,
    /// Return Address Stack.
    ras: Ras,
    /// Global History Register.
    ghr: u64,

    /// Base bimodal predictor table.
    base: Vec<i8>,
    /// Tagged component banks.
    banks: Vec<Vec<TageEntry>>,

    /// Geometric history lengths for each bank.
    hist_lengths: Vec<usize>,
    /// Tag widths for each bank.
    tag_widths: Vec<usize>,
    /// Mask for indexing the tables.
    table_mask: usize,

    /// Index of the bank providing the current prediction.
    provider_bank: usize,
    /// Index of the alternative bank.
    alt_bank: usize,

    /// Counter for periodic reset of useful bits.
    clock_counter: u32,
    /// Interval for resetting useful bits.
    reset_interval: u32,
}

impl TagePredictor {
    /// Creates a new TAGE Predictor based on configuration.
    pub fn new(config: &TageConfig, btb_size: usize, ras_size: usize) -> Self {
        assert!(
            config.table_size.is_power_of_two(),
            "TAGE table size must be power of 2"
        );

        let (hist_lengths, tag_widths, num_banks) = if !config.history_lengths.is_empty() {
            (
                config.history_lengths.clone(),
                config.tag_widths.clone(),
                config.num_banks,
            )
        } else {
            (vec![5, 15, 44, 130], vec![9, 9, 10, 10], 4)
        };

        assert_eq!(
            hist_lengths.len(),
            num_banks,
            "TAGE: History lengths vector must match num_banks"
        );
        assert_eq!(
            tag_widths.len(),
            num_banks,
            "TAGE: Tag widths vector must match num_banks"
        );

        let mut banks = Vec::new();
        for _ in 0..num_banks {
            banks.push(vec![TageEntry::default(); config.table_size]);
        }

        Self {
            btb: Btb::new(btb_size),
            ras: Ras::new(ras_size),
            ghr: 0,
            base: vec![0; config.table_size],
            banks,
            hist_lengths,
            tag_widths,
            table_mask: config.table_size - 1,

            provider_bank: 0,
            alt_bank: 0,
            clock_counter: 0,
            reset_interval: config.reset_interval,
        }
    }

    /// Folds a wide value into `bits` width by XOR-compressing.
    fn fold(val: u64, bits: usize) -> u64 {
        if bits == 0 || bits >= 64 {
            return val;
        }
        let mask = (1u64 << bits) - 1;
        let mut r = 0u64;
        let mut v = val;
        while v != 0 {
            r ^= v & mask;
            v >>= bits;
        }
        r
    }

    /// Masks the GHR to the history length for a given bank.
    fn bank_history(&self, bank: usize) -> u64 {
        let len = self.hist_lengths[bank];
        if len >= 64 {
            self.ghr
        } else {
            self.ghr & ((1u64 << len) - 1)
        }
    }

    /// Calculates the index for a specific bank using PC and GHR.
    fn index(&self, pc: u64, bank: usize) -> usize {
        let table_bits = (self.table_mask + 1).trailing_zeros() as usize;
        let h = self.bank_history(bank);
        let pc_hash = pc >> 2;
        let h_folded = Self::fold(h, table_bits);
        let h_folded2 = Self::fold(h, table_bits.wrapping_sub(1).max(1));
        (pc_hash as usize ^ h_folded as usize ^ h_folded2 as usize) & self.table_mask
    }

    /// Calculates the tag for a specific bank using PC and GHR.
    fn tag(&self, pc: u64, bank: usize) -> u16 {
        let width = self.tag_widths[bank];
        let h = self.bank_history(bank);
        let pc_hash = pc >> 2;
        let h_folded = Self::fold(h, width);
        let h_folded2 = Self::fold(h, width.wrapping_sub(1).max(1));
        ((pc_hash as usize ^ h_folded as usize ^ h_folded2 as usize) & ((1 << width) - 1)) as u16
    }
}

impl BranchPredictor for TagePredictor {
    /// Predicts branch direction and target.
    ///
    /// Searches the tagged banks for the longest history match (provider).
    /// If no match is found, uses the base predictor.
    fn predict_branch(&self, pc: u64) -> (bool, Option<u64>) {
        let mut provider = 0;
        let num_banks = self.banks.len();

        for i in (0..num_banks).rev() {
            let idx = self.index(pc, i);
            let tag = self.tag(pc, i);
            if self.banks[i][idx].tag == tag {
                provider = i + 1;
                break;
            }
        }

        if provider > 0 {
            let bank_idx = provider - 1;
            let idx = self.index(pc, bank_idx);
            let ctr = self.banks[bank_idx][idx].ctr;
            return (ctr >= 0, self.btb.lookup(pc));
        }

        let base_idx = (pc as usize) & self.table_mask;
        (self.base[base_idx] >= 0, self.btb.lookup(pc))
    }

    /// Updates the predictor state.
    ///
    /// Updates the provider bank, and potentially allocates a new entry in a
    /// bank with longer history on mispredictions. Also handles periodic
    /// resetting of useful bits.
    fn update_branch(&mut self, pc: u64, taken: bool, target: Option<u64>) {
        self.clock_counter += 1;
        if self.clock_counter >= self.reset_interval {
            self.clock_counter = 0;
            for bank in &mut self.banks {
                for entry in bank {
                    entry.u >>= 1;
                }
            }
        }

        let mut provider = 0;
        let mut alt = 0;
        let num_banks = self.banks.len();

        for i in (0..num_banks).rev() {
            let idx = self.index(pc, i);
            let tag = self.tag(pc, i);
            if self.banks[i][idx].tag == tag {
                if provider == 0 {
                    provider = i + 1;
                } else if alt == 0 {
                    alt = i + 1;
                    break;
                }
            }
        }
        self.provider_bank = provider;
        self.alt_bank = alt;

        let pred_taken = if self.provider_bank > 0 {
            let idx = self.index(pc, self.provider_bank - 1);
            self.banks[self.provider_bank - 1][idx].ctr >= 0
        } else {
            let base_idx = (pc as usize) & self.table_mask;
            self.base[base_idx] >= 0
        };

        let alt_taken = if self.alt_bank > 0 {
            let idx = self.index(pc, self.alt_bank - 1);
            self.banks[self.alt_bank - 1][idx].ctr >= 0
        } else {
            let base_idx = (pc as usize) & self.table_mask;
            self.base[base_idx] >= 0
        };

        let mispredicted = pred_taken != taken;

        if self.provider_bank > 0 {
            let bank_idx = self.provider_bank - 1;
            let idx = self.index(pc, bank_idx);
            let e = &mut self.banks[bank_idx][idx];

            if taken {
                if e.ctr < 3 {
                    e.ctr += 1;
                }
            } else if e.ctr > -4 {
                e.ctr -= 1;
            }

            if !mispredicted && (alt_taken != taken) && e.u < 3 {
                e.u += 1;
            }
        } else {
            let base_idx = (pc as usize) & self.table_mask;
            let b = &mut self.base[base_idx];
            if taken {
                if *b < 1 {
                    *b += 1;
                }
            } else if *b > -2 {
                *b -= 1;
            }
        }

        if mispredicted {
            let start_bank = if self.provider_bank == 0 {
                0
            } else {
                self.provider_bank
            };

            if start_bank < num_banks {
                let mut allocated = false;
                for i in start_bank..num_banks {
                    let idx = self.index(pc, i);
                    let tag = self.tag(pc, i);
                    let e = &mut self.banks[i][idx];

                    if e.u == 0 {
                        e.tag = tag;
                        e.ctr = if taken { 0 } else { -1 };
                        e.u = 1;
                        allocated = true;
                        break;
                    }
                }

                if !allocated {
                    for i in start_bank..num_banks {
                        let idx = self.index(pc, i);
                        if self.banks[i][idx].u > 0 {
                            self.banks[i][idx].u -= 1;
                        }
                    }
                }
            }
        }

        self.ghr = (self.ghr << 1) | (if taken { 1 } else { 0 });

        if let Some(tgt) = target {
            self.btb.update(pc, tgt);
        }
    }

    /// Predicts the target of a jump instruction using the BTB.
    fn predict_btb(&self, pc: u64) -> Option<u64> {
        self.btb.lookup(pc)
    }

    /// Handles a function call by pushing the return address to the RAS.
    fn on_call(&mut self, pc: u64, ret_addr: u64, target: u64) {
        self.ras.push(ret_addr);
        self.btb.update(pc, target);
    }

    /// Predicts the return address using the RAS.
    fn predict_return(&self) -> Option<u64> {
        self.ras.top()
    }

    /// Handles a function return by popping from the RAS.
    fn on_return(&mut self) {
        self.ras.pop();
    }

    fn speculate(&mut self, _pc: u64, taken: bool) {
        self.ghr = (self.ghr << 1) | (if taken { 1 } else { 0 });
    }

    fn snapshot_history(&self) -> u64 {
        self.ghr
    }

    fn repair_history(&mut self, ghr: u64) {
        self.ghr = ghr;
    }
}
