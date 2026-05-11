//! Fetch1 Stage: PC generation, branch prediction, I-TLB lookup, fetch
//! request issuance.
//!
//! Each cycle the stage generates up to `pipeline.width` PCs starting at
//! the current architectural PC. For each PC it:
//!
//! 1. Translates the virtual address via `cpu.translate`. On
//!    [`TranslateResult::NeedPte`] it parks an `OutstandingWalk` with a
//!    [`WalkContinuation::Fetch`] and emits a `MemReq` for the first PTE.
//! 2. Reads the instruction half-word from the RAM fast path so the
//!    branch predictor can examine the encoding inline. Cache-line timing
//!    flows through the packet model — only the data bytes use the
//!    pointer.
//! 3. Asks the branch predictor what the next PC is.
//! 4. Emits a `MemReq` with `op = Fetch` to the L1 instruction cache and
//!    parks an [`OutstandingFetch`] under the freshly-allocated `ReqId`.
//!    When the response arrives in the engine's mailbox, the drain stage
//!    pushes the corresponding `Fetch1Fetch2Entry` into the F1→F2 latch.

// RISC-V instructions may be misaligned (compressed 16-bit instructions); read_unaligned is intentional.
#![allow(clippy::cast_ptr_alignment)]

use crate::common::InstSize;
use crate::common::constants::{
    COMPRESSED_INSTRUCTION_MASK, COMPRESSED_INSTRUCTION_VALUE, OPCODE_MASK, RD_MASK, RD_SHIFT,
    RS1_MASK, RS1_SHIFT,
};
use crate::common::{AccessType, ExceptionStage, PhysAddr, RegIdx, Trap, VirtAddr};
use crate::core::Cpu;
use crate::core::arch::csr;
use crate::core::cpu::memory::TranslateResult;
use crate::core::pipeline::engine::ExecutionEngine;
use crate::core::pipeline::outstanding::{OutstandingFetch, OutstandingWalk, WalkContinuation};
use crate::core::units::bru::{BranchPredictor, Ghr};
use crate::isa::abi;
use crate::isa::rv64i::opcodes;
use crate::sim::components::ComponentId;
use crate::sim::packet::{AccessSize, MemOp, Packet};
use crate::trace_branch;
use crate::trace_fetch;

/// Reads a 16-bit instruction half-word from the RAM fast-path pointer.
///
/// Returns 0 for addresses outside DRAM. Architecturally, fetching from
/// MMIO returns garbage; the decoded `0` results in an illegal-instruction
/// trap, which matches what real hardware would do.
fn read_inst_half(cpu: &Cpu, paddr: u64) -> u16 {
    cpu.soc.bus.ram_region().filter(|r| r.contains(paddr, 2)).map_or(0u16, |r| {
        // SAFETY: `RamRegion::contains(paddr, 2)` bounds-checks the access.
        unsafe { r.ptr(paddr).cast::<u16>().read_unaligned() }
    })
}

/// Parks an in-progress page-table walk triggered by an instruction fetch.
fn park_fetch_walk<E: ExecutionEngine>(
    cpu: &mut Cpu,
    engine: &mut E,
    state: crate::core::units::mmu::ptw::WalkState,
    pte_addr: PhysAddr,
    pending: OutstandingFetch,
) {
    let common = engine.common_mut();
    let req_id = common.alloc_req_id();
    let l1_d_id = common.l1_d_id;
    let pipeline_id = common.pipeline_id;
    let _ = common.outstanding_walks.insert(
        req_id,
        OutstandingWalk {
            state,
            pte_addr,
            continuation: WalkContinuation::Fetch(pending),
        },
    );

    let cycle = cpu.soc.cycle;
    cpu.event_queue.schedule(
        cycle,
        ComponentId::Cache(l1_d_id),
        ComponentId::Pipeline(pipeline_id),
        Packet::MemReq {
            req_id,
            paddr: pte_addr,
            vaddr: None,
            size: AccessSize::B8,
            op: MemOp::Read,
        },
    );
}

/// Emits a fetch `MemReq` and parks the corresponding `OutstandingFetch`.
fn issue_fetch<E: ExecutionEngine>(
    cpu: &mut Cpu,
    engine: &mut E,
    fetch: OutstandingFetch,
) {
    let common = engine.common_mut();
    let req_id = common.alloc_req_id();
    let l1_i_id = common.l1_i_id;
    let pipeline_id = common.pipeline_id;
    let paddr = fetch.paddr;
    let pc = fetch.pc;
    let _ = common.outstanding_fetches.insert(req_id, fetch);

    let cycle = cpu.soc.cycle;
    cpu.event_queue.schedule(
        cycle,
        ComponentId::Cache(l1_i_id),
        ComponentId::Pipeline(pipeline_id),
        Packet::MemReq {
            req_id,
            paddr,
            vaddr: Some(VirtAddr::new(pc)),
            size: AccessSize::B4,
            op: MemOp::Fetch,
        },
    );
}


