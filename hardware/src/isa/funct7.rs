pub const DEFAULT: u32 = 0b0000000;
pub const SUB: u32 = 0b0100000;
pub const SRA: u32 = 0b0100000;
pub const M_EXTENSION: u32 = 0b0000001;

pub const FADD: u32 = 0b00000;
pub const FSUB: u32 = 0b00001;
pub const FMUL: u32 = 0b00010;
pub const FDIV: u32 = 0b00011;
pub const FSQRT: u32 = 0b01011;
pub const FSGNJ: u32 = 0b00100;
pub const FMIN_MAX: u32 = 0b00101;
pub const FCMP: u32 = 0b10100;
pub const FCLASS_MV_X_F: u32 = 0b11100;
pub const FCVT_W_F: u32 = 0b11000;
pub const FCVT_F_W: u32 = 0b11010;
pub const FMV_F_X: u32 = 0b11110;
pub const FCVT_DS: u32 = 0b01000;
