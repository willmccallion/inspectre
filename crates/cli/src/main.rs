//! RISC-V cycle-accurate simulator CLI.

use clap::Parser;
use pyo3::prelude::*;
use pyo3::types::PyList;
use std::ffi::CString;
use std::io::Write;
use std::{fs, process};

use inspectre::config::Config;
use inspectre::core::Cpu;
use inspectre::sim::loader;
use inspectre::soc::System;

#[derive(Parser, Debug)]
#[command(
    name = "inspectre",
    author,
    version,
    about = "RISC-V cycle-accurate simulator",
    long_about = None,
)]
struct Cli {
    /// Bare-metal binary to execute.
    #[arg(short = 'f', long, conflicts_with_all = ["kernel", "script"])]
    file: Option<String>,

    /// Kernel image for OS boot.
    #[arg(long, conflicts_with_all = ["file", "script"])]
    kernel: Option<String>,

    /// Disk image for OS boot (requires --kernel).
    #[arg(long, requires = "kernel")]
    disk: Option<String>,

    /// Device tree blob for OS boot (requires --kernel).
    #[arg(long, requires = "kernel")]
    dtb: Option<String>,

    /// Python script to run (gem5-style).
    #[arg(long, conflicts_with_all = ["file", "kernel"])]
    script: Option<String>,

    /// Arguments passed to the script as sys.argv[1:].
    #[arg(
        allow_hyphen_values = true,
        trailing_var_arg = true,
        requires = "script"
    )]
    script_args: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    if let Some(script) = cli.script {
        run_python_script(&script, cli.script_args);
    } else if let Some(kernel) = cli.kernel {
        cmd_kernel(kernel, cli.disk.unwrap_or_default(), cli.dtb);
    } else if let Some(file) = cli.file {
        cmd_file(file);
    } else {
        eprintln!(
            "\n\x1b[1;31merror:\x1b[0m one of \x1b[1m--file\x1b[0m, \x1b[1m--kernel\x1b[0m, or \x1b[1m--script\x1b[0m is required\n"
        );
        eprintln!("\x1b[1;33mUsage:\x1b[0m");
        eprintln!(
            "  \x1b[1;36minspectre \x1b[0m \x1b[32m-f\x1b[0m <binary>                                   \x1b[2mBare-metal run\x1b[0m"
        );
        eprintln!(
            "  \x1b[1;36minspectre \x1b[0m \x1b[32m--kernel\x1b[0m <Image> [\x1b[32m--disk\x1b[0m img] [\x1b[32m--dtb\x1b[0m dtb]     \x1b[2mOS boot\x1b[0m"
        );
        eprintln!(
            "  \x1b[1;36minspectre \x1b[0m \x1b[32m--script\x1b[0m <script.py> [args...]                \x1b[2mPython script\x1b[0m"
        );
        eprintln!();
        eprintln!("Run \x1b[1minspectre --help\x1b[0m for full usage information.");
        process::exit(1);
    }
}

fn cmd_file(bin_path: String) {
    let config = Config::default();
    let system = System::new(&config, "");
    let mut cpu = Cpu::new(system, &config);

    let bin_data = loader::load_binary(&bin_path);
    let load_addr = config.system.ram_base;
    cpu.bus.load_binary_at(&bin_data, load_addr);
    cpu.pc = load_addr;

    run_loop(cpu);
}

fn cmd_kernel(kernel_path: String, disk: String, dtb: Option<String>) {
    let config = Config::default();
    let system = System::new(&config, &disk);
    let mut cpu = Cpu::new(system, &config);

    loader::setup_kernel_load(&mut cpu, &config, &disk, dtb, Some(kernel_path));
    cpu.direct_mode = false;

    run_loop(cpu);
}

fn run_loop(mut cpu: Cpu) {
    loop {
        if let Err(e) = cpu.tick() {
            eprintln!("\n\x1b[1;31m[!] FATAL TRAP:\x1b[0m {}", e);
            cpu.dump_state();
            cpu.stats.print();
            process::exit(1);
        }
        if let Some(code) = cpu.take_exit() {
            println!("\n[*] Exit code {}", code);
            cpu.stats.print();
            std::io::stdout().flush().ok();
            process::exit(code as i32);
        }
    }
}

fn run_python_script(script_path: &str, script_args: Vec<String>) {
    let script_content = fs::read_to_string(script_path).unwrap_or_else(|e| {
        eprintln!("Error reading script {}: {}", script_path, e);
        process::exit(1);
    });

    Python::with_gil(|py| {
        let sys = py.import("sys").expect("sys");
        let path = sys.getattr("path").expect("path");
        // Add repo root so the pure-Python `inspectre/` package is importable.
        path.call_method1("insert", (0, ".")).expect("path.insert");

        // Inject the compiled Rust extension as `inspectre._core` so that
        // `inspectre/__init__.py` can re-export it without a circular import.
        let m = PyModule::new(py, "inspectre._core").expect("module");
        _core::register_emulator_module(&m).expect("register");
        let modules = sys.getattr("modules").expect("modules");
        modules.set_item("inspectre._core", m).expect("inject");

        let mut full_args = vec![script_path.to_string()];
        full_args.extend(script_args);
        let py_args = PyList::new(py, &full_args).expect("argv");
        sys.setattr("argv", py_args).expect("argv");

        let code_c = CString::new(script_content).expect("code");
        let file_c = CString::new(script_path).expect("file");
        let name_c = CString::new("__main__").unwrap();

        let result = PyModule::from_code(py, &code_c, &file_c, &name_c);
        if let Err(e) = result {
            e.print(py);
            process::exit(1);
        }
    });
}
