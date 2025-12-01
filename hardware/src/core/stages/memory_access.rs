use crate::core::Cpu;
use crate::core::control::{AtomicOp, MemWidth};
use crate::core::pipeline::MemWb;
use crate::core::types::{AccessType, TranslationResult, Trap, VirtAddr};

fn atomic_alu(op: AtomicOp, mem_val: u64, reg_val: u64, width: MemWidth) -> u64 {
    if matches!(width, MemWidth::Word) {
        let a = mem_val as i32;
        let b = reg_val as i32;
        let res = match op {
            AtomicOp::Swap => b,
            AtomicOp::Add => a.wrapping_add(b),
            AtomicOp::Xor => a ^ b,
            AtomicOp::And => a & b,
            AtomicOp::Or => a | b,
            AtomicOp::Min => a.min(b),
            AtomicOp::Max => a.max(b),
            AtomicOp::Minu => (mem_val as u32).min(reg_val as u32) as i32,
            AtomicOp::Maxu => (mem_val as u32).max(reg_val as u32) as i32,
            _ => 0,
        };
        res as i64 as u64
    } else {
        let a = mem_val as i64;
        let b = reg_val as i64;
        let res = match op {
            AtomicOp::Swap => b,
            AtomicOp::Add => a.wrapping_add(b),
            AtomicOp::Xor => a ^ b,
            AtomicOp::And => a & b,
            AtomicOp::Or => a | b,
            AtomicOp::Min => a.min(b),
            AtomicOp::Max => a.max(b),
            AtomicOp::Minu => (mem_val).min(reg_val) as i64,
            AtomicOp::Maxu => (mem_val).max(reg_val) as i64,
            _ => 0,
        };
        res as u64
    }
}

