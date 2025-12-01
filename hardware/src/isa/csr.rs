#![allow(dead_code)]

// Machine Information
pub const MVENDORID: u32 = 0xF11;
pub const MARCHID: u32 = 0xF12;
pub const MIMPID: u32 = 0xF13;
pub const MHARTID: u32 = 0xF14;

// Machine Trap Setup
pub const MSTATUS: u32 = 0x300;
pub const MISA: u32 = 0x301;
pub const MEDELEG: u32 = 0x302;
pub const MIDELEG: u32 = 0x303;
pub const MIE: u32 = 0x304;
pub const MTVEC: u32 = 0x305;
pub const MCOUNTEREN: u32 = 0x306;

// Machine Trap Handling
pub const MSCRATCH: u32 = 0x340;
pub const MEPC: u32 = 0x341;
pub const MCAUSE: u32 = 0x342;
pub const MTVAL: u32 = 0x343;
pub const MIP: u32 = 0x344;

// Supervisor Trap Setup
pub const SSTATUS: u32 = 0x100;
pub const SIE: u32 = 0x104;
pub const STVEC: u32 = 0x105;
pub const SCOUNTEREN: u32 = 0x106;

// Supervisor Trap Handling
pub const SSCRATCH: u32 = 0x140;
pub const SEPC: u32 = 0x141;
pub const SCAUSE: u32 = 0x142;
pub const STVAL: u32 = 0x143;
pub const SIP: u32 = 0x144;

// Supervisor Protection and Translation
pub const SATP: u32 = 0x180;

// Performance Counters
pub const CYCLE: u32 = 0xC00;
pub const TIME: u32 = 0xC01;
pub const INSTRET: u32 = 0xC02;
pub const MCYCLE: u32 = 0xB00;
pub const MINSTRET: u32 = 0xB02;

pub const MSTATUS_UIE: u64 = 1 << 0;
pub const MSTATUS_SIE: u64 = 1 << 1;
pub const MSTATUS_MIE: u64 = 1 << 3;

pub const MIE_USIP: u64 = 1 << 0;
pub const MIE_SSIP: u64 = 1 << 1;
pub const MIE_MSIP: u64 = 1 << 3;
pub const MIE_UTIE: u64 = 1 << 4;
pub const MIE_STIE: u64 = 1 << 5;
pub const MIE_MTIE: u64 = 1 << 7;
pub const MIE_UEIP: u64 = 1 << 8;
pub const MIE_SEIP: u64 = 1 << 9;
pub const MIE_MEIP: u64 = 1 << 11;

pub const MIP_USIP: u64 = 1 << 0;
pub const MIP_SSIP: u64 = 1 << 1;
pub const MIP_MSIP: u64 = 1 << 3;
pub const MIP_UTIP: u64 = 1 << 4;
pub const MIP_STIP: u64 = 1 << 5;
pub const MIP_MTIP: u64 = 1 << 7;
pub const MIP_UEIP: u64 = 1 << 8;
pub const MIP_SEIP: u64 = 1 << 9;
pub const MIP_MEIP: u64 = 1 << 11;

// Custom debug CSR to trigger RequestedTrap
pub const CSR_SIM_PANIC: u32 = 0x8FF;

// Previous Interrupt Enables
pub const MSTATUS_SPIE: u64 = 1 << 5;
pub const MSTATUS_MPIE: u64 = 1 << 7;

// Previous Privilege Modes
pub const MSTATUS_SPP: u64 = 1 << 8;
pub const MSTATUS_MPP: u64 = 3 << 11;

// Floating Point Status (FS)
// 0 = Off, 1 = Initial, 2 = Clean, 3 = Dirty
pub const MSTATUS_FS: u64 = 3 << 13;
pub const MSTATUS_FS_OFF: u64 = 0 << 13;
pub const MSTATUS_FS_INIT: u64 = 1 << 13;
pub const MSTATUS_FS_CLEAN: u64 = 2 << 13;
pub const MSTATUS_FS_DIRTY: u64 = 3 << 13;

// Memory Privileges
pub const MSTATUS_SUM: u64 = 1 << 18; // Permit Supervisor User Memory access
pub const MSTATUS_MXR: u64 = 1 << 19; // Make eXecutable Readable

// SATP (Supervisor Address Translation and Protection)
pub const SATP_MODE_SHIFT: u64 = 60;
pub const SATP_MODE_BARE: u64 = 0;
pub const SATP_MODE_SV39: u64 = 8;
pub const SATP_MODE_SV48: u64 = 9;
