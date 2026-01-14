use super::bp;
use super::bp::BranchPredictor;
use super::cache::CacheSim;
use super::control;
use super::mmu::Mmu;
use super::pipeline::{ExMem, IdEx, IfId, MemWb};
use super::register_file::RegisterFile;
use super::stages;
use super::types::{AccessType, PhysAddr, TranslationResult, Trap, VirtAddr};
use crate::config::Config;
use crate::isa::{abi, csr};
use crate::stats::SimStats;
use crate::system::System;

#[derive(Default)]
pub struct Csrs {
    pub mstatus: u64,
    pub sstatus: u64,
    pub mepc: u64,
    pub sepc: u64,
    pub mtvec: u64,
    pub stvec: u64,
    pub scause: u64,
    pub sscratch: u64,
    pub satp: u64,
    pub mscratch: u64,
    pub mcause: u64,
    pub mtval: u64,
    pub stval: u64,
    pub misa: u64,
    pub medeleg: u64,
    pub mideleg: u64,
    pub mip: u64,
    pub mie: u64,
}

pub struct Cpu {
    pub regs: RegisterFile,
    pub pc: u64,
    pub trace: bool,
    pub bus: System,
    pub exit_code: Option<u64>,

    pub csrs: Csrs,
    pub privilege: u8, // 0=User, 1=Supervisor, 3=Machine

    pub direct_mode: bool,
    pub mmio_base: u64,

    pub if_id: IfId,
    pub id_ex: IdEx,
    pub ex_mem: ExMem,
    pub mem_wb: MemWb,
    pub wb_latch: MemWb,

    pub stats: SimStats,

    pub branch_predictor: Box<dyn BranchPredictor>,
    pub l1_i_cache: CacheSim,
    pub l1_d_cache: CacheSim,
    pub l2_cache: CacheSim,
    pub l3_cache: CacheSim,

    pub stall_cycles: u64,
    pub alu_timer: u64,

    pub mmu: Mmu,

    pub load_reservation: Option<u64>,
    pub pipeline_width: usize,
}

impl Cpu {
    pub fn new(system: System, config: &Config) -> Self {
        let configured_misa = if let Some(ref override_str) = config.pipeline.misa_override {
            let s = override_str.trim_start_matches("0x");
            u64::from_str_radix(s, 16).unwrap_or(0x8000_0000_0014_1101)
        } else {
            // Default RV64IMAFDC
            let mut val: u64 = 2 << 62;
            val |= 1 << 0; // A
            val |= 1 << 2; // C
            val |= 1 << 3; // D
            val |= 1 << 5; // F
            val |= 1 << 8; // I
            val |= 1 << 12; // M
            val |= 1 << 18; // S
            val |= 1 << 20; // U
            val
        };

        let csrs = Csrs {
            mstatus: 0xa000_00000, // SXL=2, UXL=2
            misa: configured_misa,
            ..Default::default()
        };

        let bp: Box<dyn BranchPredictor> = match config.pipeline.branch_predictor.as_str() {
            "Static" => Box::new(bp::static_bp::StaticPredictor::new(
                config.pipeline.btb_size,
                config.pipeline.ras_size,
            )),
            "GShare" => Box::new(bp::gshare::GSharePredictor::new(
                config.pipeline.btb_size,
                config.pipeline.ras_size,
            )),
            "Tournament" => Box::new(bp::tournament::TournamentPredictor::new(
                &config.pipeline.tournament,
                config.pipeline.btb_size,
                config.pipeline.ras_size,
            )),
            "TAGE" => Box::new(bp::tage::TagePredictor::new(
                &config.pipeline.tage,
                config.pipeline.btb_size,
                config.pipeline.ras_size,
            )),
            _ => Box::new(bp::perceptron::PerceptronPredictor::new(
                &config.pipeline.perceptron,
                config.pipeline.btb_size,
                config.pipeline.ras_size,
            )),
        };

        Self {
            regs: RegisterFile::new(),
            pc: config.general.start_pc_val(),
            trace: config.general.trace_instructions,
            bus: system,
            exit_code: None,
            csrs,
            privilege: 3,
            direct_mode: false,
            mmio_base: config.system.disk_base_val(),
            if_id: IfId::default(),
            id_ex: IdEx::default(),
            ex_mem: ExMem::default(),
            mem_wb: MemWb::default(),
            wb_latch: MemWb::default(),
            stats: SimStats::default(),
            branch_predictor: bp,
            l1_i_cache: CacheSim::new(&config.cache.l1_i),
            l1_d_cache: CacheSim::new(&config.cache.l1_d),
            l2_cache: CacheSim::new(&config.cache.l2),
            l3_cache: CacheSim::new(&config.cache.l3),
            stall_cycles: 0,
            alu_timer: 0,
            mmu: Mmu::new(config.memory.tlb_size),
            load_reservation: None,
            pipeline_width: config.pipeline.width,
        }
    }

