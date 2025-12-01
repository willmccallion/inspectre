# RISC-V 64-bit System Simulator

This project is a cycle-accurate system simulator for the RISC-V 64-bit architecture (RV64IMAFD). It implements a 5-stage pipelined CPU, a comprehensive memory hierarchy, and a custom microkernel to demonstrate end-to-end execution of user-space applications.

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

## Build Instructions

This project requires Rust and the `riscv64-unknown-elf-gcc` toolchain.

**Build the System:**
Compiles the kernel, libc, user apps, creates the disk image, and builds the Rust simulator.
```bash
make all
```

**Run a Simulation:**
Execute a binary inside the simulator.
```bash
./sim chess
```

**Run Analysis:**
Generate performance reports across different hardware configurations.
```bash
python3 scripts/run_bench.py
```
