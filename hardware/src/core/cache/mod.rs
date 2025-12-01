pub mod policies;

use self::policies::{FifoPolicy, LruPolicy, PlruPolicy, RandomPolicy, ReplacementPolicy};
use crate::config::CacheConfig;
use crate::core::prefetch::{NextLinePrefetcher, Prefetcher, StridePrefetcher};

#[derive(Clone, Default)]
struct CacheLine {
    tag: u64,
    valid: bool,
    dirty: bool,
}

pub struct CacheSim {
    pub latency: u64,
    pub enabled: bool,
    pub prefetcher: Option<Box<dyn Prefetcher>>,
    lines: Vec<CacheLine>,
    num_sets: usize,
    ways: usize,
    line_bytes: usize,
    policy: Box<dyn ReplacementPolicy>,
}

impl CacheSim {
    pub fn new(config: &CacheConfig) -> Self {
        let safe_ways = if config.ways == 0 { 1 } else { config.ways };
        let safe_line = if config.line_bytes == 0 {
            64
        } else {
            config.line_bytes
        };
        let safe_size = if config.size_bytes == 0 {
            4096
        } else {
            config.size_bytes
        };

        let num_lines = safe_size / safe_line;
        let num_sets = num_lines / safe_ways;

        let policy: Box<dyn ReplacementPolicy> = match config.policy.as_str() {
            "FIFO" => Box::new(FifoPolicy::new(num_sets, safe_ways)),
            "Random" => Box::new(RandomPolicy::new(num_sets, safe_ways)),
            "PLRU" => Box::new(PlruPolicy::new(num_sets, safe_ways)),
            _ => Box::new(LruPolicy::new(num_sets, safe_ways)),
        };

        let prefetcher: Option<Box<dyn Prefetcher>> = match config.prefetcher.as_str() {
            "NextLine" => Some(Box::new(NextLinePrefetcher::new(
                safe_line,
                config.prefetch_degree,
            ))),
            "Stride" => Some(Box::new(StridePrefetcher::new(
                safe_line,
                config.prefetch_table_size,
                config.prefetch_degree,
            ))),
            _ => None,
        };

        Self {
            lines: vec![CacheLine::default(); num_sets * safe_ways],
            num_sets,
            ways: safe_ways,
            line_bytes: safe_line,
            latency: config.latency,
            enabled: config.enabled,
            policy,
            prefetcher,
        }
    }

    pub fn contains(&self, addr: u64) -> bool {
        if !self.enabled {
            return false;
        }

        let set_index = ((addr as usize) / self.line_bytes) % self.num_sets;
        let tag = addr / (self.line_bytes * self.num_sets) as u64;
        let base_idx = set_index * self.ways;

        for i in 0..self.ways {
            let idx = base_idx + i;
            if self.lines[idx].valid && self.lines[idx].tag == tag {
                return true;
            }
        }
        false
    }

    fn install_line(&mut self, addr: u64, is_write: bool, next_level_latency: u64) -> u64 {
        let set_index = ((addr as usize) / self.line_bytes) % self.num_sets;
        let tag = addr / (self.line_bytes * self.num_sets) as u64;
        let base_idx = set_index * self.ways;

        let victim_way = self.policy.get_victim(set_index);
        let victim_idx = base_idx + victim_way;
        let mut penalty = 0;

        // Write-back if dirty
        if self.lines[victim_idx].valid && self.lines[victim_idx].dirty {
            penalty += next_level_latency;
        }

        // Install new line
        self.lines[victim_idx] = CacheLine {
            tag,
            valid: true,
            dirty: is_write,
        };
        self.policy.update(set_index, victim_way);

        penalty
    }

    pub fn access(&mut self, addr: u64, is_write: bool, next_level_latency: u64) -> (bool, u64) {
        if !self.enabled {
            return (false, 0);
        }

        let set_index = ((addr as usize) / self.line_bytes) % self.num_sets;
        let tag = addr / (self.line_bytes * self.num_sets) as u64;
        let base_idx = set_index * self.ways;

        let mut hit = false;
        let mut penalty = 0;

        // Check for Hit
        for i in 0..self.ways {
            let idx = base_idx + i;
            if self.lines[idx].valid && self.lines[idx].tag == tag {
                self.policy.update(set_index, i);
                if is_write {
                    self.lines[idx].dirty = true;
                }
                hit = true;
                break;
            }
        }

        // Handle Miss
        if !hit {
            // Use our new helper to install the line
            penalty += self.install_line(addr, is_write, next_level_latency);
        }

        // Trigger prefetcher
        let mut prefetches = Vec::new();
        if let Some(ref mut pref) = self.prefetcher {
            prefetches = pref.observe(addr, hit);
        }

        for target in prefetches {
            if !self.contains(target) {
                self.install_line(target, false, next_level_latency);
            }
        }

        (hit, penalty)
    }
}