    pub fn tick(&mut self) -> Result<(), String> {
        if let Some(code) = self.bus.check_exit() {
            self.exit_code = Some(code);
            return Ok(());
        }

        // 1. Update Time/Interrupts from Bus
        let (timer_irq, external_irq) = self.bus.tick();

        // 2. Update MIP
        let mut mip = self.csrs.mip;
        if timer_irq {
            mip |= csr::MIP_MTIP;
        } else {
            mip &= !csr::MIP_MTIP;
        }
        if external_irq {
            mip |= csr::MIP_MEIP;
        } else {
            mip &= !csr::MIP_MEIP;
        }
        self.csrs.mip = mip;

        // 3. Check Interrupts
        let mie = self.csrs.mie;
        let mstatus = self.csrs.mstatus;

        let m_global_ie = (mstatus & csr::MSTATUS_MIE) != 0;
        let s_global_ie = (mstatus & csr::MSTATUS_SIE) != 0;

        let check = |bit: u64, enable_bit: u64, deleg_bit: u64| -> Option<Trap> {
            let pending = (mip & bit) != 0;
            let enabled = (mie & enable_bit) != 0;
            if !pending || !enabled {
                return None;
            }

            let delegated = (self.csrs.mideleg & deleg_bit) != 0;
            let target_priv = if delegated { 1 } else { 3 };

            if self.privilege < target_priv {
                return Some(self.irq_to_trap(bit));
            }
            if self.privilege == target_priv {
                if target_priv == 3 && m_global_ie {
                    return Some(self.irq_to_trap(bit));
                }
                if target_priv == 1 && s_global_ie {
                    return Some(self.irq_to_trap(bit));
                }
            }
            None
        };

        let trap = check(csr::MIP_MEIP, csr::MIE_MEIP, 1 << 11)
            .or_else(|| check(csr::MIP_MSIP, csr::MIE_MSIP, 1 << 3))
            .or_else(|| check(csr::MIP_MTIP, csr::MIE_MTIE, 1 << 7))
            .or_else(|| check(csr::MIP_SEIP, csr::MIE_SEIP, 1 << 9))
            .or_else(|| check(csr::MIP_SSIP, csr::MIE_SSIP, 1 << 1))
            .or_else(|| check(csr::MIP_STIP, csr::MIE_STIE, 1 << 5));

        if let Some(t) = trap {
            self.trap(t, self.pc);
            return Ok(());
        }

        if self.trace {
            self.print_pipeline_diagram();
        }

        if self.stall_cycles > 0 {
            self.stall_cycles -= 1;
            self.stats.cycles += 1;
            self.stats.stalls_mem += 1;
            self.track_mode_cycles();
            return Ok(());
        }
        if self.alu_timer > 0 {
            self.alu_timer -= 1;
            self.stats.cycles += 1;
            self.track_mode_cycles();
            return Ok(());
        }

        self.stats.cycles += 1;
        self.track_mode_cycles();

        if let Err(trap) = stages::write_back::wb_stage(self) {
            return Err(format!("{:?}", trap));
        }
        if self.exit_code.is_some() {
            return Ok(());
        }

        self.wb_latch = self.mem_wb.clone();
        stages::memory_access::mem_stage(self)?;
        stages::execute::execute_stage(self)?;

        let is_load_use_hazard = control::need_stall_load_use(&self.id_ex, &self.if_id);
        if is_load_use_hazard {
            self.id_ex = IdEx::bubble();
            self.stats.stalls_data += 1;
        } else {
            stages::decode::decode_stage(self)?;
            if self.if_id.entries.is_empty() {
                stages::fetch::fetch_stage(self)?;
            }
        }

        self.regs.write(abi::REG_ZERO, 0);
        Ok(())
    }

    fn irq_to_trap(&self, bit: u64) -> Trap {
        match bit {
            csr::MIP_MEIP => Trap::ExternalInterrupt,
            csr::MIP_MSIP => Trap::MachineSoftwareInterrupt,
            csr::MIP_MTIP => Trap::MachineTimerInterrupt,
            csr::MIP_SEIP => Trap::ExternalInterrupt,
            csr::MIP_SSIP => Trap::SupervisorSoftwareInterrupt,
            csr::MIP_STIP => Trap::SupervisorTimerInterrupt,
            _ => Trap::MachineTimerInterrupt,
        }
    }