/// Executes the Fetch1 stage: emits up to `pipeline.width` fetch `MemReq`
/// packets, advancing the architectural PC by the predicted next-PC.
pub fn fetch1_stage<E: ExecutionEngine>(cpu: &mut Cpu, engine: &mut E) {
    let mut current_pc = cpu.hart.pc;
    let c_enabled = (cpu.hart.csrs.misa & csr::MISA_EXT_C) != 0;
    let align_mask: u64 = if c_enabled { 1 } else { 3 };

    let line_bytes = cpu.core.l1_i_cache.line_bytes() as u64;
    let line_end = (current_pc | (line_bytes - 1)) + 1;

    for _ in 0..cpu.config.pipeline.width {
        if current_pc + 2 > line_end {
            break;
        }

        let mut fetch_trap = None;
        if (current_pc & align_mask) != 0 {
            fetch_trap = Some(Trap::InstructionAddressMisaligned(current_pc));
        }

        // 1. Translate the PC.
        let translated = if fetch_trap.is_none() {
            cpu.translate(VirtAddr::new(current_pc), AccessType::Fetch, 2)
        } else {
            TranslateResult::Ready(crate::common::TranslationResult::success(
                PhysAddr::new(0),
                0,
            ))
        };

        let (paddr, trap) = match translated {
            TranslateResult::Ready(r) => (r.paddr, r.trap),
            TranslateResult::NeedPte { pte_addr, state } => {
                // Park a fetch with paddr unknown; walk completion will
                // re-issue the actual fetch MemReq.
                let pending = OutstandingFetch {
                    pc: current_pc,
                    paddr: PhysAddr::new(0),
                    pred_taken: false,
                    pred_target: 0,
                    trap: None,
                    exception_stage: None,
                    ghr_snapshot: Ghr::default(),
                    ras_snapshot: 0,
                };
                park_fetch_walk(cpu, engine, state, pte_addr, pending);
                break;
            }
        };

        let trap_cause = fetch_trap.or(trap);
        if let Some(trap_cause) = trap_cause {
            trace_fetch!(cpu.config.general.trace_instructions;
                pc          = %crate::trace::Hex(current_pc),
                trap        = ?trap_cause,
                "F1: fetch trap"
            );
            let fetch = OutstandingFetch {
                pc: current_pc,
                paddr: PhysAddr::new(0),
                pred_taken: false,
                pred_target: 0,
                trap: Some(trap_cause),
                exception_stage: Some(ExceptionStage::Fetch),
                ghr_snapshot: Ghr::default(),
                ras_snapshot: 0,
            };
            issue_fetch(cpu, engine, fetch);
            break;
        }

        let phys_addr = paddr.val();
        let half_word = read_inst_half(cpu, phys_addr);
        let is_compressed =
            (half_word & COMPRESSED_INSTRUCTION_MASK) != COMPRESSED_INSTRUCTION_VALUE;
        let step = if is_compressed { InstSize::Compressed } else { InstSize::Standard };

        let mut next_pc_calc = current_pc.wrapping_add(step.as_u64());
        let mut pred_taken = false;
        let mut pred_target = 0;
        let mut stop_fetch = false;
        let ghr_snapshot = cpu.core.branch_predictor.snapshot_history();
        let ras_snapshot = cpu.core.branch_predictor.snapshot_ras();

        if is_compressed {
            let quadrant = half_word & 0x3;
            let funct3_c = (half_word >> 13) & 0x7;
            if quadrant == 0x01 && (funct3_c == 0b110 || funct3_c == 0b111) {
                let (taken, target) = cpu.core.branch_predictor.predict_branch(current_pc);
                cpu.core.branch_predictor.speculate(current_pc, taken);
                if taken
                    && let Some(tgt) = target
                {
                    next_pc_calc = tgt;
                    pred_taken = true;
                    pred_target = tgt;
                    stop_fetch = true;
                }
                trace_branch!(cpu.config.general.trace_instructions;
                    event       = "predict",
                    pc          = %crate::trace::Hex(current_pc),
                    paddr       = %crate::trace::Hex(phys_addr),
                    bp_type     = "compressed-branch",
                    pred_taken  = taken,
                    pred_target = %crate::trace::Hex(target.unwrap_or(0)),
                    "F1: compressed branch prediction"
                );
            }
        } else {
            let upper_va = current_pc.wrapping_add(2);
            let crosses_page = (current_pc >> 12) != (upper_va >> 12);
            let upper_phys = if crosses_page {
                match cpu.translate(VirtAddr::new(upper_va), AccessType::Fetch, 2) {
                    TranslateResult::Ready(r) => {
                        if r.trap.is_some() {
                            trace_fetch!(cpu.config.general.trace_instructions;
                                pc           = %crate::trace::Hex(current_pc),
                                paddr        = %crate::trace::Hex(phys_addr),
                                crosses_page = true,
                                "F1: page-crossing fault deferred to F2"
                            );
                            // Issue the fetch normally — the upper-half
                            // fault is surfaced when fetch2 re-translates.
                            let fetch = OutstandingFetch {
                                pc: current_pc,
                                paddr: PhysAddr::new(phys_addr),
                                pred_taken: false,
                                pred_target: 0,
                                trap: None,
                                exception_stage: None,
                                ghr_snapshot: Ghr::default(),
                                ras_snapshot,
                            };
                            issue_fetch(cpu, engine, fetch);
                            cpu.hart.pc = next_pc_calc;
                            return;
                        }
                        r.paddr
                    }
                    TranslateResult::NeedPte { pte_addr, state } => {
                        let pending = OutstandingFetch {
                            pc: current_pc,
                            paddr: PhysAddr::new(phys_addr),
                            pred_taken: false,
                            pred_target: 0,
                            trap: None,
                            exception_stage: None,
                            ghr_snapshot,
                            ras_snapshot,
                        };
                        park_fetch_walk(cpu, engine, state, pte_addr, pending);
                        return;
                    }
                }
            } else {
                PhysAddr::new(phys_addr + 2)
            };

            let upper_raw = upper_phys.val();
            let upper_half = read_inst_half(cpu, upper_raw);
            let full_inst = (upper_half as u32) << 16 | (half_word as u32);
            let opcode = full_inst & OPCODE_MASK;
            let rd = RegIdx::new(((full_inst >> RD_SHIFT) & RD_MASK) as u8);
            let rs1 = RegIdx::new(((full_inst >> RS1_SHIFT) & RS1_MASK) as u8);

            if opcode == opcodes::OP_BRANCH {
                let (taken, target) = cpu.core.branch_predictor.predict_branch(current_pc);
                cpu.core.branch_predictor.speculate(current_pc, taken);
                if taken
                    && let Some(tgt) = target
                {
                    next_pc_calc = tgt;
                    pred_taken = true;
                    pred_target = tgt;
                    stop_fetch = true;
                }
                trace_branch!(cpu.config.general.trace_instructions;
                    event       = "predict",
                    pc          = %crate::trace::Hex(current_pc),
                    paddr       = %crate::trace::Hex(phys_addr),
                    inst        = %crate::trace::Hex32(full_inst),
                    bp_type     = "branch",
                    pred_taken  = taken,
                    pred_target = %crate::trace::Hex(target.unwrap_or(0)),
                    "F1: branch prediction"
                );
            } else if opcode == opcodes::OP_JAL {
                if let Some(tgt) = cpu.core.branch_predictor.predict_btb(current_pc) {
                    next_pc_calc = tgt;
                    pred_taken = true;
                    pred_target = tgt;
                    stop_fetch = true;
                }
                trace_branch!(cpu.config.general.trace_instructions;
                    event       = "predict",
                    pc          = %crate::trace::Hex(current_pc),
                    paddr       = %crate::trace::Hex(phys_addr),
                    inst        = %crate::trace::Hex32(full_inst),
                    bp_type     = "JAL/BTB",
                    pred_taken  = pred_taken,
                    pred_target = %crate::trace::Hex(pred_target),
                    "F1: JAL prediction"
                );
            } else if opcode == opcodes::OP_JALR {
                let rd_link = rd == abi::REG_RA || rd == abi::REG_T0;
                let rs1_link = rs1 == abi::REG_RA || rs1 == abi::REG_T0;
                let use_ras = rs1_link && (!rd_link || rd != rs1);
                if use_ras {
                    if let Some(tgt) = cpu.core.branch_predictor.predict_return() {
                        next_pc_calc = tgt;
                        pred_taken = true;
                        pred_target = tgt;
                    }
                } else if let Some(tgt) = cpu.core.branch_predictor.predict_btb(current_pc) {
                    next_pc_calc = tgt;
                    pred_taken = true;
                    pred_target = tgt;
                }
                stop_fetch = true;
                trace_branch!(cpu.config.general.trace_instructions;
                    event       = "predict",
                    pc          = %crate::trace::Hex(current_pc),
                    paddr       = %crate::trace::Hex(phys_addr),
                    inst        = %crate::trace::Hex32(full_inst),
                    bp_type     = if use_ras { "JALR/RAS" } else { "JALR/BTB" },
                    pred_taken  = pred_taken,
                    pred_target = %crate::trace::Hex(pred_target),
                    "F1: JALR prediction"
                );
            }
        }

        trace_fetch!(cpu.config.general.trace_instructions;
            pc          = %crate::trace::Hex(current_pc),
            paddr       = %crate::trace::Hex(phys_addr),
            compressed  = is_compressed,
            pred_taken,
            pred_target = %crate::trace::Hex(pred_target),
            "F1: fetch entry issued"
        );

        let fetch = OutstandingFetch {
            pc: current_pc,
            paddr: PhysAddr::new(phys_addr),
            pred_taken,
            pred_target,
            trap: None,
            exception_stage: None,
            ghr_snapshot,
            ras_snapshot,
        };
        issue_fetch(cpu, engine, fetch);

        current_pc = next_pc_calc;
        if stop_fetch {
            break;
        }
    }

    cpu.hart.pc = current_pc;
}
