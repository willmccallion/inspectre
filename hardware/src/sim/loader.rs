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

pub fn setup_kernel_load(
    cpu: &mut Cpu,
    config: &Config,
    disk_path: &str,
    dtb_path: Option<String>,
) {
    let ram_base = config.system.ram_base_val();
    let kernel_addr = ram_base + config.system.kernel_offset;

    // Load Kernel
    if !disk_path.is_empty() {
        let kernel_data = load_binary(disk_path);
        println!(
            "[Loader] Loading kernel ({} bytes) to RAM @ {:#x}",
            kernel_data.len(),
            kernel_addr
        );
        cpu.bus.load_binary_at(&kernel_data, kernel_addr);
    }

    // Load DTB
    let mut dtb_addr = 0;
    if let Some(path) = dtb_path {
        let dtb_data = load_binary(&path);
        // Put DTB near top of RAM or after kernel.
        // A safe bet is usually 0x87000000 or similar, but let's put it at RAM_BASE + 0x2000000 (32MB)
        dtb_addr = ram_base + 0x2000000;
        println!(
            "[Loader] Loading DTB ({} bytes) to RAM @ {:#x}",
            dtb_data.len(),
            dtb_addr
        );
        cpu.bus.load_binary_at(&dtb_data, dtb_addr);
    }

    cpu.pc = ram_base;
    cpu.privilege = 3;

    // MRET trampoline
    cpu.bus
        .load_binary_at(&sys_ops::MRET.to_le_bytes(), ram_base);

    cpu.csr_write(csr::MEPC, kernel_addr);

    // MSTATUS: MPP=S-mode (1), MPIE=1
    let mstatus_val = (1 << 11) | csr::MSTATUS_MPIE | csr::MSTATUS_FS_INIT;
    cpu.csr_write(csr::MSTATUS, mstatus_val);

    // Delegation
    cpu.csr_write(csr::MEDELEG, 0xFFFF_FFFF_FFFF_FFFF);
    cpu.csr_write(csr::MIDELEG, 0xFFFF_FFFF_FFFF_FFFF);
    cpu.csr_write(csr::SATP, 0);

    // Boot Registers
    cpu.regs.write(abi::REG_A0, 0); // Hart ID
    cpu.regs.write(abi::REG_A1, dtb_addr); // DTB Address

    println!(
        "[Loader] CPU starting in M-Mode -> MRET -> Kernel @ {:#x} (DTB: {:#x})",
        kernel_addr, dtb_addr
    );
}
