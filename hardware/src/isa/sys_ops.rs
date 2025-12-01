#![allow(dead_code)]

// Full Instruction Encodings (for exact matching)
pub const ECALL: u32 = 0x0000_0073;
pub const EBREAK: u32 = 0x0010_0073;
pub const MRET: u32 = 0x3020_0073;
pub const SRET: u32 = 0x1020_0073;
pub const WFI: u32 = 0x1050_0073;
pub const SFENCE_VMA: u32 = 0x1200_0073; // sfence.vma x0, x0

// CSR Funct3 Codes
pub const CSRRW: u32 = 0b001;
pub const CSRRS: u32 = 0b010;
pub const CSRRC: u32 = 0b011;
pub const CSRRWI: u32 = 0b101;
pub const CSRRSI: u32 = 0b110;
pub const CSRRCI: u32 = 0b111;

// Trap Causes
pub const CAUSE_MACHINE_TIMER: u64 = 0x8000_0000_0000_0007;
pub const CAUSE_USER_ECALL: u64 = 8;

// Syscall Numbers
pub const SYS_EXIT: u64 = 93;
