//! Memory2 Stage: D-cache access for loads, store buffer resolution.
//!
//! For loads: read data from the cache/memory (with store-to-load forwarding).
//! For stores: resolve the store buffer entry with paddr + data (NO memory write).
//! This stage is the same for both in-order and O3 backends.

use crate::common::error::{ExceptionStage, LrScRecord, Trap};
use crate::core::Cpu;
use crate::core::pipeline::latches::{Mem1Mem2Entry, Mem2WbEntry};
use crate::core::pipeline::load_queue::LoadQueue;
use crate::core::pipeline::rob::RobTag;
use crate::core::pipeline::signals::{AtomicOp, MemWidth};
use crate::core::pipeline::store_buffer::{ForwardResult, StoreBuffer};
use crate::core::units::lsu::Lsu;
use crate::trace_fwd;
use crate::trace_mem;
use crate::trace_trap;

/// Executes the Memory2 stage: D-cache access + store buffer forwarding.
///
/// Returns `Some(violating_rob_tag)` if a memory ordering violation is detected
/// (a store resolved its address and overlapped with a younger already-executed load).
/// The caller should flush from this tag onward.
pub fn memory2_stage(
    cpu: &mut Cpu,
    input: &mut Vec<Mem1Mem2Entry>,
    output: &mut Vec<Mem2WbEntry>,
    store_buffer: &mut StoreBuffer,
    mut load_queue: Option<&mut LoadQueue>,
    mut vec_store_buffer: Option<&mut crate::core::pipeline::vec_store_buffer::VecStoreBuffer>,
) -> Option<(RobTag, u64)> {
    let mut violation: Option<(RobTag, u64)> = None;
    let mut entries = std::mem::take(input);

    // Process in program order so younger loads don't stall behind unresolved older stores.
    entries.sort_by_key(|e| e.rob_tag.0);

    output.clear();

    let mut iter = entries.into_iter();

    while let Some(mem) = iter.next() {
        if let Some(ref trap) = mem.trap {
            trace_trap!(cpu.config.general.trace_instructions;
                event   = "propagate",
                stage   = "M2",
                pc      = %crate::trace::Hex(mem.pc),
                rob_tag = mem.rob_tag.0,
                trap    = ?trap,
                "M2: trap propagated through memory2"
            );
            output.push(Mem2WbEntry {
                rob_tag: mem.rob_tag,
                pc: mem.pc,
                inst: mem.inst,
                inst_size: mem.inst_size,
                rd: mem.rd,
                alu: mem.alu,
                load_data: 0,
                ctrl: mem.ctrl,
                trap: mem.trap,
                exception_stage: mem.exception_stage,
                rd_phys: mem.rd_phys,
                fp_flags: mem.fp_flags,
                pte_update: mem.pte_update,
                sfence_vma: mem.sfence_vma,
                lr_sc: None,
                vec_mem: mem.vec_mem,
            });
            // Remaining entries stay in input — commit's trap handler flushes them.
            input.extend(iter);
            return violation;
        }

        let raw_paddr = mem.paddr;
        let ram_region = cpu.soc.bus.ram_region().filter(|r| r.contains(raw_paddr.val(), 1));
        let is_ram = ram_region.is_some();

        let mut ld: u64 = 0;
        let trap: Option<Trap> = None;
        let exception_stage: Option<ExceptionStage> = None;
        let mut lr_sc: Option<LrScRecord> = None;

        if mem.ctrl.atomic_op != AtomicOp::None {
            match mem.ctrl.atomic_op {
                AtomicOp::Lr => {
                    // LR reads the globally-visible value: stall on older stores to this addr.
                    if store_buffer.has_older_store_to(raw_paddr, mem.ctrl.width, mem.rob_tag) {
                        input.push(mem);
                        input.extend(iter);
                        return violation;
                    }
                    ld = match mem.ctrl.width {
                        MemWidth::Word => (cpu.soc.bus.read_u32(raw_paddr) as i32) as i64 as u64,
                        MemWidth::Double => cpu.soc.bus.read_u64(raw_paddr),
                        _ => 0,
                    };
                    // Defer reservation to commit; speculative LR mustn't touch arch state.
                    lr_sc = Some(LrScRecord::Lr { paddr: raw_paddr });
                }
                AtomicOp::Sc => {
                    // Assume SC succeeds; commit verifies the reservation and cancels on miss.
                    store_buffer.resolve(mem.rob_tag, mem.vaddr, raw_paddr, mem.store_data);
                    ld = 0;
                    lr_sc = Some(LrScRecord::Sc { paddr: raw_paddr });

                    if let Some(ref lq) = load_queue
                        && let Some(violating_tag) =
                            lq.check_ordering_violation(raw_paddr, mem.ctrl.width, mem.rob_tag)
                    {
                        match violation {
                            None => violation = Some((violating_tag, mem.pc)),
                            Some((prev, _)) if violating_tag.is_older_than(prev) => {
                                violation = Some((violating_tag, mem.pc));
                            }
                            _ => {}
                        }
                    }
                }
                _ => {
                    // AMO RMW operates on globally-visible value: stall on older stores here.
                    if store_buffer.has_older_store_to(raw_paddr, mem.ctrl.width, mem.rob_tag) {
                        input.push(mem);
                        input.extend(iter);
                        return violation;
                    }
                    let old_val = match mem.ctrl.width {
                        MemWidth::Word => (cpu.soc.bus.read_u32(raw_paddr) as i32) as i64 as u64,
                        MemWidth::Double => cpu.soc.bus.read_u64(raw_paddr),
                        _ => 0,
                    };

                    let new_val = Lsu::atomic_alu(
                        mem.ctrl.atomic_op,
                        old_val,
                        mem.store_data,
                        mem.ctrl.width,
                    );

                    store_buffer.resolve(mem.rob_tag, mem.vaddr, raw_paddr, new_val);

                    if let Some(ref lq) = load_queue
                        && let Some(violating_tag) =
                            lq.check_ordering_violation(raw_paddr, mem.ctrl.width, mem.rob_tag)
                    {
                        match violation {
                            None => violation = Some((violating_tag, mem.pc)),
                            Some((prev, _)) if violating_tag.is_older_than(prev) => {
                                violation = Some((violating_tag, mem.pc));
                            }
                            _ => {}
                        }
                    }

                    ld = old_val;
                    // AMO reservation clear is deferred to the next SC commit (avoid squash corruption).
                }
            }
        } else if mem.ctrl.mem_read {
            // Check scalar SB first, then VSB for older vec stores still in flight.
            let sb_result = store_buffer.forward_load(raw_paddr, mem.ctrl.width, mem.rob_tag);
            let fwd = match sb_result {
                ForwardResult::Miss => match vec_store_buffer.as_deref() {
                    Some(vsb) => vsb.forward_load(raw_paddr, mem.ctrl.width, mem.rob_tag),
                    None => ForwardResult::Miss,
                },
                other => other,
            };
            match fwd {
                ForwardResult::Hit(forwarded) => {
                    // SB returns raw masked data; apply sign extension for signed loads.
                    ld = if mem.ctrl.signed_load {
                        match mem.ctrl.width {
                            MemWidth::Byte => (forwarded as u8 as i8) as i64 as u64,
                            MemWidth::Half => (forwarded as u16 as i16) as i64 as u64,
                            MemWidth::Word => (forwarded as u32 as i32) as i64 as u64,
                            _ => forwarded,
                        }
                    } else {
                        forwarded
                    };
                    if mem.ctrl.fp_reg_write {
                        match mem.ctrl.width {
                            MemWidth::Word => ld |= 0xFFFF_FFFF_0000_0000,
                            MemWidth::Half => {
                                // Zfh flh: NaN-box upper 48 bits.
                                ld = (ld & 0xFFFF) | 0xFFFF_FFFF_FFFF_0000;
                            }
                            _ => {}
                        }
                    }

                    trace_fwd!(cpu.config.general.trace_instructions;
                        event           = "forward",
                        load_pc         = %crate::trace::Hex(mem.pc),
                        load_tag        = mem.rob_tag.0,
                        paddr           = %crate::trace::Hex(raw_paddr.val()),
                        width           = ?mem.ctrl.width,
                        signed          = mem.ctrl.signed_load,
                        forwarded_val   = %crate::trace::Hex(ld),
                        "M2: store-to-load forwarding HIT"
                    );
                    trace_mem!(cpu.config.general.trace_instructions;
                        stage       = "M2",
                        rob_tag     = mem.rob_tag.0,
                        pc          = %crate::trace::Hex(mem.pc),
                        op          = "load",
                        paddr       = %crate::trace::Hex(raw_paddr.val()),
                        width       = ?mem.ctrl.width,
                        forwarded   = true,
                        load_data   = %crate::trace::Hex(ld),
                        "M2: load satisfied from store buffer"
                    );
                }
                ForwardResult::Stall => {
                    trace_fwd!(cpu.config.general.trace_instructions;
                        event           = "stall",
                        load_pc         = %crate::trace::Hex(mem.pc),
                        load_tag        = mem.rob_tag.0,
                        paddr           = %crate::trace::Hex(raw_paddr.val()),
                        width           = ?mem.ctrl.width,
                        partial_overlap = true,
                        "M2: store-to-load forwarding STALL (partial overlap)"
                    );
                    input.push(mem);
                    input.extend(iter);
                    return violation;
                }
                ForwardResult::Miss => {
                    trace_fwd!(cpu.config.general.trace_instructions;
                        event   = "miss",
                        load_pc = %crate::trace::Hex(mem.pc),
                        load_tag = mem.rob_tag.0,
                        paddr   = %crate::trace::Hex(raw_paddr.val()),
                        width   = ?mem.ctrl.width,
                        is_ram,
                        "M2: store buffer miss — reading from memory"
                    );
                    ld = if let Some(r) = ram_region {
                        // SAFETY: `ram_region` was derived from `RamRegion::contains`,
                        // confirming the address sits inside DRAM; widths up to 8 bytes
                        // also fit because `Soc` only registers contiguous DRAM regions.
                        unsafe {
                            let ptr = r.ptr(raw_paddr.val());
                            match (mem.ctrl.width, mem.ctrl.signed_load) {
                                (MemWidth::Byte, true) => (*ptr as i8) as i64 as u64,
                                (MemWidth::Half, true) => {
                                    (ptr.cast::<u16>().read_unaligned() as i16) as i64 as u64
                                }
                                (MemWidth::Word, true) => {
                                    (ptr.cast::<u32>().read_unaligned() as i32) as i64 as u64
                                }
                                (MemWidth::Byte, false) => *ptr as u64,
                                (MemWidth::Half, false) => {
                                    ptr.cast::<u16>().read_unaligned() as u64
                                }
                                (MemWidth::Word, false) => {
                                    ptr.cast::<u32>().read_unaligned() as u64
                                }
                                (MemWidth::Double, _) => ptr.cast::<u64>().read_unaligned(),
                                _ => 0,
                            }
                        }
                    } else {
                        match (mem.ctrl.width, mem.ctrl.signed_load) {
                            (MemWidth::Byte, true) => {
                                (cpu.soc.bus.read_u8(raw_paddr) as i8) as i64 as u64
                            }
                            (MemWidth::Half, true) => {
                                (cpu.soc.bus.read_u16(raw_paddr) as i16) as i64 as u64
                            }
                            (MemWidth::Word, true) => {
                                (cpu.soc.bus.read_u32(raw_paddr) as i32) as i64 as u64
                            }
                            (MemWidth::Byte, false) => cpu.soc.bus.read_u8(raw_paddr) as u64,
                            (MemWidth::Half, false) => cpu.soc.bus.read_u16(raw_paddr) as u64,
                            (MemWidth::Word, false) => cpu.soc.bus.read_u32(raw_paddr) as u64,
                            (MemWidth::Double, _) => cpu.soc.bus.read_u64(raw_paddr),
                            _ => 0,
                        }
                    };

                    if mem.ctrl.fp_reg_write {
                        match mem.ctrl.width {
                            MemWidth::Word => ld |= 0xFFFF_FFFF_0000_0000,
                            MemWidth::Half => {
                                // Zfh flh: NaN-box upper 48 bits.
                                ld = (ld & 0xFFFF) | 0xFFFF_FFFF_FFFF_0000;
                            }
                            _ => {}
                        }
                    }
                }
            }

            if let Some(ref mut lq) = load_queue {
                let lq_elem = mem.vec_mem.as_ref().map(|vme| vme.elem_idx);
                lq.fill_data(mem.rob_tag, lq_elem, ld);
            }

            trace_mem!(cpu.config.general.trace_instructions;
                stage     = "M2",
                rob_tag   = mem.rob_tag.0,
                pc        = %crate::trace::Hex(mem.pc),
                op        = "load",
                paddr     = %crate::trace::Hex(raw_paddr.val()),
                width     = ?mem.ctrl.width,
                forwarded = false,
                load_data = %crate::trace::Hex(ld),
                is_ram,
                "M2: load complete"
            );
        } else if mem.ctrl.mem_write {
            // Vector store elements go to VecStoreBuffer; scalar stores resolve their SB slot.
            if mem.vec_mem.as_ref().is_some_and(|vme| vme.is_store) {
                if let Some(vsb) = vec_store_buffer.as_deref_mut() {
                    vsb.resolve_element(mem.rob_tag, raw_paddr, mem.store_data, mem.ctrl.width);
                }
            } else {
                store_buffer.resolve(mem.rob_tag, mem.vaddr, raw_paddr, mem.store_data);
            }

            if let Some(ref lq) = load_queue
                && let Some(violating_tag) =
                    lq.check_ordering_violation(raw_paddr, mem.ctrl.width, mem.rob_tag)
            {
                trace_fwd!(cpu.config.general.trace_instructions;
                    event             = "violation",
                    store_pc          = %crate::trace::Hex(mem.pc),
                    store_tag         = mem.rob_tag.0,
                    paddr             = %crate::trace::Hex(raw_paddr.val()),
                    width             = ?mem.ctrl.width,
                    violation_flush   = violating_tag.0,
                    "M2: memory ordering VIOLATION — younger load executed with stale data"
                );
                match violation {
                    None => violation = Some((violating_tag, mem.pc)),
                    Some((prev, _)) if violating_tag.is_older_than(prev) => {
                        violation = Some((violating_tag, mem.pc));
                    }
                    _ => {}
                }
            }

            trace_mem!(cpu.config.general.trace_instructions;
                stage      = "M2",
                rob_tag    = mem.rob_tag.0,
                pc         = %crate::trace::Hex(mem.pc),
                op         = "store-resolve",
                paddr      = %crate::trace::Hex(raw_paddr.val()),
                vaddr      = %crate::trace::Hex(mem.vaddr.val()),
                width      = ?mem.ctrl.width,
                store_data = %crate::trace::Hex(mem.store_data),
                is_ram,
                "M2: store resolved into store buffer (write deferred to commit)"
            );
        } else {
            trace_mem!(cpu.config.general.trace_instructions;
                stage   = "M2",
                rob_tag = mem.rob_tag.0,
                pc      = %crate::trace::Hex(mem.pc),
                op      = "passthrough",
                "M2: non-memory instruction pass-through"
            );
        }

        output.push(Mem2WbEntry {
            rob_tag: mem.rob_tag,
            pc: mem.pc,
            inst: mem.inst,
            inst_size: mem.inst_size,
            rd: mem.rd,
            rd_phys: mem.rd_phys,
            alu: mem.alu,
            load_data: ld,
            ctrl: mem.ctrl,
            trap: trap.clone(),
            exception_stage,
            fp_flags: mem.fp_flags,
            pte_update: mem.pte_update,
            sfence_vma: mem.sfence_vma,
            lr_sc,
            vec_mem: mem.vec_mem,
        });

        if trap.is_some() {
            input.extend(iter);
            return violation;
        }
    }

    violation
}

