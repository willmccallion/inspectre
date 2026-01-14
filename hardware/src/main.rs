use clap::Parser;
use std::{fs, process};

mod config;
mod core;
mod isa;
mod sim;
mod stats;
mod system;

use crate::config::Config;
use crate::core::Cpu;
use crate::isa::abi;
use crate::sim::loader;
use crate::system::System;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "hardware/configs/default.toml")]
    config: String,

    #[arg(short, long, default_value = "software/Image")] // Default to Linux Image
    disk: String,

    #[arg(short, long)]
    file: Option<String>,

    #[arg(long)]
    dtb: Option<String>,
}

fn main() {
    let args = Args::parse();
    let config_content = fs::read_to_string(&args.config).expect("Failed to read config");
    let config: Config = toml::from_str(&config_content).expect("Failed to parse config");

    let disk_path = if args.file.is_some() { "" } else { &args.disk };

    let system = System::new(&config, disk_path);
    let mut cpu = Cpu::new(system, &config);

    if let Some(bin_path) = args.file {
        println!("[*] Direct Execution Mode");
        let bin_data = loader::load_binary(&bin_path);
        let load_addr = config.system.ram_base_val();

        cpu.bus.load_binary_at(&bin_data, load_addr);
        cpu.pc = load_addr;

        let stack_top = load_addr.wrapping_add(config.general.user_stack_size as u64);
        cpu.regs.write(abi::REG_SP, stack_top);

        cpu.direct_mode = true;
        cpu.privilege = 0; // User mode
    } else {
        println!("[*] Full System Mode");
        loader::setup_kernel_load(&mut cpu, &config, disk_path, args.dtb);
    }

    loop {
        if let Err(e) = cpu.tick() {
            eprintln!("\n[!] FATAL TRAP: {}", e);
            cpu.dump_state();
            cpu.stats.print();
            process::exit(1);
        }

        if let Some(code) = cpu.take_exit() {
            println!("\n[*] Exiting with code {}", code);
            cpu.stats.print();
            process::exit(code as i32);
        }
    }
}
