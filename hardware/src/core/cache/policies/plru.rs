use super::ReplacementPolicy;

pub struct PlruPolicy {
    // One bit per way per set.
    usage: Vec<u64>,
    ways: usize,
}

impl PlruPolicy {
    pub fn new(sets: usize, ways: usize) -> Self {
        Self {
            usage: vec![0; sets],
            ways,
        }
    }
}

impl ReplacementPolicy for PlruPolicy {
    fn update(&mut self, set: usize, way: usize) {
        let mask = 1 << way;
        self.usage[set] |= mask;

        // If all bits are set, reset all except the current one
        let all_ones = (1 << self.ways) - 1;
        if (self.usage[set] & all_ones) == all_ones {
            self.usage[set] = mask;
        }
    }

    fn get_victim(&mut self, set: usize) -> usize {
        // Find the first way with a 0 bit
        for i in 0..self.ways {
            if (self.usage[set] >> i) & 1 == 0 {
                return i;
            }
        }
        // Should not happen if logic is correct
        0
    }
}
