use super::Prefetcher;

pub struct NextLinePrefetcher {
    line_bytes: u64,
    degree: usize,
}

impl NextLinePrefetcher {
    pub fn new(line_bytes: usize, degree: usize) -> Self {
        Self {
            line_bytes: line_bytes as u64,
            degree: if degree == 0 { 1 } else { degree },
        }
    }
}

impl Prefetcher for NextLinePrefetcher {
    fn observe(&mut self, addr: u64, _hit: bool) -> Vec<u64> {
        let mut prefetches = Vec::new();

        for k in 1..=self.degree {
            let offset = self.line_bytes * k as u64;
            let target = (addr & !(self.line_bytes - 1)) + offset;
            prefetches.push(target);
        }
        prefetches
    }
}
