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
use crate::isa::{abi, csr, sys_ops};
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
}

impl Cpu {
    pub fn new(system: System, config: &Config) -> Self {
        let configured_misa = if let Some(ref override_str) = config.pipeline.misa_override {
            let s = override_str.trim_start_matches("0x");
            u64::from_str_radix(s, 16).unwrap_or(0x8000_0000_0014_1101)
        } else {
            // Default RV64IMAFD:
            // 63:62 = 2 (RV64)
            // Extensions: A(0), D(3), F(5), I(8), M(12)
            let mut val: u64 = 2 << 62;
            val |= 1 << 0; // A - Atomic
            val |= 1 << 3; // D - Double Float
            val |= 1 << 5; // F - Single Float
            val |= 1 << 8; // I - Integer
            val |= 1 << 12; // M - Multiply/Divide
            val
        };

        // Initialize CSRs including the configured MISA
        let csrs = Csrs {
            mstatus: csr::MSTATUS_FS_INIT,
            sstatus: csr::MSTATUS_FS_INIT,
            misa: configured_misa,
            ..Default::default()
        };

        // Initialize branch predictor from config
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
        }
    }

    pub fn tick(&mut self) -> Result<(), String> {
        // Timer & Interrupt Handling
        let timer_irq = self.bus.tick();

        // Update MIP for Timer
        let mut mip = self.csr_read(csr::MIP);
        if timer_irq {
            mip |= csr::MIP_MTIP;
        } else {
            mip &= !csr::MIP_MTIP;
        }
        self.csr_write(csr::MIP, mip);

        let mie = self.csr_read(csr::MIE);
        let mstatus = self.csrs.mstatus;

        // Extract Global Interrupt Enables
        let m_global_ie = (mstatus & csr::MSTATUS_MIE) != 0;
        let s_global_ie = (mstatus & csr::MSTATUS_SIE) != 0;
        let u_global_ie = (mstatus & csr::MSTATUS_UIE) != 0;

        let check_irq = |pending_bit: u64, enable_bit: u64, mode: u8, global_ie: bool| -> bool {
            let pending = (mip & pending_bit) != 0;
            let enabled = (mie & enable_bit) != 0;
            if !pending || !enabled {
                return false;
            }

            if self.privilege < mode {
                return true;
            }
            if self.privilege == mode {
                return global_ie;
            }
            false
        };

        // Priority Order (External > Software > Timer, M > S > U)
        let trap_cause = if check_irq(csr::MIP_MEIP, csr::MIE_MEIP, 3, m_global_ie) {
            Some(Trap::ExternalInterrupt)
        } else if check_irq(csr::MIP_MSIP, csr::MIE_MSIP, 3, m_global_ie) {
            Some(Trap::MachineSoftwareInterrupt)
        } else if check_irq(csr::MIP_MTIP, csr::MIE_MTIE, 3, m_global_ie) {
            Some(Trap::MachineTimerInterrupt)
        } else if check_irq(csr::MIP_SEIP, csr::MIE_SEIP, 1, s_global_ie) {
            Some(Trap::ExternalInterrupt)
        } else if check_irq(csr::MIP_SSIP, csr::MIE_SSIP, 1, s_global_ie) {
            Some(Trap::SupervisorSoftwareInterrupt)
        } else if check_irq(csr::MIP_STIP, csr::MIE_STIE, 1, s_global_ie) {
            Some(Trap::SupervisorTimerInterrupt)
        } else if check_irq(csr::MIP_USIP, csr::MIE_USIP, 0, u_global_ie) {
            Some(Trap::UserSoftwareInterrupt)
        } else {
            None
        };

        if let Some(trap) = trap_cause {
            self.trap(trap, self.pc);
            return Ok(());
        }

        if self.trace {
            self.print_pipeline_diagram();
        }

        // Handle stalls (Memory)
        if self.stall_cycles > 0 {
            self.stall_cycles -= 1;
            self.stats.cycles += 1;
            self.stats.stalls_mem += 1;
            self.track_mode_cycles();
            return Ok(());
        }

        // Handle stalls (ALU / Multi-cycle ops)
        if self.alu_timer > 0 {
            self.alu_timer -= 1;
            self.stats.cycles += 1;
            self.track_mode_cycles();
            return Ok(());
        }

        // Advance cycle & stats
        self.stats.cycles += 1;
        self.track_mode_cycles();

        // Write Back Stage
        if let Err(trap) = stages::write_back::wb_stage(self) {
            return Err(format!("{:?}", trap));
        }

        // Check for program exit
        if self.exit_code.is_some() {
            return Ok(());
        }

        // Latch MEM result to WB
        self.wb_latch = self.mem_wb.clone();

        // Memory & Execute Stages
        stages::memory_access::mem_stage(self)?;
        stages::execute::execute_stage(self)?;

        // Hazard Detection (Load-Use)
        let is_load_use_hazard = control::need_stall_load_use(&self.id_ex, self.if_id.inst);

        if is_load_use_hazard {
            // Stall: Inject Bubble into ID/EX, do not fetch new instruction
            self.id_ex = IdEx::bubble();
            self.stats.stalls_data += 1;
        } else {
            // Normal operation: Decode & Fetch
            stages::decode::decode_stage(self)?;
            stages::fetch::fetch_stage(self)?;
        }

        // Hardwire zero register
        self.regs.write(abi::REG_ZERO, 0);

        Ok(())
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

        // Standard MMU Translation (SV39)
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
        let code = match cause {
            Trap::InstructionAddressMisaligned(_) => 0,
            Trap::InstructionAccessFault(_) => 1,
            Trap::IllegalInstruction(_) => 2,
            Trap::Breakpoint(_) => 3,
            Trap::LoadAddressMisaligned(_) => 4,
            Trap::LoadAccessFault(_) => 5,
            Trap::StoreAddressMisaligned(_) => 6,
            Trap::StoreAccessFault(_) => 7,
            Trap::EnvironmentCallFromUMode => 8,
            Trap::EnvironmentCallFromSMode => 9,
            Trap::EnvironmentCallFromMMode => 11,
            Trap::InstructionPageFault(_) => 12,
            Trap::LoadPageFault(_) => 13,
            Trap::StorePageFault(_) => 15,
            Trap::MachineTimerInterrupt => 0x8000_0000_0000_0007,
            _ => 0,
        };

        if self.direct_mode {
            if let Trap::EnvironmentCallFromUMode = cause {
                let get_reg = |reg: usize, cpu: &Self| -> u64 {
                    if reg == abi::REG_ZERO {
                        return 0;
                    }
                    if cpu.ex_mem.ctrl.reg_write && cpu.ex_mem.rd == reg {
                        return cpu.ex_mem.alu;
                    }
                    if cpu.mem_wb.ctrl.reg_write && cpu.mem_wb.rd == reg {
                        if cpu.mem_wb.ctrl.mem_read {
                            return cpu.mem_wb.load_data;
                        } else {
                            return cpu.mem_wb.alu;
                        }
                    }
                    cpu.regs.read(reg)
                };

                let a7 = get_reg(abi::REG_A7, self);
                if a7 == sys_ops::SYS_EXIT {
                    let code = get_reg(abi::REG_A0, self);
                    self.exit_code = Some(code);
                    return;
                }
            }
            eprintln!(
                "Unhandled Trap in Direct Mode: Cause={:?}, PC={:#x}",
                cause, epc
            );
            self.exit_code = Some(1);
            return;
        }

        self.stats.traps_taken += 1;
        self.csrs.sepc = epc;
        self.csrs.scause = code;

        let mut sstatus = self.csrs.sstatus;
        if self.privilege == 0 {
            sstatus &= !csr::MSTATUS_SPP;
        } else {
            sstatus |= csr::MSTATUS_SPP;
        }
        let sie = (sstatus & csr::MSTATUS_MIE) != 0;
        if sie {
            sstatus |= csr::MSTATUS_MPIE;
        } else {
            sstatus &= !csr::MSTATUS_MPIE;
        }
        sstatus &= !csr::MSTATUS_MIE;
        self.csrs.sstatus = sstatus;

        let vector = self.csrs.stvec & !3;
        self.pc = vector;
        self.privilege = 1;

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
        let fmt = |pc, inst, _lbl| {
            if inst == 0x13 || inst == 0 {
                format!("[{:^8}]", "nop")
            } else {
                format!("[{:08x}]", pc)
            }
        };
        eprintln!(
            "{} -> {} -> {} -> {} -> {}",
            fmt(self.if_id.pc, self.if_id.inst, "IF"),
            fmt(self.id_ex.pc, self.id_ex.inst, "ID"),
            fmt(self.ex_mem.pc, self.ex_mem.inst, "EX"),
            fmt(self.mem_wb.pc, self.mem_wb.inst, "MEM"),
            fmt(self.wb_latch.pc, self.wb_latch.inst, "WB")
        );
    }

    pub(crate) fn csr_read(&self, addr: u32) -> u64 {
        match addr {
            csr::MVENDORID => 0,
            csr::MARCHID => 0,
            csr::MIMPID => 0,
            csr::MHARTID => 0,

            csr::MSTATUS => self.csrs.mstatus,
            csr::MEDELEG => 0,
            csr::MIDELEG => 0,
            csr::MIE => 0,
            csr::MTVEC => self.csrs.mtvec,
            csr::MCOUNTEREN => 0,
            csr::MISA => self.csrs.misa,

            csr::MSCRATCH => self.csrs.mscratch,
            csr::MEPC => self.csrs.mepc,
            csr::MCAUSE => self.csrs.mcause,
            csr::MTVAL => self.csrs.mtval,
            csr::MIP => 0,

            csr::SSTATUS => self.csrs.sstatus,
            csr::SIE => 0,
            csr::STVEC => self.csrs.stvec,
            csr::SCOUNTEREN => 0,

            csr::SSCRATCH => self.csrs.sscratch,
            csr::SEPC => self.csrs.sepc,
            csr::SCAUSE => self.csrs.scause,
            csr::STVAL => self.csrs.stval,
            csr::SIP => 0,

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
            csr::MEDELEG => {}
            csr::MIDELEG => {}
            csr::MIE => {}
            csr::MTVEC => self.csrs.mtvec = val,
            csr::MCOUNTEREN => {}
            csr::MISA => self.csrs.misa = val,

            csr::MSCRATCH => self.csrs.mscratch = val,
            csr::MEPC => self.csrs.mepc = val & !1,
            csr::MCAUSE => self.csrs.mcause = val,
            csr::MTVAL => self.csrs.mtval = val,
            csr::MIP => {}

            csr::SSTATUS => self.csrs.sstatus = val,
            csr::SIE => {}
            csr::STVEC => self.csrs.stvec = val,
            csr::SCOUNTEREN => {}

            csr::SSCRATCH => self.csrs.sscratch = val,
            csr::SEPC => self.csrs.sepc = val & !1,
            csr::SCAUSE => self.csrs.scause = val,
            csr::STVAL => self.csrs.stval = val,
            csr::SIP => {}

            csr::SATP => self.csrs.satp = val,

            _ => {}
        }
    }

    pub(crate) fn do_mret(&mut self) {
        self.pc = self.csrs.mepc & !1;
        self.privilege = 1;
        self.if_id = Default::default();
        self.id_ex = IdEx::default();
    }

    pub(crate) fn do_sret(&mut self) {
        self.pc = self.csrs.sepc & !1;
        let spp = (self.csrs.sstatus & csr::MSTATUS_SPP) != 0;
        self.privilege = if spp { 1 } else { 0 };
        let spie = (self.csrs.sstatus & csr::MSTATUS_MPIE) != 0;

        if spie {
            self.csrs.sstatus |= csr::MSTATUS_MIE;
        } else {
            self.csrs.sstatus &= !csr::MSTATUS_MIE;
        }

        self.csrs.sstatus |= csr::MSTATUS_MPIE;
        self.if_id = Default::default();
        self.id_ex = IdEx::default();
    }
}
