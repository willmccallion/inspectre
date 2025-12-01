use super::Prefetcher;

#[derive(Default, Clone, Copy)]
struct StreamEntry {
    last_addr: u64,
    stride: i64,
    confidence: u8, // 2-bit saturating counter
}

pub struct StridePrefetcher {
    table: Vec<StreamEntry>,
    line_bytes: u64,
    table_mask: usize,
    degree: usize,
}

impl StridePrefetcher {
    pub fn new(line_bytes: usize, table_size: usize, degree: usize) -> Self {
        let safe_size = if table_size > 0 && (table_size & (table_size - 1)) == 0 {
            table_size
        } else {
            64
        };

        Self {
            table: vec![StreamEntry::default(); safe_size],
            line_bytes: line_bytes as u64,
            table_mask: safe_size - 1,
            degree: if degree == 0 { 1 } else { degree },
        }
    }
}

impl Prefetcher for StridePrefetcher {
    fn observe(&mut self, addr: u64, _hit: bool) -> Vec<u64> {
        let idx = ((addr >> 6) as usize) & self.table_mask;
        let entry = &mut self.table[idx];

        let current_stride = (addr as i64) - (entry.last_addr as i64);
        let mut prefetches = Vec::new();

        if current_stride == entry.stride {
            if entry.confidence < 3 {
                entry.confidence += 1;
            } else {
                for k in 1..=self.degree {
                    let lookahead = entry.stride * k as i64;
                    let target = (addr as i64 + lookahead) as u64;
                    // Align to cache line
                    let aligned = target & !(self.line_bytes - 1);
                    prefetches.push(aligned);
                }
            }
        } else if entry.confidence > 0 {
            entry.confidence -= 1;
        } else {
            entry.stride = current_stride;
        }

        entry.last_addr = addr;
        prefetches
    }
}
