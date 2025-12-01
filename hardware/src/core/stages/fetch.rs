use crate::core::Cpu;
use crate::core::pipeline::IfId;
use crate::core::types::{AccessType, TranslationResult, Trap, VirtAddr};
use crate::isa::{abi, opcodes};

pub fn fetch_stage(cpu: &mut Cpu) -> Result<(), String> {
    let pc = cpu.pc;

    if pc % 4 != 0 {
        return Err(format!("{:?}", Trap::InstructionAddressMisaligned(pc)));
    }

    let TranslationResult {
        paddr,
        cycles,
        trap,
    } = cpu.translate(VirtAddr::new(pc), AccessType::Fetch);
    cpu.stall_cycles += cycles;

    if let Some(trap_msg) = trap {
        return Err(format!("{:?}", trap_msg));
    }

    let latency = cpu.simulate_memory_access(paddr, AccessType::Fetch);
    cpu.stall_cycles += latency;

    let inst = cpu.bus.bus.read_u32(paddr.val());
    cpu.if_id = IfId { pc, inst };

    if cpu.trace {
        eprintln!("IF  pc={:#x} inst={:#010x}", pc, inst);
    }

    let opcode = inst & 0x7f;
    let rd = ((inst >> 7) & 0x1f) as usize;
    let rs1 = ((inst >> 15) & 0x1f) as usize;
    let mut next_pc = pc.wrapping_add(4);

    if opcode == opcodes::OP_BRANCH {
        let (pred_taken, pred_target) = cpu.branch_predictor.predict_branch(pc);
        if pred_taken {
            if let Some(tgt) = pred_target {
                next_pc = tgt;
            }
        }
    } else if opcode == opcodes::OP_JAL {
        if let Some(tgt) = cpu.branch_predictor.predict_btb(pc) {
            next_pc = tgt;
        }
    } else if opcode == opcodes::OP_JALR {
        if rd == abi::REG_ZERO && rs1 == abi::REG_RA {
            if let Some(tgt) = cpu.branch_predictor.predict_return() {
                next_pc = tgt;
            }
        } else if let Some(tgt) = cpu.branch_predictor.predict_btb(pc) {
            next_pc = tgt;
        }
    }

    cpu.pc = next_pc;
    Ok(())
}
