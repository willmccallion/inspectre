pub struct RegisterFile {
    regs: [u64; 32],
    fregs: [f64; 32],
}

impl RegisterFile {
    pub fn new() -> Self {
        Self {
            regs: [0; 32],
            fregs: [0.0; 32],
        }
    }

    pub fn read(&self, idx: usize) -> u64 {
        if idx == 0 { 0 } else { self.regs[idx] }
    }

    pub fn write(&mut self, idx: usize, val: u64) {
        if idx != 0 {
            self.regs[idx] = val;
        }
    }

    pub fn read_f(&self, idx: usize) -> u64 {
        self.fregs[idx].to_bits()
    }

    pub fn write_f(&mut self, idx: usize, val: u64) {
        self.fregs[idx] = f64::from_bits(val);
    }

    pub fn dump(&self) {
        for i in (0..32).step_by(2) {
            println!(
                "x{:<2}={:#018x} x{:<2}={:#018x}",
                i,
                self.regs[i],
                i + 1,
                self.regs[i + 1]
            );
        }
    }
}