pub fn mem_stage(cpu: &mut Cpu) -> Result<(), String> {
    let ex = cpu.ex_mem.clone();

    if ex.ctrl.mem_read || ex.ctrl.mem_write {
        let align_mask = match ex.ctrl.width {
            MemWidth::Byte => 0,
            MemWidth::Half => 1,
            MemWidth::Word => 3,
            MemWidth::Double => 7,
            _ => 0,
        };

        if (ex.alu & align_mask) != 0 {
            let trap = if ex.ctrl.mem_read {
                Trap::LoadAddressMisaligned(ex.alu)
            } else {
                Trap::StoreAddressMisaligned(ex.alu)
            };

            cpu.mem_wb = MemWb {
                pc: ex.pc,
                inst: ex.inst,
                rd: ex.rd,
                alu: 0,
                load_data: 0,
                ctrl: ex.ctrl,
                trap: Some(trap),
            };
            return Ok(());
        }
    }

    if let Some(trap) = ex.trap {
        cpu.mem_wb = MemWb {
            pc: ex.pc,
            inst: ex.inst,
            rd: ex.rd,
            alu: 0,
            load_data: 0,
            ctrl: ex.ctrl,
            trap: Some(trap),
        };
        return Ok(());
    }

    if cpu.trace {
        if ex.ctrl.mem_read {
            eprintln!("MEM pc={:#x} LOAD addr={:#x}", ex.pc, ex.alu);
        } else if ex.ctrl.mem_write {
            eprintln!(
                "MEM pc={:#x} STORE addr={:#x} data={:#x}",
                ex.pc, ex.alu, ex.store_data
            );
        } else {
            eprintln!("MEM pc={:#x}", ex.pc);
        }
    }

    let mut ld = 0;
    let mut trap = None;

    if ex.ctrl.mem_read || ex.ctrl.mem_write {
        let access_type = if ex.ctrl.mem_write {
            AccessType::Write
        } else {
            AccessType::Read
        };

        let TranslationResult {
            paddr,
            cycles,
            trap: fault,
        } = cpu.translate(VirtAddr::new(ex.alu), access_type);
        cpu.stall_cycles += cycles;

        if let Some(t) = fault {
            trap = Some(t);
        } else {
            if paddr.val() < cpu.mmio_base {
                let lat = cpu.simulate_memory_access(paddr, access_type);
                cpu.stall_cycles += lat;
            }

            let raw_paddr = paddr.val();

            if ex.ctrl.atomic_op != AtomicOp::None {
                match ex.ctrl.atomic_op {
                    AtomicOp::Lr => {
                        ld = match ex.ctrl.width {
                            MemWidth::Word => {
                                (cpu.bus.bus.read_u32(raw_paddr) as i32) as i64 as u64
                            }
                            MemWidth::Double => cpu.bus.bus.read_u64(raw_paddr),
                            _ => 0,
                        };
                        cpu.load_reservation = Some(raw_paddr);
                    }
                    AtomicOp::Sc => {
                        if cpu.load_reservation == Some(raw_paddr) {
                            match ex.ctrl.width {
                                MemWidth::Word => {
                                    cpu.bus.bus.write_u32(raw_paddr, ex.store_data as u32)
                                }
                                MemWidth::Double => cpu.bus.bus.write_u64(raw_paddr, ex.store_data),
                                _ => {}
                            }
                            ld = 0;
                        } else {
                            ld = 1;
                        }
                        cpu.load_reservation = None;
                    }
                    _ => {
                        let old_val = match ex.ctrl.width {
                            MemWidth::Word => {
                                (cpu.bus.bus.read_u32(raw_paddr) as i32) as i64 as u64
                            }
                            MemWidth::Double => cpu.bus.bus.read_u64(raw_paddr),
                            _ => 0,
                        };

                        let new_val =
                            atomic_alu(ex.ctrl.atomic_op, old_val, ex.store_data, ex.ctrl.width);

                        match ex.ctrl.width {
                            MemWidth::Word => cpu.bus.bus.write_u32(raw_paddr, new_val as u32),
                            MemWidth::Double => cpu.bus.bus.write_u64(raw_paddr, new_val),
                            _ => {}
                        }

                        ld = old_val;
                        if cpu.load_reservation == Some(raw_paddr) {
                            cpu.load_reservation = None;
                        }
                    }
                }
            } else {
                if ex.ctrl.mem_read {
                    ld = match (ex.ctrl.width, ex.ctrl.signed_load) {
                        (MemWidth::Byte, true) => {
                            (cpu.bus.bus.read_u8(raw_paddr) as i8) as i64 as u64
                        }
                        (MemWidth::Half, true) => {
                            (cpu.bus.bus.read_u16(raw_paddr) as i16) as i64 as u64
                        }
                        (MemWidth::Word, true) => {
                            (cpu.bus.bus.read_u32(raw_paddr) as i32) as i64 as u64
                        }
                        (MemWidth::Byte, false) => cpu.bus.bus.read_u8(raw_paddr) as u64,
                        (MemWidth::Half, false) => cpu.bus.bus.read_u16(raw_paddr) as u64,
                        (MemWidth::Word, false) => cpu.bus.bus.read_u32(raw_paddr) as u64,
                        (MemWidth::Double, _) => cpu.bus.bus.read_u64(raw_paddr),
                        _ => 0,
                    };
                    if ex.ctrl.fp_reg_write && matches!(ex.ctrl.width, MemWidth::Word) {
                        ld |= 0xFFFF_FFFF_0000_0000;
                    }
                } else if ex.ctrl.mem_write {
                    if cpu.load_reservation == Some(raw_paddr) {
                        cpu.load_reservation = None;
                    }

                    match ex.ctrl.width {
                        MemWidth::Byte => cpu.bus.bus.write_u8(raw_paddr, ex.store_data as u8),
                        MemWidth::Half => cpu.bus.bus.write_u16(raw_paddr, ex.store_data as u16),
                        MemWidth::Word => cpu.bus.bus.write_u32(raw_paddr, ex.store_data as u32),
                        MemWidth::Double => cpu.bus.bus.write_u64(raw_paddr, ex.store_data),
                        _ => {}
                    }
                }
            }
        }
    }

    cpu.mem_wb = MemWb {
        pc: ex.pc,
        inst: ex.inst,
        rd: ex.rd,
        alu: ex.alu,
        load_data: ld,
        ctrl: ex.ctrl,
        trap,
    };
    Ok(())
}
