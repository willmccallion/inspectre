use super::ReplacementPolicy;

pub struct FifoPolicy {
    next_way: Vec<usize>,
    ways: usize,
}

impl FifoPolicy {
    pub fn new(sets: usize, ways: usize) -> Self {
        Self {
            next_way: vec![0; sets],
            ways,
        }
    }
}

impl ReplacementPolicy for FifoPolicy {
    fn update(&mut self, set: usize, way: usize) {
        if self.next_way[set] == way {
            self.next_way[set] = (self.next_way[set] + 1) % self.ways;
        }
    }

    fn get_victim(&mut self, set: usize) -> usize {
        self.next_way[set]
    }
}
