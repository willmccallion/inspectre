//! CPU Python binding.
//!
//! Exposes the simulator CPU to Python: create from config dict, tick, run until exit,
//! load kernel, and retrieve stats. Handles Python signal checks and stdout flush for UART visibility.

use crate::conversion::py_dict_to_config;
use crate::stats::PyStats;
use crate::system::PySystem;
use inspectre::core::Cpu;
use inspectre::core::arch::mode::PrivilegeMode;
use inspectre::sim::loader;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::io::Write;

/// Python-exposed CPU: wraps the core `Cpu` for stepping and running from Python.
#[pyclass]
pub struct PyCpu {
    pub inner: Cpu,
}

#[pymethods]
impl PyCpu {
    /// Creates a new CPU instance.
    ///
    /// This method initializes a CPU by taking ownership of the underlying system
    /// and applying the provided configuration.
    ///
    /// # Arguments
    /// * `py` - Python interpreter token.
    /// * `system` - The system instance to be associated with this CPU. Note that this
    ///   consumes the internal state of the system.
    /// * `config_dict` - A Python dictionary containing configuration parameters.
    ///
    /// # Errors
    /// Returns a `PyRuntimeError` if the system instance has already been attached to a CPU.
    #[new]
    fn new(py: Python, system: &mut PySystem, config_dict: &Bound<'_, PyAny>) -> PyResult<Self> {
        let sys = system.inner.take().ok_or_else(|| {
            PyRuntimeError::new_err("System instance has already been attached to a CPU")
        })?;

        let config = py_dict_to_config(py, config_dict)?;

        let cpu = Cpu::new(sys, &config);

        Ok(PyCpu { inner: cpu })
    }

    /// Loads a kernel into memory and prepares the CPU for execution.
    ///
    /// This method sets up the kernel image, applies the provided configuration,
    /// and optionally loads a Device Tree Blob (DTB). It also disables direct mode
    /// on the internal CPU state.
    ///
    /// # Arguments
    /// * `py` - Python interpreter token.
    /// * `kernel_path` - The file path to the kernel image to be loaded.
    /// * `config_dict` - A Python dictionary containing the system configuration.
    /// * `dtb_path` - An optional file path to the Device Tree Blob.
    ///
    /// # Errors
    /// Returns a `PyResult` error if the configuration dictionary cannot be parsed.
    #[pyo3(signature = (kernel_path, config_dict, dtb_path=None))]
    pub fn load_kernel(
        &mut self,
        py: Python,
        kernel_path: String,
        config_dict: &Bound<'_, PyAny>,
        dtb_path: Option<String>,
    ) -> PyResult<()> {
        let config = py_dict_to_config(py, config_dict)?;

        loader::setup_kernel_load(&mut self.inner, &config, "", dtb_path, Some(kernel_path));
        self.inner.direct_mode = false;
        Ok(())
    }

    /// Executes a single CPU cycle.
    ///
    /// This method advances the internal state of the CPU by one tick.
    ///
    /// # Errors
    ///
    /// Returns a `PyRuntimeError` if the underlying CPU operation fails.
    pub fn tick(&mut self) -> PyResult<()> {
        self.inner.tick().map_err(PyRuntimeError::new_err)
    }

    /// Returns a snapshot of the current CPU statistics.
    ///
    /// This method clones the internal statistics and converts them into a [`PyStats`]
    /// object, typically for exposure to Python.
    pub fn get_stats(&self) -> PyStats {
        PyStats::from(self.inner.stats.clone())
    }

    /// Returns the current value of the program counter (PC).
    pub fn get_pc(&self) -> u64 {
        self.inner.pc
    }

