# RISC-V 64-bit System Simulator

This project is a cycle-accurate system simulator for the RISC-V 64-bit architecture (RV64IMAFD). It implements a 5-stage pipelined CPU, a comprehensive memory hierarchy, and a custom microkernel to demonstrate end-to-end execution of user-space applications. Linux still fails to completely boot but is close.

## Technologies Used

* **Languages:** Rust (Simulator), C (Kernel/Userland), RISC-V Assembly, Python (Analysis)
* **Concepts:** Pipelining, Virtual Memory (SV39), Cache Coherence, Branch Prediction, OS Development
* **Tools:** Make, GCC Cross-Compiler, Cargo

## Key Implementation Details

### CPU Core (Rust)

* **5-Stage Pipeline:** Implements Fetch, Decode, Execute, Memory, and Writeback stages with full data forwarding and hazard detection.
* **Branch Prediction:** Features multiple swappable predictors including Static, GShare, Tournament, Perceptron, and TAGE (Tagged Geometric History).
* **Floating Point:** Support for single and double-precision floating-point arithmetic (F/D extensions).

### Memory System

* **Memory Management Unit (MMU):** Implements SV39 virtual addressing with translation lookaside buffers (iTLB and dTLB).
* **Cache Hierarchy:** Configurable L1, L2, and L3 caches supporting LRU, PLRU, and Random replacement policies.
* **DRAM Controller:** Simulates timing constraints including row-buffer conflicts, CAS/RAS latency, and precharge penalties.

### System Software (C & Assembly)

* **Microkernel:** Custom kernel handling boot sequences, physical memory allocation, context switching, and syscalls.
* **Libc:** A minimal standard library written from scratch (includes `printf`, `malloc`, string manipulation).
* **User Applications:** Includes a chess engine, raytracer, matrix multiplication, and quicksort algorithms ported to run on the simulator.

### Performance Analysis

* **Automated Benchmarking:** Python scripts to sweep hardware parameters (e.g., cache size vs. IPC) and visualize bottlenecks.
* **Design Space Exploration:** Includes a genetic algorithm script to evolve hardware configurations for optimal performance on specific workloads.

## Project Structure

* `hardware/`: The CPU simulator source code (Rust).
* `software/kernel/`: Microkernel source code (C).
* `software/libc/`: Custom C standard library implementation.
* `software/user/`: User-space applications (Raytracer, Chess, etc.).
* `scripts/`: Python tools for performance analysis and benchmarking.

## Build and Run

Requires Rust and the `riscv64-unknown-elf-gcc` toolchain.

**Build:** From repo root (builds simulator and `sim` CLI)
```bash
make all
# or: cargo build --release
```

**Run a binary (bare-metal):**
```bash
./target/release/sim run -f software/bin/benchmarks/qsort.bin
```

**Run Python scripts (P550 vs M1 comparison, stats via `.query()`):**
```bash
./target/release/sim script scripts/p550/run.py
./target/release/sim script scripts/tests/compare_p550_m1.py
```
See **scripts/README.md** for script options. Full documentation: **[docs/](docs/README.md)** (architecture, API, getting started).

## License

This project is licensed under the MIT License — see [LICENSE](LICENSE).
