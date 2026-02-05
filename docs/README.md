# RISC-V System Simulator Documentation

Modular documentation for the hybrid Rust/Python RISC-V architecture simulator.

---

## Getting Started

- [Installation](getting_started/installation.md) — Build the Rust core and set up Python.
- [Quickstart](getting_started/quickstart.md) — First runs: binaries, scripts, and Linux boot.

## Architecture (How it works)

- [Pipeline](architecture/pipeline.md) — 5-stage pipeline, latches, and hazards.
- [Branch prediction](architecture/branch_prediction.md) — TAGE, Perceptron, Tournament, BTB, RAS.
- [Memory hierarchy](architecture/memory_hierarchy.md) — Caches, replacement policies, MMU/TLB, PTW, prefetchers.
- [ISA support](architecture/isa_support.md) — RV64I/M/A/F/D/C and privileged extensions.

## API Reference

### Rust (Internal Core)
- [Hardware crates](api/rust/hardware_crates.md) — Overview of `hardware/src`.
- [Bindings](api/rust/bindings.md) — PyO3 Rust-to-Python bridge.
- [SoC integration](api/rust/soc_integration.md) — Bus, memory controller, and devices.

### Python (User Interface)
- [Simulation objects](api/python/simulation_objects.md) — CPU models, `Simulator`, `run_experiment`.
- [Configuration](api/python/configuration.md) — `SimConfig`, params, and predictor/cache settings.
- [Scripting](api/python/scripting.md) — Using the scripts in `scripts/`.

---

## Project Structure

- [Contributing](CONTRIBUTING.md) — Guidelines for developers.

```
.
├── hardware/     # Rust Core/Hardware (Simulator)
├── bindings/     # Rust-to-Python bridge (PyO3)
├── cli/          # Rust CLI (sim)
├── python/       # Python Interface and Library
├── scripts/      # Machine configs and run scripts
├── software/     # RISC-V software (benchmarks, kernel, userland)
└── docs/         # This documentation
```
