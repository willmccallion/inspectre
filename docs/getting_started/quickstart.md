# Quickstart

Build the simulator and run your first benchmark, script, and OS boot.

---

## 1. Build

```bash
cargo build --release
# and build software benchmarks
make software
```

The simulator executable is `./target/release/sim`.

---

## 2. Run a Bare-Metal Binary

Run a benchmark directly using the CLI:

```bash
./target/release/sim run -f software/bin/benchmarks/qsort.bin
```

This uses the [Rust core](../api/rust/hardware_crates.md) with a simple default in-order configuration.

---

## 3. Run a Machine Script

Run a benchmark on a pipelined, cache-accurate P550 model:

```bash
./target/release/sim script scripts/p550/run.py
```

Run with the M1 model:

```bash
./target/release/sim script scripts/m1/run.py
```

These scripts use **`run_experiment()`** and print IPC and query-based stats (e.g., `.query("miss")`). Edit **`scripts/p550/config.py`** to change the hardware model. See [scripting](../api/python/scripting.md).

---

## 4. Compare P550 vs M1

Run the same binary on both machines and see the difference in cycles, IPC, and misses:

```bash
./target/release/sim script scripts/tests/compare_p550_m1.py
```

---

## 5. Boot Linux

Build the full Linux stack (kernel, rootfs, OpenSBI, DTB) and boot it in the simulator:

```bash
./target/release/sim script scripts/setup/boot_linux.py
```

Or via Makefile:
```bash
make run-linux
```

The boot uses the M1 machine model and the [Simulator](../api/python/simulation_objects.md) API.

---

## See also

- [Installation](installation.md)
- [Scripting](../api/python/scripting.md)
- [Configuration](../api/python/configuration.md)
- [Architecture](../architecture/README.md)
