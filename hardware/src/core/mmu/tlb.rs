#[derive(Clone, Copy, Default)]
struct TlbEntry {
    vpn: u64,
    ppn: u64,
    valid: bool,
    r: bool,
    w: bool,
    x: bool,
    u: bool,
}

pub struct Tlb {
    entries: Vec<TlbEntry>,
    size: usize,
    repl_ptr: usize,
}

impl Tlb {
    pub fn new(size: usize) -> Self {
        Self {
            entries: vec![TlbEntry::default(); size],
            size,
            repl_ptr: 0,
        }
    }

    /// Returns (Physical Page Number, Read, Write, Execute, User)
    pub fn lookup(&self, vpn: u64) -> Option<(u64, bool, bool, bool, bool)> {
        for entry in &self.entries {
            if entry.valid && entry.vpn == vpn {
                return Some((entry.ppn, entry.r, entry.w, entry.x, entry.u));
            }
        }
        None
    }

    pub fn insert(&mut self, vpn: u64, ppn: u64, pte: u64) {
        let r = (pte >> 1) & 1 != 0;
        let w = (pte >> 2) & 1 != 0;
        let x = (pte >> 3) & 1 != 0;
        let u = (pte >> 4) & 1 != 0;

        self.entries[self.repl_ptr] = TlbEntry {
            vpn,
            ppn,
            valid: true,
            r,
            w,
            x,
            u,
        };
        self.repl_ptr = (self.repl_ptr + 1) % self.size;
    }

    pub fn flush(&mut self) {
        for e in &mut self.entries {
            e.valid = false;
        }
    }
}
