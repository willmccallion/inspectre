//! Hart lifecycle and debug hooks.

use super::Hart;

impl Hart {
    /// Dumps the current hart state (PC and registers) to stdout.
    pub fn dump_state(&self) {
        println!("PC = {:#018x}", self.pc);
        self.regs.dump();
    }
}
