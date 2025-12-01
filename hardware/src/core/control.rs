use super::pipeline::{ExMem, IdEx, MemWb};

#[derive(Clone, Copy, Debug, Default)]
pub enum AluOp {
    #[default]
    Add,
    Sub,
    Sll,
    Slt,
    Sltu,
    Xor,
    Srl,
    Sra,
    Or,
    And,
    Mul,
    Mulh,
    Mulhsu,
    Mulhu,
    Div,
    Divu,
    Rem,
    Remu,
    FAdd,
    FSub,
    FMul,
    FDiv,
    FSqrt,
    FMin,
    FMax,
    FMAdd,
    FMSub,
    FNMAdd,
    FNMSub,
    FCvtWS,
    FCvtLS,
    FCvtSW,
    FCvtSL,
    FCvtSD,
    FCvtDS,
    FSgnJ,
    FSgnJN,
    FSgnJX,
    FEq,
    FLt,
    FLe,
    FClass,
    FMvToX,
    FMvToF,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AtomicOp {
    #[default]
    None,
    Lr,
    Sc,
    Swap,
    Add,
    Xor,
    And,
    Or,
    Min,
    Max,
    Minu,
    Maxu,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum MemWidth {
    #[default]
    Nop,
    Byte,
    Half,
    Word,
    Double,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum OpASrc {
    #[default]
    Reg1,
    Pc,
    Zero,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum OpBSrc {
    #[default]
    Imm,
    Reg2,
    Zero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CsrOp {
    #[default]
    None,
    Rw,
    Rs,
    Rc,
    Rwi,
    Rsi,
    Rci,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ControlSignals {
    pub reg_write: bool,
    pub fp_reg_write: bool,
    pub mem_read: bool,
    pub mem_write: bool,
    pub branch: bool,
    pub jump: bool,
    pub is_rv32: bool,
    pub width: MemWidth,
    pub signed_load: bool,
    pub alu: AluOp,
    pub a_src: OpASrc,
    pub b_src: OpBSrc,
    pub is_system: bool,
    pub csr_addr: u32,
    pub is_mret: bool,
    pub is_sret: bool,
    pub csr_op: CsrOp,
    pub rs1_fp: bool,
    pub rs2_fp: bool,
    pub rs3_fp: bool,
    pub atomic_op: AtomicOp,
}

pub fn need_stall_load_use(id_ex: &IdEx, if_id_inst: u32) -> bool {
    if !id_ex.ctrl.mem_read {
        return false;
    }

    if !id_ex.ctrl.fp_reg_write && id_ex.rd == 0 {
        return false;
    }

    let next_rs1 = ((if_id_inst >> 15) & 0x1f) as usize;
    let next_rs2 = ((if_id_inst >> 20) & 0x1f) as usize;
    let next_rs3 = ((if_id_inst >> 27) & 0x1f) as usize;

    id_ex.rd == next_rs1 || id_ex.rd == next_rs2 || id_ex.rd == next_rs3
}

pub fn forward_rs(id_ex: &IdEx, ex_mem: &ExMem, mem_wb: &MemWb) -> (u64, u64, u64) {
    let mut a = id_ex.rv1;
    let mut b = id_ex.rv2;
    let mut c = id_ex.rv3;

    let check = |dest: usize, dest_fp: bool, src: usize, src_fp: bool| -> bool {
        if dest_fp != src_fp {
            return false;
        }
        if dest != src {
            return false;
        }
        if !dest_fp && dest == 0 {
            return false;
        }
        true
    };

    if mem_wb.ctrl.reg_write || mem_wb.ctrl.fp_reg_write {
        let wb_val = if mem_wb.ctrl.mem_read {
            mem_wb.load_data
        } else if mem_wb.ctrl.jump {
            mem_wb.pc.wrapping_add(4)
        } else {
            mem_wb.alu
        };

        let dest_fp = mem_wb.ctrl.fp_reg_write;

        if check(mem_wb.rd, dest_fp, id_ex.rs1, id_ex.ctrl.rs1_fp) {
            a = wb_val;
        }
        if check(mem_wb.rd, dest_fp, id_ex.rs2, id_ex.ctrl.rs2_fp) {
            b = wb_val;
        }
        if check(mem_wb.rd, dest_fp, id_ex.rs3, id_ex.ctrl.rs3_fp) {
            c = wb_val;
        }
    }

    if (ex_mem.ctrl.reg_write || ex_mem.ctrl.fp_reg_write) && !ex_mem.ctrl.mem_read {
        let ex_val = if ex_mem.ctrl.jump {
            ex_mem.pc.wrapping_add(4)
        } else {
            ex_mem.alu
        };

        let dest_fp = ex_mem.ctrl.fp_reg_write;

        if check(ex_mem.rd, dest_fp, id_ex.rs1, id_ex.ctrl.rs1_fp) {
            a = ex_val;
        }
        if check(ex_mem.rd, dest_fp, id_ex.rs2, id_ex.ctrl.rs2_fp) {
            b = ex_val;
        }
        if check(ex_mem.rd, dest_fp, id_ex.rs3, id_ex.ctrl.rs3_fp) {
            c = ex_val;
        }
    }

    (a, b, c)
}
