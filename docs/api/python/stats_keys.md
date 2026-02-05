# Statistics Reference

This document lists the available statistics keys exposed to Python via the `StatsObject` (returned by `run_experiment()` or `Simulator.run()`).

## Common Metrics

- **`cycles`**: Total clock cycles elapsed.
- **`instructions_retired`**: Total instructions that successfully committed/retired.
- **`ipc`**: Instructions Per Cycle (`instructions_retired / cycles`).

## Cache Statistics

| Key | Description |
|-----|-------------|
| **`icache_hits`** | Instruction cache hits. |
| **`icache_misses`** | Instruction cache misses. |
| **`dcache_hits`** | Data cache hits. |
| **`dcache_misses`** | Data cache misses. |
| **`l2_hits`** | L2 cache hits. |
| **`l2_misses`** | L2 cache misses. |
| **`l3_hits`** | L3 cache hits. |
| **`l3_misses`** | L3 cache misses. |

## Branch Prediction

- **`branch_predictions`**: Total branches encountered.
- **`branch_mispredictions`**: Total branch mispredictions.
- **`branch_accuracy_pct`**: Branch prediction accuracy percentage.

## Pipeline Stalls

- **`stalls_mem`**: Cycles stalled waiting for memory.
- **`stalls_control`**: Cycles stalled due to control hazards (branches/jumps).
- **`stalls_data`**: Cycles stalled due to data hazards (RAW/WAW/WAR).

## Instruction Mix

- **`inst_alu`**: Integer arithmetic instructions.
- **`inst_load`**: Integer load instructions.
- **`inst_store`**: Integer store instructions.
- **`inst_branch`**: Branch/jump instructions.
- **`inst_system`**: System/CSR instructions.
- **`inst_fp_load`**: Floating-point load instructions.
- **`inst_fp_store`**: Floating-point store instructions.
- **`inst_fp_arith`**: Floating-point arithmetic instructions.
- **`inst_fp_fma`**: Floating-point fused multiply-add.
- **`inst_fp_div_sqrt`**: Floating-point divide/square-root.

## Execution Mode

- **`cycles_user`**: Cycles spent in User mode.
- **`cycles_kernel`**: Cycles spent in Supervisor/Kernel mode.
- **`cycles_machine`**: Cycles spent in Machine mode.
- **`traps_taken`**: Total traps/exceptions handled.

---

## Filtering Stats

Use **`.query(pattern)`** in Python to filter these keys:

```python
# Show only cache misses
print(result.stats.query("miss"))

# Show instruction mix
print(result.stats.query("^inst_"))
```