    /// Runs the simulation until the program exits (e.g., via SysCon power-off) or until the optional cycle limit is reached.
    ///
    /// Periodically checks for Python signals (e.g., Ctrl-C) and flushes stdout so UART
    /// output is visible when invoked from Python.
    ///
    /// # Arguments
    /// * `limit` - Optional maximum number of cycles to run. If None, runs until program exits.
    ///
    /// # Returns
    ///
    /// The exit code returned by the simulated program if it exited, or None if the cycle limit was reached.
    #[pyo3(signature = (limit=None))]
    pub fn run(&mut self, py: Python, limit: Option<u64>) -> PyResult<Option<u64>> {
        let start_cycles = self.inner.stats.cycles;
        loop {
            // Check if we've hit the cycle limit (if specified)
            if let Some(max_cycles) = limit
                && self.inner.stats.cycles - start_cycles >= max_cycles
            {
                let _ = std::io::stdout().flush();
                return Ok(None);
            }

            if self.inner.stats.cycles.is_multiple_of(10000) {
                py.check_signals()?;
                let _ = std::io::stdout().flush();
            }

            match self.inner.tick() {
                Ok(_) => {
                    if let Some(code) = self.inner.take_exit() {
                        let _ = std::io::stdout().flush();
                        return Ok(Some(code));
                    }
                }
                Err(e) => return Err(PyRuntimeError::new_err(e)),
            }
        }
    }

    /// Enable or disable direct (bare-metal) mode. When enabled, traps cause exit instead of jumping to trap handler.
    pub fn set_direct_mode(&mut self, enabled: bool) {
        self.inner.direct_mode = enabled;
        if enabled {
            self.inner.privilege = PrivilegeMode::User;
        }
    }

    /// Set the program counter.
    pub fn set_pc(&mut self, pc: u64) {
        self.inner.pc = pc;
    }

    /// Write a general-purpose register (0–31). x0 is read-only and ignored.
    pub fn write_register(&mut self, reg: u8, value: u64) {
        if reg < 32 {
            self.inner.regs.write(reg as usize, value);
        }
    }

    /// Read a general-purpose register (0–31).
    pub fn read_register(&self, reg: u8) -> u64 {
        if reg < 32 {
            self.inner.regs.read(reg as usize)
        } else {
            0
        }
    }

    /// Read a 32-bit value from a physical memory address.
    pub fn read_memory_u32(&mut self, paddr: u64) -> u32 {
        self.inner.bus.bus.read_u32(paddr)
    }

    /// Read a 64-bit value from a physical memory address.
    pub fn read_memory_u64(&mut self, paddr: u64) -> u64 {
        self.inner.bus.bus.read_u64(paddr)
    }

    /// Read a CSR by name. Returns None if unknown.
    pub fn read_csr(&self, name: &str) -> Option<u64> {
        let c = &self.inner.csrs;
        match name {
            "mstatus" => Some(c.mstatus),
            "misa" => Some(c.misa),
            "mie" => Some(c.mie),
            "mip" => Some(c.mip),
            "mtvec" => Some(c.mtvec),
            "mepc" => Some(c.mepc),
            "mcause" => Some(c.mcause),
            "mtval" => Some(c.mtval),
            "medeleg" => Some(c.medeleg),
            "mideleg" => Some(c.mideleg),
            "sstatus" => Some(c.sstatus),
            "sie" => Some(c.sie),
            "sip" => Some(c.sip),
            "stvec" => Some(c.stvec),
            "sepc" => Some(c.sepc),
            "scause" => Some(c.scause),
            "stval" => Some(c.stval),
            "satp" => Some(c.satp),
            _ => None,
        }
    }

    /// Get the current privilege mode as a string ("M", "S", or "U").
    pub fn get_privilege(&self) -> &'static str {
        match self.inner.privilege {
            PrivilegeMode::Machine => "M",
            PrivilegeMode::Supervisor => "S",
            PrivilegeMode::User => "U",
        }
    }

    /// Enable or disable instruction tracing.
    pub fn set_trace(&mut self, enabled: bool) {
        self.inner.trace = enabled;
    }

    /// Return the last N committed (pc, instruction) pairs from the ring buffer.
    pub fn get_pc_trace(&self) -> Vec<(u64, u32)> {
        self.inner.pc_trace.clone()
    }
}
