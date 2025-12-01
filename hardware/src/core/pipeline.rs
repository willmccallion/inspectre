use crate::core::control::ControlSignals;
use crate::core::types::Trap;

#[derive(Clone, Copy)]
pub struct IfId {
    pub pc: u64,
    pub inst: u32,
}

impl Default for IfId {
    fn default() -> Self {
        Self {
            inst: 0x0000_0013, // NOP
            pc: 0,
        }
    }
}

impl IdEx {
    pub fn bubble() -> Self {
        Self {
            inst: 0x0000_0013, // NOP
            ctrl: ControlSignals {
                reg_write: false,
                mem_write: false,
                mem_read: false,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

#[derive(Default, Clone)]
pub struct IdEx {
    pub pc: u64,
    pub inst: u32,
    pub rs1: usize,
    pub rs2: usize,
    pub rs3: usize,
    pub rd: usize,
    pub imm: i64,
    pub rv1: u64,
    pub rv2: u64,
    pub rv3: u64,
    pub ctrl: ControlSignals,
    pub trap: Option<Trap>,
}

#[derive(Default, Clone)]
pub struct ExMem {
    pub pc: u64,
    pub inst: u32,
    pub rd: usize,
    pub alu: u64,
    pub store_data: u64,
    pub ctrl: ControlSignals,
    pub trap: Option<Trap>,
}

#[derive(Default, Clone)]
pub struct MemWb {
    pub pc: u64,
    pub inst: u32,
    pub rd: usize,
    pub alu: u64,
    pub load_data: u64,
    pub ctrl: ControlSignals,
    pub trap: Option<Trap>,
}
