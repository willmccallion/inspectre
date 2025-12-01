use super::ReplacementPolicy;

pub struct LruPolicy {
    usage: Vec<Vec<usize>>,
}

impl LruPolicy {
    pub fn new(sets: usize, ways: usize) -> Self {
        let mut usage = Vec::with_capacity(sets);
        for _ in 0..sets {
            usage.push((0..ways).collect());
        }
        Self { usage }
    }
}

impl ReplacementPolicy for LruPolicy {
    fn update(&mut self, set: usize, way: usize) {
        let stack = &mut self.usage[set];
        if let Some(pos) = stack.iter().position(|&x| x == way) {
            stack.remove(pos);
        }
        stack.insert(0, way);
    }

    fn get_victim(&mut self, set: usize) -> usize {
        *self.usage[set].last().unwrap()
    }
}
