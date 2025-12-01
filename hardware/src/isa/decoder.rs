use super::instruction::{Decoded, InstructionBits};
use super::opcodes;

pub fn decode(inst: u32) -> Decoded {
    let opcode = inst.opcode();
    let imm = match opcode {
        opcodes::OP_IMM
        | opcodes::OP_LOAD
        | opcodes::OP_JALR
        | opcodes::OP_IMM_32
        | opcodes::OP_LOAD_FP => ((inst as i32) >> 20) as i64,

        opcodes::OP_STORE | opcodes::OP_STORE_FP => {
            let low = (inst >> 7) & 0x1F;
            let high = (inst >> 25) & 0x7F;
            let combined = (high << 5) | low;
            ((combined as i32) << 20 >> 20) as i64
        }

        opcodes::OP_BRANCH => {
            let bit_11 = (inst >> 7) & 1;
            let bits_4_1 = (inst >> 8) & 0xF;
            let bits_10_5 = (inst >> 25) & 0x3F;
            let bit_12 = (inst >> 31) & 1;
            let combined = (bit_12 << 12) | (bit_11 << 11) | (bits_10_5 << 5) | (bits_4_1 << 1);
            ((combined as i32) << 19 >> 19) as i64
        }

        opcodes::OP_LUI | opcodes::OP_AUIPC => ((inst & 0xFFFFF000) as i32) as i64,

        opcodes::OP_JAL => {
            let bits_19_12 = (inst >> 12) & 0xFF;
            let bit_11 = (inst >> 20) & 1;
            let bits_10_1 = (inst >> 21) & 0x3FF;
            let bit_20 = (inst >> 31) & 1;
            let combined = (bit_20 << 20) | (bits_19_12 << 12) | (bit_11 << 11) | (bits_10_1 << 1);
            ((combined as i32) << 11 >> 11) as i64
        }

        _ => 0,
    };

    Decoded {
        raw: inst,
        opcode,
        rd: inst.rd(),
        rs1: inst.rs1(),
        rs2: inst.rs2(),
        funct3: inst.funct3(),
        funct7: inst.funct7(),
        imm,
    }
}
