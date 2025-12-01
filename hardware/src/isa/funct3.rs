pub const LB: u32 = 0b000;
pub const LH: u32 = 0b001;
pub const LW: u32 = 0b010;
pub const LD: u32 = 0b011;
pub const LBU: u32 = 0b100;
pub const LHU: u32 = 0b101;
pub const LWU: u32 = 0b110;

pub const SB: u32 = 0b000;
pub const SH: u32 = 0b001;
pub const SW: u32 = 0b010;
pub const SD: u32 = 0b011;

pub const BEQ: u32 = 0b000;
pub const BNE: u32 = 0b001;
pub const BLT: u32 = 0b100;
pub const BGE: u32 = 0b101;
pub const BLTU: u32 = 0b110;
pub const BGEU: u32 = 0b111;

pub const ADD_SUB: u32 = 0b000;
pub const SLL: u32 = 0b001;
pub const SLT: u32 = 0b010;
pub const SLTU: u32 = 0b011;
pub const XOR: u32 = 0b100;
pub const SRL_SRA: u32 = 0b101;
pub const OR: u32 = 0b110;
pub const AND: u32 = 0b111;

pub const FMIN: u32 = 0b000;
pub const FMAX: u32 = 0b001;
pub const FEQ: u32 = 0b000;
pub const FLT: u32 = 0b001;
pub const FLE: u32 = 0b010;
pub const FCLASS: u32 = 0b001;
pub const FMV_X_W: u32 = 0b000;
pub const FSGNJ: u32 = 0b000;
pub const FSGNJN: u32 = 0b001;
pub const FSGNJX: u32 = 0b010;
