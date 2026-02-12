//! Instruction Fetch (IF) Stage.
//!
//! This module implements the first stage of the instruction pipeline.
//! It is responsible for fetching instructions from memory using the
//! current Program Counter (PC), handling compressed instruction expansion,
//! and performing branch prediction to determine the next PC.

use crate::common::constants::{
    COMPRESSED_INSTRUCTION_MASK, COMPRESSED_INSTRUCTION_VALUE, INSTRUCTION_SIZE_16,
    INSTRUCTION_SIZE_32, OPCODE_MASK, RD_MASK, RD_SHIFT, RS1_MASK, RS1_SHIFT,
};
use crate::common::{AccessType, ExceptionStage, TranslationResult, Trap, VirtAddr};
use crate::core::Cpu;
use crate::core::pipeline::latches::IfIdEntry;
use crate::core::units::bru::BranchPredictor;
use crate::isa::abi;
use crate::isa::rv64i::opcodes;
use crate::isa::rvc::expand::expand;

/// Executes the instruction fetch stage of the pipeline.
///
/// Fetches instructions from memory starting at the current program counter.
/// Handles compressed instruction expansion, branch prediction, address
/// translation, and instruction alignment checks. Updates the IF/ID pipeline
/// latch with fetched instructions.
///
/// # Arguments
///
/// * `cpu` - Mutable reference to the CPU state
///
/// # Behavior
///
/// - Fetches up to `pipeline_width` instructions per cycle
/// - Expands compressed (16-bit) instructions to 32-bit format
/// - Performs branch prediction for control flow instructions
/// - Stops fetching on misaligned addresses or translation faults
/// - Updates the program counter based on predictions
pub fn fetch_stage(cpu: &mut Cpu) {
    let mut fetched = std::mem::take(&mut cpu.if_id_shadow);
    fetched.clear();

    let mut current_pc = cpu.pc;

    for _ in 0..cpu.pipeline_width {
        let mut fetch_trap = None;
        if (current_pc & 1) != 0 {
            if fetched.is_empty() {
                fetch_trap = Some(Trap::InstructionAddressMisaligned(current_pc));
            } else {
                break;
            }
        }

        let TranslationResult {
            paddr,
            cycles,
            trap,
        } = if fetch_trap.is_none() {
            cpu.translate(VirtAddr::new(current_pc), AccessType::Fetch)
        } else {
            TranslationResult {
                paddr: crate::common::PhysAddr::new(0),
                cycles: 0,
                trap: None,
            }
        };
        cpu.stall_cycles += cycles;

        let trap_cause = fetch_trap.or(trap);
        if let Some(ref trap_cause) = trap_cause {
            if fetched.is_empty() {
                if cpu.trace {
                    eprintln!("IF  pc={:#x} # TRAP: {:?}", current_pc, trap_cause);
                }
                fetched.push(IfIdEntry {
                    pc: current_pc,
                    inst: 0,
                    inst_size: 4,
                    pred_taken: false,
                    pred_target: 0,
                    trap: Some(trap_cause.clone()),
                    exception_stage: Some(ExceptionStage::Fetch),
                });
                break;
            } else {
                break;
            }
        }

        let phys_addr = paddr.val();

        let half_word = if phys_addr >= cpu.ram_start && phys_addr < cpu.ram_end {
            let offset = (phys_addr - cpu.ram_start) as usize;
            // SAFETY: This is safe because:
            // 1. `phys_addr` is validated to be within RAM bounds (>= ram_start && < ram_end)
            // 2. `offset` is computed from validated bounds, ensuring it's within allocated memory
            // 3. `ram_ptr` points to valid, initialized memory allocated during CPU construction
            // 4. `read_unaligned()` handles any alignment issues that may occur at arbitrary addresses
            // 5. The u16 read cannot overflow the buffer as offset is strictly < (ram_end - ram_start)
            unsafe {
                let ptr = cpu.ram_ptr.add(offset) as *const u16;
                ptr.read_unaligned()
            }
        } else {
            cpu.bus.bus.read_u16(phys_addr)
        };

        let is_compressed =
            (half_word & COMPRESSED_INSTRUCTION_MASK) != COMPRESSED_INSTRUCTION_VALUE;

        let (inst, step, inst_trap) = if is_compressed {
            let expanded = expand(half_word);
            if expanded == 0 {
                if fetched.is_empty() {
                    (
                        0,
                        INSTRUCTION_SIZE_16,
                        Some(Trap::IllegalInstruction(half_word as u32)),
                    )
                } else {
                    break;
                }
            } else {
                (expanded, INSTRUCTION_SIZE_16, None)
            }
        } else {
            /// Byte offset to fetch upper half-word of 32-bit instruction.
            const UPPER_HALF_OFFSET: u64 = 2;

            /// Bit shift to combine upper and lower half-words into full instruction.
            const UPPER_HALF_SHIFT: u32 = 16;

            // When a 32-bit instruction spans a page boundary (PC at offset
            // 0xFFE), the upper half-word lives on a different virtual page
            // which may map to a non-contiguous physical address.  We must
            // translate the upper half's VA independently in that case.
            let upper_va = current_pc.wrapping_add(UPPER_HALF_OFFSET);
            let crosses_page = (current_pc >> 12) != (upper_va >> 12);

            let (upper_phys, upper_fault) = if crosses_page {
                let result = cpu.translate(VirtAddr::new(upper_va), AccessType::Fetch);
                cpu.stall_cycles += result.cycles;
                (result.paddr.val(), result.trap)
            } else {
                (phys_addr + UPPER_HALF_OFFSET, None)
            };

            if let Some(t) = upper_fault {
                (0, INSTRUCTION_SIZE_32, Some(t))
            } else {
                let upper_half = if upper_phys >= cpu.ram_start && upper_phys < cpu.ram_end {
                    let offset = (upper_phys - cpu.ram_start) as usize;
                    // SAFETY: upper_phys is validated to be within RAM bounds.
                    unsafe {
                        let ptr = cpu.ram_ptr.add(offset) as *const u16;
                        ptr.read_unaligned()
                    }
                } else {
                    cpu.bus.bus.read_u16(upper_phys)
                };

                let full_inst = (upper_half as u32) << UPPER_HALF_SHIFT | (half_word as u32);
                (full_inst, INSTRUCTION_SIZE_32, None)
            }
        };

        if let Some(t) = inst_trap {
            if cpu.trace {
                eprintln!("IF  pc={:#x} # TRAP: {:?}", current_pc, t);
            }
            fetched.push(IfIdEntry {
                pc: current_pc,
                inst: 0,
                inst_size: step,
                pred_taken: false,
                pred_target: 0,
                trap: Some(t),
                exception_stage: Some(ExceptionStage::Fetch),
            });
            break;
        }

        if phys_addr >= cpu.mmio_base {
            cpu.stall_cycles += cpu.simulate_memory_access(paddr, AccessType::Fetch);
        } else {
            cpu.stall_cycles += cpu.bus.bus.calculate_transit_time(4);
        }

        if cpu.trace {
            eprintln!("IF  pc={:#x} inst={:#010x} (sz={})", current_pc, inst, step);
        }

        let opcode = inst & OPCODE_MASK;
        let rd = ((inst >> RD_SHIFT) & RD_MASK) as usize;
        let rs1 = ((inst >> RS1_SHIFT) & RS1_MASK) as usize;
        let mut next_pc_calc = current_pc.wrapping_add(step);
        let mut pred_taken = false;
        let mut pred_target = 0;
        let mut stop_fetch = false;

        if opcode == opcodes::OP_BRANCH {
            let (taken, target) = cpu.branch_predictor.predict_branch(current_pc);
            if taken && let Some(tgt) = target {
                next_pc_calc = tgt;
                pred_taken = true;
                pred_target = tgt;
                stop_fetch = true;
            }
        } else if opcode == opcodes::OP_JAL {
            if let Some(tgt) = cpu.branch_predictor.predict_btb(current_pc) {
                next_pc_calc = tgt;
                pred_taken = true;
                pred_target = tgt;
                stop_fetch = true;
            }
        } else if opcode == opcodes::OP_JALR {
            if rd == abi::REG_ZERO && rs1 == abi::REG_RA {
                if let Some(tgt) = cpu.branch_predictor.predict_return() {
                    next_pc_calc = tgt;
                    pred_taken = true;
                    pred_target = tgt;
                }
            } else if let Some(tgt) = cpu.branch_predictor.predict_btb(current_pc) {
                next_pc_calc = tgt;
                pred_taken = true;
                pred_target = tgt;
            }
            stop_fetch = true;
        }

        fetched.push(IfIdEntry {
            pc: current_pc,
            inst,
            inst_size: step,
            pred_taken,
            pred_target,
            trap: None,
            exception_stage: None,
        });

        current_pc = next_pc_calc;

        if stop_fetch {
            break;
        }
    }

    cpu.pc = current_pc;
    cpu.if_id.entries = fetched;
}
