#[derive(Clone, Copy, Default)]
struct BtbEntry {
    tag: u64,
    target: u64,
    valid: bool,
}

pub struct Btb {
    table: Vec<BtbEntry>,
    size: usize,
}

impl Btb {
    pub fn new(size: usize) -> Self {
        Self {
            table: vec![BtbEntry::default(); size],
            size,
        }
    }

    fn index(&self, pc: u64) -> usize {
        ((pc >> 2) as usize) & (self.size - 1)
    }

    pub fn lookup(&self, pc: u64) -> Option<u64> {
        let idx = self.index(pc);
        let e = self.table[idx];
        if e.valid && e.tag == pc {
            Some(e.target)
        } else {
            None
        }
    }

    pub fn update(&mut self, pc: u64, target: u64) {
        let idx = self.index(pc);
        self.table[idx] = BtbEntry {
            tag: pc,
            target,
            valid: true,
        };
    }
}
