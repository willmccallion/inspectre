# ISA Support

This document lists the RISC-V instruction set extensions implemented in the simulator and their locations in the codebase.

**Source:** `hardware/src/isa/`

---

## Overview

| Extension   | Path              | Description |
|-------------|-------------------|-------------|
| **RV64I**   | `isa/rv64i/`      | Base 64-bit integer (opcodes, funct3, funct7). |
| **M**       | `isa/rv64m/`      | Integer multiply/divide. |
| **A**       | `isa/rv64a/`      | Atomics (LR/SC, AMO). |
| **F**       | `isa/rv64f/`      | Single-precision floating-point. |
| **D**       | `isa/rv64d/`      | Double-precision floating-point. |
| **C**       | `isa/rvc/`        | Compressed (16-bit) instructions; expanded before decode. |
| **Privileged** | `isa/privileged/` | Trap causes, system opcodes, CSRs. |

Decoding is centralized in **`isa/decode.rs`**; each extension provides opcodes and funct encodings. **`isa/instruction.rs`** defines the internal instruction representation used across the pipeline. **`isa/abi.rs`** provides ABI/register names for debugging.

---

## RV64I (`rv64i/`)

Opcodes and funct fields for arithmetic, loads/stores, branches, jumps, and system instructions. Implemented in `opcodes.rs`, `funct3.rs`, `funct7.rs`.

---

## RV64M (`rv64m/`)

Multiply and divide: MUL, MULH, MULHSU, MULHU, DIV, DIVU, REM, REMU (and 64-bit variants). Execution uses the ALU unit; divide-by-zero and overflow behavior follow the RISC-V spec.

---

## RV64A (`rv64a/`)

LR/SC and AMO (swap, add, and, or, xor, min/max). Fence and ordering. Implemented in `opcodes.rs`, `funct3.rs`, `funct5.rs`.

---

## RV64F / RV64D (`rv64f/`, `rv64d/`)

Single- and double-precision load/store, arithmetic, compare, convert, move, classify. Execution in **`core/units/fpu.rs`**. Rounding and NaN handling follow the spec; FCSR is used where implemented.

---

## RVC (`rvc/`)

16-bit compressed instructions. **`expand.rs`** expands them to 32-bit equivalents before they are fed into the main decoder. **`constants.rs`** holds C-specific encoding constants.

---

## Privileged (`privileged/`)

Trap causes (`cause.rs`), privilege levels, and system opcodes (e.g., ECALL, EBREAK, SRET, MRET, WFI; CSR access). CSRs and trap handling are in **`core/arch/csr.rs`**, **`core/arch/trap.rs`**, and **`core/arch/mode.rs`**.

---

## See also

- [Pipeline](pipeline.md) — where decoded instructions flow.
- [Hardware crates](../api/rust/hardware_crates.md) — layout of `hardware/src`.