    pub fn translate(&mut self, vaddr: VirtAddr, access: AccessType) -> TranslationResult {
        if self.direct_mode {
            let paddr = vaddr.val();
            if !self.bus.bus.is_valid_address(paddr) {
                let trap = match access {
                    AccessType::Fetch => Trap::InstructionAccessFault(paddr),
                    AccessType::Read => Trap::LoadAccessFault(paddr),
                    AccessType::Write => Trap::StoreAccessFault(paddr),
                };
                return TranslationResult::fault(trap, 0);
            }
            return TranslationResult::success(PhysAddr::new(paddr), 0);
        }
        let res = self
            .mmu
            .translate(vaddr, access, self.privilege, &self.csrs, &mut self.bus.bus);
        if res.trap.is_none() {
            let paddr = res.paddr.val();
            if !self.bus.bus.is_valid_address(paddr) {
                let trap = match access {
                    AccessType::Fetch => Trap::InstructionAccessFault(paddr),
                    AccessType::Read => Trap::LoadAccessFault(paddr),
                    AccessType::Write => Trap::StoreAccessFault(paddr),
                };
                return TranslationResult::fault(trap, res.cycles);
            }
        }
        res
    }

    pub fn simulate_memory_access(&mut self, addr: PhysAddr, access: AccessType) -> u64 {
        let mut total_penalty = 0;
        let raw_addr = addr.val();
        let ram_latency = self.bus.mem_controller.access_latency(raw_addr);
        let next_lat = ram_latency;
        let is_inst = matches!(access, AccessType::Fetch);
        let is_write = matches!(access, AccessType::Write);

        let (l1_hit, l1_pen) = if is_inst {
            if self.l1_i_cache.enabled {
                self.l1_i_cache.access(raw_addr, false, next_lat)
            } else {
                (false, 0)
            }
        } else if self.l1_d_cache.enabled {
            self.l1_d_cache.access(raw_addr, is_write, next_lat)
        } else {
            (false, 0)
        };

        total_penalty += l1_pen;
        if is_inst && self.l1_i_cache.enabled {
            if l1_hit {
                self.stats.icache_hits += 1;
                return total_penalty;
            }
            self.stats.icache_misses += 1;
        } else if !is_inst && self.l1_d_cache.enabled {
            if l1_hit {
                self.stats.dcache_hits += 1;
                return total_penalty;
            }
            self.stats.dcache_misses += 1;
        }

        if self.l2_cache.enabled {
            total_penalty += self.l2_cache.latency;
            let (l2_hit, l2_pen) = self.l2_cache.access(raw_addr, is_write, next_lat);
            total_penalty += l2_pen;
            if l2_hit {
                self.stats.l2_hits += 1;
                return total_penalty;
            }
            self.stats.l2_misses += 1;
        }

        if self.l3_cache.enabled {
            total_penalty += self.l3_cache.latency;
            let (l3_hit, l3_pen) = self.l3_cache.access(raw_addr, is_write, next_lat);
            total_penalty += l3_pen;
            if l3_hit {
                self.stats.l3_hits += 1;
                return total_penalty;
            }
            self.stats.l3_misses += 1;
        }

        total_penalty += self.bus.bus.calculate_transit_time(8);
        total_penalty += ram_latency;
        total_penalty += self.bus.bus.calculate_transit_time(64);
        total_penalty
    }

