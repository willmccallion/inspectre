use crate::core::Cpu;
use crate::core::control::AluOp;
use crate::core::types::Trap;

pub fn wb_stage(cpu: &mut Cpu) -> Result<(), Trap> {
    let wb = cpu.mem_wb.clone();
    if let Some(trap) = wb.trap {
        return Err(trap);
    }

    if cpu.trace {
        eprintln!("WB  pc={:#x}", wb.pc);
    }

    if wb.inst != 0 && wb.inst != 0x13 {
        cpu.stats.instructions_retired += 1;
        if wb.ctrl.mem_read {
            if wb.ctrl.fp_reg_write {
                cpu.stats.inst_fp_load += 1;
            } else {
                cpu.stats.inst_load += 1;
            }
        } else if wb.ctrl.mem_write {
            if wb.ctrl.rs2_fp {
                cpu.stats.inst_fp_store += 1;
            } else {
                cpu.stats.inst_store += 1;
            }
        } else if wb.ctrl.branch || wb.ctrl.jump {
            cpu.stats.inst_branch += 1;
        } else if wb.ctrl.is_system {
            cpu.stats.inst_system += 1;
        } else {
            match wb.ctrl.alu {
                AluOp::FAdd
                | AluOp::FSub
                | AluOp::FMul
                | AluOp::FMin
                | AluOp::FMax
                | AluOp::FSgnJ
                | AluOp::FSgnJN
                | AluOp::FSgnJX
                | AluOp::FEq
                | AluOp::FLt
                | AluOp::FLe
                | AluOp::FClass
                | AluOp::FCvtWS
                | AluOp::FCvtLS
                | AluOp::FCvtSW
                | AluOp::FCvtSL
                | AluOp::FCvtSD
                | AluOp::FCvtDS
                | AluOp::FMvToX
                | AluOp::FMvToF => cpu.stats.inst_fp_arith += 1,
                AluOp::FDiv | AluOp::FSqrt => cpu.stats.inst_fp_div_sqrt += 1,
                AluOp::FMAdd | AluOp::FMSub | AluOp::FNMAdd | AluOp::FNMSub => {
                    cpu.stats.inst_fp_fma += 1
                }
                _ => cpu.stats.inst_alu += 1,
            }
        }
    }

    let val = if wb.ctrl.mem_read {
        wb.load_data
    } else if wb.ctrl.jump {
        wb.pc.wrapping_add(4)
    } else {
        wb.alu
    };

    if cpu.trace {
        if wb.ctrl.reg_write {
            eprintln!("WB  pc={:#x} x{} <= {:#x}", wb.pc, wb.rd, val);
        } else if wb.ctrl.fp_reg_write {
            eprintln!("WB  pc={:#x} f{} <= {:#x}", wb.pc, wb.rd, val);
        } else {
            eprintln!("WB  pc={:#x}", wb.pc);
        }
    }

    if wb.ctrl.fp_reg_write {
        cpu.regs.write_f(wb.rd, val);
    } else if wb.ctrl.reg_write && wb.rd != 0 {
        cpu.regs.write(wb.rd, val);
    }
    Ok(())
}
