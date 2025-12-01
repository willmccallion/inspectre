use std::time::Instant;

pub struct SimStats {
    start_time: Instant,
    pub cycles: u64,
    pub instructions_retired: u64,

    pub inst_load: u64,
    pub inst_store: u64,
    pub inst_branch: u64,
    pub inst_alu: u64,
    pub inst_system: u64,

    pub inst_fp_load: u64,
    pub inst_fp_store: u64,
    pub inst_fp_arith: u64,
    pub inst_fp_fma: u64,
    pub inst_fp_div_sqrt: u64,

    pub branch_predictions: u64,
    pub branch_mispredictions: u64,

    pub cycles_user: u64,
    pub cycles_kernel: u64,
    pub cycles_machine: u64,

    pub stalls_mem: u64,
    pub stalls_control: u64,
    pub stalls_data: u64,

    pub traps_taken: u64,

    pub icache_hits: u64,
    pub icache_misses: u64,
    pub dcache_hits: u64,
    pub dcache_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l3_hits: u64,
    pub l3_misses: u64,
}

impl Default for SimStats {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            cycles: 0,
            instructions_retired: 0,
            inst_load: 0,
            inst_store: 0,
            inst_branch: 0,
            inst_alu: 0,
            inst_system: 0,
            inst_fp_load: 0,
            inst_fp_store: 0,
            inst_fp_arith: 0,
            inst_fp_fma: 0,
            inst_fp_div_sqrt: 0,
            branch_predictions: 0,
            branch_mispredictions: 0,
            cycles_user: 0,
            cycles_kernel: 0,
            cycles_machine: 0,
            stalls_mem: 0,
            stalls_control: 0,
            stalls_data: 0,
            traps_taken: 0,
            icache_hits: 0,
            icache_misses: 0,
            dcache_hits: 0,
            dcache_misses: 0,
            l2_hits: 0,
            l2_misses: 0,
            l3_hits: 0,
            l3_misses: 0,
        }
    }
}

impl SimStats {
    pub fn print(&self) {
        let duration = self.start_time.elapsed();
        let seconds = duration.as_secs_f64();

        let cyc = if self.cycles == 0 { 1 } else { self.cycles };
        let instr = if self.instructions_retired == 0 {
            1
        } else {
            self.instructions_retired
        };

        let ipc = self.instructions_retired as f64 / cyc as f64;
        let cpi = cyc as f64 / instr as f64;
        let mips = (self.instructions_retired as f64 / seconds) / 1_000_000.0;
        let khz = (self.cycles as f64 / seconds) / 1000.0;

        println!("\n==========================================================");
        println!("RISC-V SYSTEM SIMULATION STATISTICS");
        println!("==========================================================");
        println!("host_seconds             {:.4} s", seconds);
        println!("sim_cycles               {}", self.cycles);
        println!("sim_freq                 {:.2} kHz", khz);
        println!("sim_insts                {}", self.instructions_retired);
        println!("sim_ipc                  {:.4}", ipc);
        println!("sim_cpi                  {:.4}", cpi);
        println!("sim_mips                 {:.2}", mips);
        println!("----------------------------------------------------------");
        println!("CORE BREAKDOWN");
        println!(
            "  cycles.user            {} ({:.2}%)",
            self.cycles_user,
            (self.cycles_user as f64 / cyc as f64) * 100.0
        );
        println!(
            "  cycles.kernel          {} ({:.2}%)",
            self.cycles_kernel,
            (self.cycles_kernel as f64 / cyc as f64) * 100.0
        );
        println!(
            "  cycles.machine         {} ({:.2}%)",
            self.cycles_machine,
            (self.cycles_machine as f64 / cyc as f64) * 100.0
        );
        println!(
            "  stalls.memory          {} ({:.2}%)",
            self.stalls_mem,
            (self.stalls_mem as f64 / cyc as f64) * 100.0
        );
        println!(
            "  stalls.control         {} ({:.2}%)",
            self.stalls_control,
            (self.stalls_control as f64 / cyc as f64) * 100.0
        );
        println!(
            "  stalls.data            {} ({:.2}%)",
            self.stalls_data,
            (self.stalls_data as f64 / cyc as f64) * 100.0
        );
        println!("----------------------------------------------------------");
        println!("INSTRUCTION MIX");
        let total_inst = instr as f64;
        println!(
            "  op.alu                 {} ({:.2}%)",
            self.inst_alu,
            (self.inst_alu as f64 / total_inst) * 100.0
        );
        println!(
            "  op.load                {} ({:.2}%)",
            self.inst_load,
            (self.inst_load as f64 / total_inst) * 100.0
        );
        println!(
            "  op.store               {} ({:.2}%)",
            self.inst_store,
            (self.inst_store as f64 / total_inst) * 100.0
        );
        println!(
            "  op.branch              {} ({:.2}%)",
            self.inst_branch,
            (self.inst_branch as f64 / total_inst) * 100.0
        );
        println!(
            "  op.system              {} ({:.2}%)",
            self.inst_system,
            (self.inst_system as f64 / total_inst) * 100.0
        );
        println!(
            "  op.fp_arith            {} ({:.2}%)",
            self.inst_fp_arith,
            (self.inst_fp_arith as f64 / total_inst) * 100.0
        );
        println!("----------------------------------------------------------");
        println!("BRANCH PREDICTION");
        let bp_total = self.branch_predictions;
        let bp_miss = self.branch_mispredictions;
        let bp_acc = if bp_total > 0 {
            100.0 * (1.0 - (bp_miss as f64 / bp_total as f64))
        } else {
            0.0
        };
        println!("  bp.lookups             {}", bp_total);
        println!("  bp.mispredicts         {}", bp_miss);
        println!("  bp.accuracy            {:.2}%", bp_acc);
        println!("----------------------------------------------------------");
        println!("MEMORY HIERARCHY");

        let print_cache = |name: &str, hits: u64, misses: u64| {
            let total = hits + misses;
            let rate = if total > 0 {
                (hits as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            println!(
                "  {:<6} accesses: {:<10} | hits: {:<10} | miss_rate: {:.2}%",
                name,
                total,
                hits,
                100.0 - rate
            );
        };

        print_cache("L1-I", self.icache_hits, self.icache_misses);
        print_cache("L1-D", self.dcache_hits, self.dcache_misses);
        print_cache("L2", self.l2_hits, self.l2_misses);
        print_cache("L3", self.l3_hits, self.l3_misses);
        println!("==========================================================");
    }
}
