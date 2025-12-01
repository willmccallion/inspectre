pub struct Ras {
    stack: Vec<u64>,
    ptr: usize,
    capacity: usize,
}

impl Ras {
    pub fn new(capacity: usize) -> Self {
        Self {
            stack: vec![0; capacity],
            ptr: 0,
            capacity,
        }
    }

    pub fn push(&mut self, addr: u64) {
        if self.ptr < self.capacity {
            self.stack[self.ptr] = addr;
            self.ptr += 1;
        } else {
            self.stack[self.capacity - 1] = addr;
        }
    }

    pub fn pop(&mut self) -> Option<u64> {
        if self.ptr == 0 {
            None
        } else {
            self.ptr -= 1;
            Some(self.stack[self.ptr])
        }
    }

    pub fn top(&self) -> Option<u64> {
        if self.ptr == 0 {
            None
        } else {
            Some(self.stack[self.ptr - 1])
        }
    }
}
