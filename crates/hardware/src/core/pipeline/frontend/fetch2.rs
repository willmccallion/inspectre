//! Fetch2 Stage: I-cache access and compressed instruction expansion.
//!
//! This stage reads the instruction bytes from the I-cache (or memory),
//! expands compressed (16-bit) instructions to 32-bit, and produces
//! `IfIdEntry` results for the decode stage.
//!
//! I-cache timing is modeled per cache line:
//!
//! - **Hit:** Instructions are decoded and delivered to `output` (the
//!   fetch2→decode latch) the same cycle. No stall.
//! - **Miss:** `simulate_memory_access` installs the line and returns
//!   the miss penalty. Instructions are decoded into `pending` (a
//!   holding buffer), `stall_out` is set to the penalty, and nothing
//!   is written to `output`. When the stall expires the caller moves
//!   `pending` → `output`. The I-cache is NOT re-accessed on delivery
//!   (the line was already installed on the miss), so there is exactly
//!   one miss stat and zero spurious hit stats per miss event.

// RISC-V instructions may be misaligned (compressed 16-bit instructions); read_unaligned is intentional.
#![allow(clippy::cast_ptr_alignment)]

use crate::common::constants::{COMPRESSED_INSTRUCTION_MASK, COMPRESSED_INSTRUCTION_VALUE};
use crate::common::{AccessType, ExceptionStage, InstSize, Trap, VirtAddr};
use crate::core::Cpu;
use crate::core::pipeline::latches::{Fetch1Fetch2Entry, IfIdEntry};
use crate::isa::rvc::expand::expand;
use crate::{trace_fetch, trace_trap};

/// Executes the Fetch2 stage: I-cache access + RVC expansion.
///
/// Consumes Fetch1→Fetch2 entries and produces Fetch2→Decode entries.
///
/// - On an I-cache **hit**, decoded instructions go into `output`.
/// - On an I-cache **miss**, decoded instructions go into `pending`
///   and `stall_out` is set to the miss penalty. The caller delivers
///   `pending` when the stall expires (without re-probing the cache).
pub fn fetch2_stage(
    cpu: &mut Cpu,
    input: &mut Vec<Fetch1Fetch2Entry>,
    output: &mut Vec<IfIdEntry>,
    pending: &mut Vec<IfIdEntry>,
    stall_out: &mut u64,
) {
    output.clear();
    pending.clear();

    if input.is_empty() {
        return;
    }

    // simulate_memory_access installs lines on miss, so a single probe is enough.
    let mut icache_penalty: u64 = 0;
    if cpu.core.l1_i_cache.enabled {
        let line_mask = !(cpu.core.l1_i_cache.line_bytes() as u64 - 1);
        let mut last_line: u64 = u64::MAX;

        for f1 in input.iter() {
            if f1.trap.is_some() {
                break;
            }
            let this_line = f1.paddr.val() & line_mask;
            if this_line == last_line {
                continue;
            }
            last_line = this_line;

            let penalty = cpu.simulate_memory_access(f1.paddr, AccessType::Fetch);
            icache_penalty += penalty;
        }
    }

    // On miss, decode into pending so the caller delivers it after the stall.
    let dest = if icache_penalty > 0 {
        *stall_out += icache_penalty;
        pending
    } else {
        output
    };

    let entries = std::mem::take(input);

    for f1 in entries {
        if let Some(ref trap) = f1.trap {
            trace_trap!(cpu.config.general.trace_instructions;
                event   = "propagate",
                stage   = "F2",
                pc      = %crate::trace::Hex(f1.pc),
                trap    = ?trap,
                "F2: trap propagated from F1"
            );
            dest.push(IfIdEntry {
                pc: f1.pc,
                inst: 0,
                inst_size: InstSize::Standard,
                pred_taken: f1.pred_taken,
                pred_target: f1.pred_target,
                trap: f1.trap,
                exception_stage: f1.exception_stage,
                ghr_snapshot: f1.ghr_snapshot,
                ras_snapshot: f1.ras_snapshot,
            });
            break;
        }

        let phys_addr = f1.paddr.val();

        let half_word = match cpu.soc.bus.ram_region().filter(|r| r.contains(phys_addr, 2)) {
            // SAFETY: bounds-checked by `RamRegion::contains(phys_addr, 2)` above.
            Some(r) => unsafe { (r.ptr(phys_addr).cast::<u16>()).read_unaligned() },
            None => cpu.soc.bus.read_u16(f1.paddr),
        };

        let is_compressed =
            (half_word & COMPRESSED_INSTRUCTION_MASK) != COMPRESSED_INSTRUCTION_VALUE;

        let (inst, step, inst_trap) = if is_compressed {
            let expanded = expand(half_word);
            if expanded == 0 {
                (0, InstSize::Compressed, Some(Trap::IllegalInstruction(half_word as u32)))
            } else {
                (expanded, InstSize::Compressed, None)
            }
        } else {
            let upper_va = f1.pc.wrapping_add(2);

            // Re-translate upper halfword: a fine-grained PMP boundary can split a 4-byte inst.
            let result = cpu.translate(VirtAddr::new(upper_va), AccessType::Fetch, 2);
            *stall_out += result.cycles;
            let (upper_phys, upper_fault) = (result.paddr, result.trap);

            if let Some(t) = upper_fault {
                (0, InstSize::Standard, Some(t))
            } else {
                let upper_raw = upper_phys.val();
                let upper_half = match cpu.soc.bus.ram_region().filter(|r| r.contains(upper_raw, 2))
                {
                    // SAFETY: bounds-checked by `RamRegion::contains(upper_raw, 2)` above.
                    Some(r) => unsafe { (r.ptr(upper_raw).cast::<u16>()).read_unaligned() },
                    None => cpu.soc.bus.read_u16(upper_phys),
                };

                let full_inst = (upper_half as u32) << 16 | (half_word as u32);
                (full_inst, InstSize::Standard, None)
            }
        };

        if let Some(t) = inst_trap {
            trace_trap!(cpu.config.general.trace_instructions;
                event   = "decode-trap",
                stage   = "F2",
                pc      = %crate::trace::Hex(f1.pc),
                trap    = ?t,
                "F2: instruction decode trap"
            );
            dest.push(IfIdEntry {
                pc: f1.pc,
                inst: 0,
                inst_size: step,
                pred_taken: f1.pred_taken,
                pred_target: f1.pred_target,
                trap: Some(t),
                exception_stage: Some(ExceptionStage::Fetch),
                ghr_snapshot: f1.ghr_snapshot,
                ras_snapshot: f1.ras_snapshot,
            });
            break;
        }

        trace_fetch!(cpu.config.general.trace_instructions;
            pc          = %crate::trace::Hex(f1.pc),
            inst        = inst,
            inst_size   = step.as_u64(),
            compressed  = is_compressed,
            "F2: decoded instruction"
        );

        dest.push(IfIdEntry {
            pc: f1.pc,
            inst,
            inst_size: step,
            pred_taken: f1.pred_taken,
            pred_target: f1.pred_target,
            trap: None,
            exception_stage: None,
            ghr_snapshot: f1.ghr_snapshot,
            ras_snapshot: f1.ras_snapshot,
        });
    }
}
