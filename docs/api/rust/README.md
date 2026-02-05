# Rust API (Internal Core)

Documentation for the Rust simulator core and its exposure to Python.

| Document | Description |
|----------|-------------|
| [Hardware crates](hardware_crates.md) | Overview of `hardware/src` modules (config, core, isa, sim, soc, stats). |
| [Bindings](bindings.md) | PyO3 bindings: `cpu.rs`, `system.rs`, conversion, stats, devices. |
| [SoC integration](soc_integration.md) | Bus, memory controller, and devices (CLINT, PLIC, UART, VirtIO, etc.). |
