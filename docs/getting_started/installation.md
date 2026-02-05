# Installation

Build the Rust simulator core and set up the Python environment.

---

## Prerequisites

- **Rust:** Install [rustup](https://rustup.rs/); ensure `cargo` is on your PATH.
- **Python:** 3.8+ (3.10+ recommended). No extra libraries needed for basic run (uses standard library and PyO3).
- **RISC-V toolchain:** `riscv64-unknown-elf-gcc` to build software/benchmarks.
- **Build tools:** `make`, `dtc` (device tree compiler, for Linux boot).

---

## Building the Simulator

From the **repository root**:

```bash
cargo build --release
```

This builds the `hardware` crate, the `bindings` extension, and the `cli` binary: **`target/release/sim`**.

Alternatively, use the Makefile:
```bash
make hardware
```

---

## Python Setup

The CLI automatically adds the repo’s `python/` and script directories to `sys.path` when you use **`sim script`**. You do not need a system-wide installation.

For development or direct `python` execution:
```bash
export PYTHONPATH="${PYTHONPATH:-}:$(pwd)/python"
```

If you use a virtualenv, you can install in editable mode:
```bash
pip install -e ./python
```

---

## Verifying the Installation

1. **Help:** `./target/release/sim --help`
2. **Smoke Test:**
   ```bash
   make hardware software
   ./target/release/sim script scripts/tests/smoke_test.py
   ```
3. **Linux Toolchain:** `riscv64-unknown-elf-gcc --version`

---

## Next Steps

- [Quickstart](quickstart.md) — First runs.
- [Scripting](../api/python/scripting.md) — Running machine models.
