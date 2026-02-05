# Architecture (How It Works)

This section describes the internal design of the RISC-V simulator: pipeline, branch prediction, memory hierarchy, and ISA support.

| Document | Description |
|----------|-------------|
| [Pipeline](pipeline.md) | 5-stage pipeline (fetch, decode, execute, memory, writeback) and supporting modules. |
| [Branch prediction](branch_prediction.md) | BTB, TAGE, Perceptron, GShare, Tournament, RAS. |
| [Memory hierarchy](memory_hierarchy.md) | Caches, replacement policies (LRU, PLRU, FIFO, MRU, Random), MMU, TLB, PTW, prefetchers. |
| [ISA support](isa_support.md) | RV64I, M, A, F, D, C, and privileged extensions. |

Source locations: `hardware/src/core/` (pipeline, units), `hardware/src/isa/`.
