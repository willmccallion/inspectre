//! Instruction encoding and decoding utilities.
//!
//! Provides bit extraction functions and structures for decoding
//! RISC-V instruction fields from 32-bit instruction encodings.

use crate::common::{CsrAddr, RegIdx};

/// Bit mask for extracting the opcode field (bits 0-6).
pub const OPCODE_MASK: u32 = 0x7F;
/// Bit mask for extracting the destination register field (bits 7-11).
pub const RD_MASK: u32 = 0x1F;
/// Bit mask for extracting the first source register field (bits 15-19).
pub const RS1_MASK: u32 = 0x1F;
/// Bit mask for extracting the second source register field (bits 20-24).
pub const RS2_MASK: u32 = 0x1F;
/// Bit mask for extracting the funct3 field (bits 12-14).
pub const FUNCT3_MASK: u32 = 0x7;
/// Bit mask for extracting the funct7 field (bits 25-31).
pub const FUNCT7_MASK: u32 = 0x7F;
/// Bit mask for extracting the CSR address field (bits 20-31).
pub const CSR_MASK: u32 = 0xFFF;

/// Trait for extracting instruction fields from encoded instructions.
///
/// Provides methods to extract all standard RISC-V instruction fields
/// from a 32-bit instruction encoding.
pub trait InstructionBits {
    /// Extracts the opcode field (bits 0-6).
    ///
    /// The opcode determines the instruction format and operation category.
    /// Returns the 7-bit opcode value.
    fn opcode(&self) -> u32;

    /// Extracts the destination register field (bits 7-11).
    ///
    /// Returns the 5-bit register index (0-31) for the destination register.
    /// Register 0 (x0) is hardwired to zero and writes are ignored.
    fn rd(&self) -> RegIdx;

    /// Extracts the first source register field (bits 15-19).
    ///
    /// Returns the 5-bit register index (0-31) for the first source operand.
    fn rs1(&self) -> RegIdx;

    /// Extracts the second source register field (bits 20-24).
    ///
    /// Returns the 5-bit register index (0-31) for the second source operand.
    fn rs2(&self) -> RegIdx;

    /// Extracts the funct3 field (bits 12-14).
    ///
    /// Used to distinguish between different operations within the same opcode.
    /// Returns the 3-bit funct3 value.
    fn funct3(&self) -> u32;

    /// Extracts the funct7 field (bits 25-31).
    ///
    /// Used for RV64 operations and to distinguish between standard and
    /// alternate encodings (e.g., ADD vs SUB). Returns the 7-bit funct7 value.
    fn funct7(&self) -> u32;

    /// Extracts the CSR address field (bits 20-31).
    ///
    /// Returns the 12-bit CSR address used for CSR read/write operations.
    fn csr(&self) -> CsrAddr;

    /// Extracts the third source register field (bits 27-31, for FMA instructions).
    ///
    /// Returns the 5-bit register index (0-31).
    fn rs3(&self) -> RegIdx;
}

impl InstructionBits for u32 {
    #[inline(always)]
    fn opcode(&self) -> u32 {
        self & OPCODE_MASK
    }

    #[inline(always)]
    fn rd(&self) -> RegIdx {
        RegIdx::new(((self >> 7) & RD_MASK) as u8)
    }

    #[inline(always)]
    fn rs1(&self) -> RegIdx {
        RegIdx::new(((self >> 15) & RS1_MASK) as u8)
    }

    #[inline(always)]
    fn rs2(&self) -> RegIdx {
        RegIdx::new(((self >> 20) & RS2_MASK) as u8)
    }

    #[inline(always)]
    fn rs3(&self) -> RegIdx {
        RegIdx::new(((self >> 27) & RS1_MASK) as u8)
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
    fn csr(&self) -> CsrAddr {
        CsrAddr::from_u32((self >> 20) & CSR_MASK)
    }
}

/// Decoded instruction structure containing all extracted fields.
///
/// Contains all instruction fields extracted during decoding, including
/// opcode, register indices, function codes, and sign-extended immediate.
#[derive(Clone, Debug, Default)]
pub struct Decoded {
    /// Raw 32-bit instruction encoding.
    pub raw: u32,
    /// Extracted opcode field.
    pub opcode: u32,
    /// Destination register index.
    pub rd: RegIdx,
    /// First source register index.
    pub rs1: RegIdx,
    /// Second source register index.
    pub rs2: RegIdx,
    /// Function code field 3.
    pub funct3: u32,
    /// Function code field 7.
    pub funct7: u32,
    /// Sign-extended immediate value.
    pub imm: i64,
}
