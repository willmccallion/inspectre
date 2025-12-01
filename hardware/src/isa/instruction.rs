pub const OPCODE_MASK: u32 = 0x7F;
pub const RD_MASK: u32 = 0x1F;
pub const RS1_MASK: u32 = 0x1F;
pub const RS2_MASK: u32 = 0x1F;
pub const FUNCT3_MASK: u32 = 0x7;
pub const FUNCT7_MASK: u32 = 0x7F;
pub const CSR_MASK: u32 = 0xFFF;

pub trait InstructionBits {
    fn opcode(&self) -> u32;
    fn rd(&self) -> usize;
    fn rs1(&self) -> usize;
    fn rs2(&self) -> usize;
    fn funct3(&self) -> u32;
    fn funct7(&self) -> u32;
    fn csr(&self) -> u32;
    fn rs3(&self) -> usize;
}

impl InstructionBits for u32 {
    #[inline(always)]
    fn opcode(&self) -> u32 {
        self & OPCODE_MASK
    }

    #[inline(always)]
    fn rd(&self) -> usize {
        ((self >> 7) & RD_MASK) as usize
    }

    #[inline(always)]
    fn rs1(&self) -> usize {
        ((self >> 15) & RS1_MASK) as usize
    }

    #[inline(always)]
    fn rs2(&self) -> usize {
        ((self >> 20) & RS2_MASK) as usize
    }

    #[inline(always)]
    fn rs3(&self) -> usize {
        ((self >> 27) & RS1_MASK) as usize
    }

    #[inline(always)]
    fn funct3(&self) -> u32 {
        (self >> 12) & FUNCT3_MASK
    }

    #[inline(always)]
    fn funct7(&self) -> u32 {
        (self >> 25) & FUNCT7_MASK
    }

    #[inline(always)]
    fn csr(&self) -> u32 {
        (self >> 20) & CSR_MASK
    }
}

#[derive(Clone, Debug, Default)]
pub struct Decoded {
    pub raw: u32,
    pub opcode: u32,
    pub rd: usize,
    pub rs1: usize,
    pub rs2: usize,
    pub funct3: u32,
    pub funct7: u32,
    pub imm: i64,
}