    pub fn trap(&mut self, cause: Trap, epc: u64) {
        let (is_interrupt, code) = match cause {
            Trap::InstructionAddressMisaligned(_) => (false, 0),
            Trap::InstructionAccessFault(_) => (false, 1),
            Trap::IllegalInstruction(_) => (false, 2),
            Trap::Breakpoint(_) => (false, 3),
            Trap::LoadAddressMisaligned(_) => (false, 4),
            Trap::LoadAccessFault(_) => (false, 5),
            Trap::StoreAddressMisaligned(_) => (false, 6),
            Trap::StoreAccessFault(_) => (false, 7),
            Trap::EnvironmentCallFromUMode => (false, 8),
            Trap::EnvironmentCallFromSMode => (false, 9),
            Trap::EnvironmentCallFromMMode => (false, 11),
            Trap::InstructionPageFault(_) => (false, 12),
            Trap::LoadPageFault(_) => (false, 13),
            Trap::StorePageFault(_) => (false, 15),
            Trap::UserSoftwareInterrupt => (true, 0),
            Trap::SupervisorSoftwareInterrupt => (true, 1),
            Trap::MachineSoftwareInterrupt => (true, 3),
            Trap::SupervisorTimerInterrupt => (true, 5),
            Trap::MachineTimerInterrupt => (true, 7),
            Trap::ExternalInterrupt => (true, 9),
            Trap::RequestedTrap(c) => (false, c),
        };

        let deleg_mask = if is_interrupt {
            self.csrs.mideleg
        } else {
            self.csrs.medeleg
        };
        let delegate_to_s = (self.privilege <= 1) && ((deleg_mask >> code) & 1) != 0;

        let tval = match cause {
            Trap::InstructionAddressMisaligned(a)
            | Trap::InstructionAccessFault(a)
            | Trap::LoadAddressMisaligned(a)
            | Trap::LoadAccessFault(a)
            | Trap::StoreAddressMisaligned(a)
            | Trap::StoreAccessFault(a)
            | Trap::InstructionPageFault(a)
            | Trap::LoadPageFault(a)
            | Trap::StorePageFault(a) => a,
            Trap::IllegalInstruction(i) => i as u64,
            _ => 0,
        };

        if delegate_to_s {
            self.csrs.scause = if is_interrupt { (1 << 63) | code } else { code };
            self.csrs.sepc = epc;
            self.csrs.stval = tval;

            let mut sstatus = self.csrs.sstatus;
            if (sstatus & csr::MSTATUS_SIE) != 0 {
                sstatus |= csr::MSTATUS_SPIE;
            } else {
                sstatus &= !csr::MSTATUS_SPIE;
            }
            if self.privilege == 1 {
                sstatus |= csr::MSTATUS_SPP;
            } else {
                sstatus &= !csr::MSTATUS_SPP;
            }
            sstatus &= !csr::MSTATUS_SIE;
            self.csrs.sstatus = sstatus;

            self.privilege = 1;
            self.pc = (self.csrs.stvec & !3)
                + (if (self.csrs.stvec & 1) != 0 && is_interrupt {
                    4 * code
                } else {
                    0
                });
        } else {
            self.csrs.mcause = if is_interrupt { (1 << 63) | code } else { code };
            self.csrs.mepc = epc;
            self.csrs.mtval = tval;

            let mut mstatus = self.csrs.mstatus;
            if (mstatus & csr::MSTATUS_MIE) != 0 {
                mstatus |= csr::MSTATUS_MPIE;
            } else {
                mstatus &= !csr::MSTATUS_MPIE;
            }
            mstatus &= !csr::MSTATUS_MPP;
            mstatus |= (self.privilege as u64) << 11;
            mstatus &= !csr::MSTATUS_MIE;
            self.csrs.mstatus = mstatus;

            self.privilege = 3;
            self.pc = (self.csrs.mtvec & !3)
                + (if (self.csrs.mtvec & 1) != 0 && is_interrupt {
                    4 * code
                } else {
                    0
                });
        }

        self.stats.traps_taken += 1;
        self.if_id = Default::default();
        self.id_ex = IdEx::default();
    }

    pub fn take_exit(&mut self) -> Option<u64> {
        self.exit_code.take()
    }

    pub fn dump_state(&self) {
        println!("PC = {:#018x}", self.pc);
        self.regs.dump();
    }

    fn track_mode_cycles(&mut self) {
        match self.privilege {
            0 => self.stats.cycles_user += 1,
            1 => self.stats.cycles_kernel += 1,
            3 => self.stats.cycles_machine += 1,
            _ => {}
        }
    }

    pub fn print_pipeline_diagram(&self) {
        eprintln!(
            "IF:{} -> ID:{} -> EX:{} -> MEM:{} -> WB:{}",
            self.if_id.entries.len(),
            self.id_ex.entries.len(),
            self.ex_mem.entries.len(),
            self.mem_wb.entries.len(),
            self.wb_latch.entries.len()
        );
    }

    pub(crate) fn csr_read(&self, addr: u32) -> u64 {
        match addr {
            csr::MVENDORID => 0,
            csr::MARCHID => 0,
            csr::MIMPID => 0,
            csr::MHARTID => 0,
            csr::MSTATUS => self.csrs.mstatus,
            csr::MEDELEG => self.csrs.medeleg,
            csr::MIDELEG => self.csrs.mideleg,
            csr::MIE => self.csrs.mie,
            csr::MTVEC => self.csrs.mtvec,
            csr::MISA => self.csrs.misa,
            csr::MSCRATCH => self.csrs.mscratch,
            csr::MEPC => self.csrs.mepc,
            csr::MCAUSE => self.csrs.mcause,
            csr::MTVAL => self.csrs.mtval,
            csr::MIP => self.csrs.mip,
            csr::SSTATUS => self.csrs.sstatus,
            csr::SIE => self.csrs.mie & self.csrs.mideleg,
            csr::STVEC => self.csrs.stvec,
            csr::SSCRATCH => self.csrs.sscratch,
            csr::SEPC => self.csrs.sepc,
            csr::SCAUSE => self.csrs.scause,
            csr::STVAL => self.csrs.stval,
            csr::SIP => self.csrs.mip & self.csrs.mideleg,
            csr::SATP => self.csrs.satp,
            csr::CYCLE | csr::MCYCLE | csr::TIME => self.stats.cycles,
            csr::INSTRET | csr::MINSTRET => self.stats.instructions_retired,
            _ => 0,
        }
    }

