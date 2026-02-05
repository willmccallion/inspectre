# Memory Hierarchy

This document describes the cache subsystem, replacement policies, MMU, TLB, and Page Table Walker in the simulator.

**Source:** `hardware/src/core/units/cache/`, `hardware/src/core/units/mmu/`, `hardware/src/core/units/prefetch/`

---

## Overview

The memory hierarchy includes instruction and data caches with configurable size, associativity, and line size; replacement policies (LRU, PLRU, FIFO, MRU, Random); MMU with TLB and Page Table Walker; and prefetchers (NextLine, Stride, Stream, Tagged). Configuration is driven from Python `SimConfig` (cache sizes, policies, prefetcher type, TLB size). See [configuration](../api/python/configuration.md).

---

## Cache Structure

**Path:** `hardware/src/core/units/cache/`

- **`mod.rs`:** Cache logic (lookup, fill, eviction). Caches are split into L1-I, L1-D, and L2 (and optionally L3) as configured.
- **Parameters (from config):** `enabled`, `size_bytes`, `line_bytes`, `ways`, `policy`, `latency`, `prefetcher`, `prefetch_table_size`, `prefetch_degree`.

---

## Replacement Policies

**Path:** `hardware/src/core/units/cache/policies/`

| Policy  | File        | Description |
|---------|-------------|-------------|
| **LRU** | `lru.rs`    | Least Recently Used. |
| **PLRU**| `plru.rs`   | Pseudo-LRU (tree or bit-based). |
| **FIFO**| `fifo.rs`   | First In, First Out per set. |
| **MRU** | `mru.rs`    | Most Recently Used. |
| **Random**| `random.rs`| Random replacement. |

Python `CacheConfig.policy` accepts: `"LRU"`, `"PLRU"`, `"FIFO"`, `"Random"`, `"MRU"`. The cache module uses the selected policy to choose a victim on eviction.

---

## MMU and TLB

**Path:** `hardware/src/core/units/mmu/`

- **`tlb.rs`:** Translates virtual addresses to physical. TLB size is `config.memory.tlb_size`.
- **`ptw.rs`:** Page Table Walker for TLB misses (e.g., Sv39 page tables).
- **`mod.rs`:** Orchestrates TLB lookup and PTW on miss.

---

## Prefetchers

**Path:** `hardware/src/core/units/prefetch/`

| Prefetcher | File           | Use |
|------------|----------------|-----|
| NextLine   | `next_line.rs` | Prefetch the next line after a miss. |
| Stride     | `stride.rs`    | Stride detection and prefetch ahead. |
| Stream     | `stream.rs`    | Stream buffer for sequential patterns. |
| Tagged     | `tagged.rs`    | Tagged prefetcher. |

Python `CacheConfig.prefetcher` accepts: `"None"`, `"NextLine"`, `"Stride"`, `"Stream"`, `"Tagged"`. `prefetch_degree` and `prefetch_table_size` configure behavior where applicable.

---

## Data Path

- **Fetch:** PC → I-cache (and I-TLB if MMU enabled) → instruction.
- **Load/Store:** Effective address → D-TLB → D-cache / memory; PTW runs on TLB miss.

The SoC memory controller and bus are in [soc_integration](../api/rust/soc_integration.md).

---

## See also

- [Pipeline](pipeline.md) — fetch and memory stages.
- [API: Python configuration](../api/python/configuration.md) — cache and MMU parameters.
- [SOC integration](../api/rust/soc_integration.md) — memory controller and bus.
