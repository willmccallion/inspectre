use super::ReplacementPolicy;

pub struct RandomPolicy {
    ways: usize,
    state: u64,
}

impl RandomPolicy {
    pub fn new(_sets: usize, ways: usize) -> Self {
        Self {
            ways,
            state: 123456789,
        }
    }
}

impl ReplacementPolicy for RandomPolicy {
    fn update(&mut self, _set: usize, _way: usize) {}

    fn get_victim(&mut self, _set: usize) -> usize {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        (x as usize) % self.ways
    }
}
