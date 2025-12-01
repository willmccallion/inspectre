use crate::config::Config;
use crate::core::Cpu;
use crate::isa::{abi, csr, sys_ops};
use std::fs;
use std::process;

pub fn load_binary(path: &str) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|e| {
        eprintln!("\n[!] FATAL: Could not read file '{}': {}", path, e);
        process::exit(1);
    })
}

pub fn setup_kernel_load(cpu: &mut Cpu, config: &Config, disk_path: &str) {
    if disk_path.is_empty() {
        return;
    }

    let ram_base = config.system.ram_base_val();
    let kernel_addr = ram_base + config.system.kernel_offset;

    let kernel_data = load_binary(disk_path);
    if !kernel_data.is_empty() {
        println!(
            "[Loader] Loading kernel ({} bytes) to RAM @ {:#x}",
            kernel_data.len(),
            kernel_addr
        );
        cpu.bus.load_binary_at(&kernel_data, kernel_addr);
    }

    cpu.pc = ram_base;
    cpu.privilege = 3;

    cpu.bus
        .load_binary_at(&sys_ops::MRET.to_le_bytes(), ram_base);

    cpu.csr_write(csr::MEPC, kernel_addr);

    let mstatus_val = (1 << 11) | csr::MSTATUS_MPIE | csr::MSTATUS_FS_INIT;
    cpu.csr_write(csr::MSTATUS, mstatus_val);
    cpu.csr_write(csr::MEDELEG, 0xFFFF_FFFF_FFFF_FFFF);
    cpu.csr_write(csr::MIDELEG, 0xFFFF_FFFF_FFFF_FFFF);
    cpu.csr_write(csr::SATP, 0);

    cpu.regs.write(abi::REG_A0, 0);
    cpu.regs.write(abi::REG_A1, 0);

    println!(
        "[Loader] Trampoline set. CPU starting in M-Mode -> MRET -> Kernel @ {:#x}",
        kernel_addr
    );
}