    pub(crate) fn csr_write(&mut self, addr: u32, val: u64) {
        match addr {
            csr::CSR_SIM_PANIC => {
                self.trap(Trap::RequestedTrap(val), self.pc);
            }
            csr::MSTATUS => self.csrs.mstatus = val,
            csr::MEDELEG => self.csrs.medeleg = val,
            csr::MIDELEG => self.csrs.mideleg = val,
            csr::MIE => self.csrs.mie = val,
            csr::MTVEC => self.csrs.mtvec = val,
            csr::MISA => self.csrs.misa = val,
            csr::MSCRATCH => self.csrs.mscratch = val,
            csr::MEPC => self.csrs.mepc = val & !1,
            csr::MCAUSE => self.csrs.mcause = val,
            csr::MTVAL => self.csrs.mtval = val,
            csr::MIP => {
                let mask = csr::MIP_SSIP | csr::MIP_STIP | csr::MIP_SEIP;
                self.csrs.mip = (self.csrs.mip & !mask) | (val & mask);
            }
            csr::SSTATUS => {
                let mask = csr::MSTATUS_SIE
                    | csr::MSTATUS_SPIE
                    | csr::MSTATUS_SPP
                    | csr::MSTATUS_FS
                    | csr::MSTATUS_SUM
                    | csr::MSTATUS_MXR;
                self.csrs.mstatus = (self.csrs.mstatus & !mask) | (val & mask);
                self.csrs.sstatus = self.csrs.mstatus & mask;
            }
            csr::SIE => {
                let mask = self.csrs.mideleg;
                self.csrs.mie = (self.csrs.mie & !mask) | (val & mask);
            }
            csr::STVEC => self.csrs.stvec = val,
            csr::SSCRATCH => self.csrs.sscratch = val,
            csr::SEPC => self.csrs.sepc = val & !1,
            csr::SCAUSE => self.csrs.scause = val,
            csr::STVAL => self.csrs.stval = val,
            csr::SIP => {
                let mask = self.csrs.mideleg & (csr::MIP_SSIP);
                self.csrs.mip = (self.csrs.mip & !mask) | (val & mask);
            }
            csr::SATP => self.csrs.satp = val,
            _ => {}
        }
    }

    pub(crate) fn do_mret(&mut self) {
        self.pc = self.csrs.mepc & !1;
        let mstatus = self.csrs.mstatus;
        let mpp = (mstatus >> 11) & 3;
        let mpie = (mstatus & csr::MSTATUS_MPIE) != 0;

        self.privilege = mpp as u8;
        let mut new_mstatus = mstatus;
        if mpie {
            new_mstatus |= csr::MSTATUS_MIE;
        } else {
            new_mstatus &= !csr::MSTATUS_MIE;
        }
        new_mstatus |= csr::MSTATUS_MPIE;
        new_mstatus &= !csr::MSTATUS_MPP;

        self.csrs.mstatus = new_mstatus;
        self.if_id = Default::default();
        self.id_ex = IdEx::default();
    }

    pub(crate) fn do_sret(&mut self) {
        self.pc = self.csrs.sepc & !1;
        let sstatus = self.csrs.sstatus;
        let spp = (sstatus & csr::MSTATUS_SPP) != 0;
        let spie = (sstatus & csr::MSTATUS_SPIE) != 0;

        self.privilege = if spp { 1 } else { 0 };
        let mut new_sstatus = sstatus;
        if spie {
            new_sstatus |= csr::MSTATUS_SIE;
        } else {
            new_sstatus &= !csr::MSTATUS_SIE;
        }
        new_sstatus |= csr::MSTATUS_SPIE;
        new_sstatus &= !csr::MSTATUS_SPP;

        self.csrs.sstatus = new_sstatus;
        let mask = csr::MSTATUS_SIE | csr::MSTATUS_SPIE | csr::MSTATUS_SPP;
        self.csrs.mstatus = (self.csrs.mstatus & !mask) | (new_sstatus & mask);

        self.if_id = Default::default();
        self.id_ex = IdEx::default();
    }
}