#[cfg(test)]
#[allow(unused_results)]
mod tests {
    use super::*;
    use crate::common::{InstSize, PhysAddr, RegIdx, VirtAddr};
    use crate::config::Config;
    use crate::core::pipeline::signals::ControlSignals;
    use crate::core::pipeline::store_buffer::StoreBuffer;
    use crate::soc::builder::Soc;

    #[test]
    fn test_memory2_pass_through() {
        let config = Config::default();
        let mut cpu = Cpu::build(&config, "");
        let mut store_buffer = StoreBuffer::new(4);

        let mut input = vec![Mem1Mem2Entry {
            rob_tag: RobTag(1),
            pc: 0x1000,
            inst: 0,
            inst_size: InstSize::Standard,
            rd: RegIdx::new(1),
            rd_phys: crate::core::pipeline::prf::PhysReg(0),
            alu: 42,
            vaddr: VirtAddr::new(0),
            paddr: PhysAddr::new(0),
            store_data: 0,
            ctrl: ControlSignals::default(),
            trap: None,
            exception_stage: None,
            fp_flags: 0,
            complete_cycle: 10,
            pte_update: None,
            sfence_vma: None,
            vec_mem: None,
        }];
        let mut output = Vec::new();

        let violation =
            memory2_stage(&mut cpu, &mut input, &mut output, &mut store_buffer, None, None);

        assert!(violation.is_none());
        assert_eq!(input.len(), 0);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].load_data, 0);
    }

    #[test]
    fn test_memory2_trap_propagation() {
        let config = Config::default();
        let mut cpu = Cpu::build(&config, "");
        let mut store_buffer = StoreBuffer::new(4);

        let mut input = vec![Mem1Mem2Entry {
            rob_tag: RobTag(1),
            pc: 0x1000,
            inst: 0,
            inst_size: InstSize::Standard,
            rd: RegIdx::new(1),
            rd_phys: crate::core::pipeline::prf::PhysReg(0),
            alu: 0,
            vaddr: VirtAddr::new(0),
            paddr: PhysAddr::new(0),
            store_data: 0,
            ctrl: ControlSignals::default(),
            trap: Some(crate::common::Trap::IllegalInstruction(0)),
            exception_stage: Some(ExceptionStage::Execute),
            fp_flags: 0,
            complete_cycle: 10,
            pte_update: None,
            sfence_vma: None,
            vec_mem: None,
        }];
        let mut output = Vec::new();

        let violation =
            memory2_stage(&mut cpu, &mut input, &mut output, &mut store_buffer, None, None);

        assert!(violation.is_none());
        assert_eq!(input.len(), 0); // Input is drained because trap is pushed
        assert_eq!(output.len(), 1);
        assert!(output[0].trap.is_some());
    }

    #[test]
    fn test_memory2_atomic_lr_sc_deferred() {
        use crate::common::error::LrScRecord;

        let config = Config::default();
        let mut cpu = Cpu::build(&config, "");
        let mut store_buffer = StoreBuffer::new(4);

        let ctrl_lr = ControlSignals {
            atomic_op: crate::core::pipeline::signals::AtomicOp::Lr,
            width: crate::core::pipeline::signals::MemWidth::Word,
            ..Default::default()
        };

        let mut input_lr = vec![Mem1Mem2Entry {
            rob_tag: RobTag(1),
            pc: 0x1000,
            inst: 0,
            inst_size: InstSize::Standard,
            rd: RegIdx::new(1),
            rd_phys: crate::core::pipeline::prf::PhysReg(0),
            alu: 0,
            vaddr: VirtAddr::new(0x8000_0000),
            paddr: PhysAddr::new(0x8000_0000),
            store_data: 0,
            ctrl: ctrl_lr,
            trap: None,
            exception_stage: None,
            fp_flags: 0,
            complete_cycle: 10,
            pte_update: None,
            sfence_vma: None,
            vec_mem: None,
        }];
        let mut output = Vec::new();

        memory2_stage(&mut cpu, &mut input_lr, &mut output, &mut store_buffer, None, None);
        // LR does NOT set reservation at Memory2 — deferred to commit
        assert!(!cpu.check_reservation(PhysAddr::new(0x8000_0000)));
        // But the output carries the deferred LR record
        assert!(matches!(output[0].lr_sc, Some(LrScRecord::Lr { paddr: PhysAddr(0x8000_0000) })));

        let ctrl_sc = ControlSignals {
            atomic_op: crate::core::pipeline::signals::AtomicOp::Sc,
            width: crate::core::pipeline::signals::MemWidth::Word,
            ..Default::default()
        };
        store_buffer.allocate(RobTag(2), crate::core::pipeline::signals::MemWidth::Word);

        let mut input_sc = vec![Mem1Mem2Entry {
            rob_tag: RobTag(2),
            pc: 0x1004,
            inst: 0,
            inst_size: InstSize::Standard,
            rd: RegIdx::new(2),
            rd_phys: crate::core::pipeline::prf::PhysReg(0),
            alu: 0,
            vaddr: VirtAddr::new(0x8000_0000),
            paddr: PhysAddr::new(0x8000_0000),
            store_data: 42,
            ctrl: ctrl_sc,
            trap: None,
            exception_stage: None,
            fp_flags: 0,
            complete_cycle: 10,
            pte_update: None,
            sfence_vma: None,
            vec_mem: None,
        }];

        memory2_stage(&mut cpu, &mut input_sc, &mut output, &mut store_buffer, None, None);
        // SC optimistically returns 0 (success) — actual check deferred to commit
        assert_eq!(output[0].load_data, 0);
        assert!(matches!(output[0].lr_sc, Some(LrScRecord::Sc { paddr: PhysAddr(0x8000_0000) })));
        // Reservation unchanged at Memory2 (still not set — LR was deferred)
        assert!(!cpu.check_reservation(PhysAddr::new(0x8000_0000)));
    }

    #[test]
    fn test_memory2_ordering_violation() {
        let config = Config::default();
        let mut cpu = Cpu::build(&config, "");
        let mut store_buffer = StoreBuffer::new(4);
        let mut load_queue = LoadQueue::new(4);

        // A younger load already executed to the same address
        load_queue.allocate(RobTag(5), crate::core::pipeline::signals::MemWidth::Word, None);
        load_queue.fill_address(
            RobTag(5),
            None,
            VirtAddr::new(0x8000_0000),
            PhysAddr::new(0x8000_0000),
        );
        load_queue.fill_data(RobTag(5), None, 0);

        let ctrl_store = ControlSignals {
            mem_write: true,
            width: crate::core::pipeline::signals::MemWidth::Word,
            ..Default::default()
        };

        store_buffer.allocate(RobTag(2), crate::core::pipeline::signals::MemWidth::Word);

        let mut input = vec![Mem1Mem2Entry {
            rob_tag: RobTag(2), // Older store
            pc: 0x1000,
            inst: 0,
            inst_size: InstSize::Standard,
            rd: RegIdx::new(0),
            rd_phys: crate::core::pipeline::prf::PhysReg(0),
            alu: 0,
            vaddr: VirtAddr::new(0x8000_0000),
            paddr: PhysAddr::new(0x8000_0000),
            store_data: 42,
            ctrl: ctrl_store,
            trap: None,
            exception_stage: None,
            fp_flags: 0,
            complete_cycle: 10,
            pte_update: None,
            sfence_vma: None,
            vec_mem: None,
        }];
        let mut output = Vec::new();

        let violation = memory2_stage(
            &mut cpu,
            &mut input,
            &mut output,
            &mut store_buffer,
            Some(&mut load_queue),
            None,
        );

        assert_eq!(violation, Some((RobTag(5), 0x1000))); // Older store detected overlap with younger load
    }
}
