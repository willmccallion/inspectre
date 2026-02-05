# Contributing

Thank you for your interest in contributing to the RISC-V System Simulator!

## Development Flow

1. **Rust Core:** Located in `hardware/`. Use `cargo test` to run the architecture and unit tests.
2. **Bindings:** Located in `bindings/`. Built automatically with the CLI.
3. **Python:** Located in `python/`. Use `pytest` (if configured) or run `scripts/tests/smoke_test.py`.

## Coding Standards

- **Rust:** Follow standard `cargo fmt` and `clippy` guidelines.
- **Python:** Use type hints and follow PEP 8.
- **Documentation:** Update relevant `.md` files in `docs/` when adding features.

## Adding a New Instruction

1. Define the opcode/funct in `hardware/src/isa/<extension>/`.
2. Add the decoding logic in `hardware/src/isa/decode.rs`.
3. Implement execution logic in `hardware/src/core/units/alu.rs` or relevant unit.
4. Add an assembly or C test case in `hardware/tests/`.

## Adding a New Device

1. Implement the `Device` trait in `hardware/src/soc/devices/`.
2. Add the device to the SoC builder in `hardware/src/soc/builder.rs`.
3. (Optional) Create Python bindings in `bindings/src/devices/`.
