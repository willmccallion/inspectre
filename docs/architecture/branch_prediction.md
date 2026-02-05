# Branch Prediction

This document describes the branch prediction and related units in the simulator.

**Source:** `hardware/src/core/units/bru/`

---

## Overview

The Branch Resolution Unit (BRU) provides next-PC prediction for fetch and branch resolution in execute. All predictors implement the **BranchPredictor** trait (`branch_predictor.rs`) and are dispatched via **BranchPredictorWrapper** (static dispatch, no vtable in the fetch loop). Configuration selects the algorithm and BTB/RAS sizes; the wrapper is built from `Config` in `BranchPredictorWrapper::new(config)`.

Supported predictor types (from `Config` / Python): **Static**, **GShare**, **Tournament**, **TAGE**, **Perceptron**. Each uses a shared **BTB** and **RAS** for target and return-address prediction.

---

## Branch Predictor Trait (`branch_predictor.rs`)

- **`predict_branch(pc)`** → `(bool, Option<u64>)`: whether the branch is predicted taken and the predicted target (if taken).
- **`update_branch(pc, taken, target)`**: called after resolution to train the predictor and update BTB.
- **`predict_btb(pc)`** → `Option<u64>`: BTB-only target prediction.
- **`on_call(pc, ret_addr, target)`**: push return address onto RAS on call.
- **`predict_return()`** → `Option<u64>`: pop predicted return address from RAS.
- **`on_return()`**: pop RAS on return.

---

## Branch Target Buffer (`btb.rs`)

Stores predicted targets for branch and jump instructions, indexed by PC. All predictor implementations use the same BTB; size is configured via `config.pipeline.btb_size`.

---

## Predictors

### Static (`static_bp.rs`)

Static policy (e.g., always not-taken). Used as baseline and for debugging. BTB and RAS are still used for targets and returns.

### GShare (`gshare.rs`)

Global history XOR PC index into a pattern history table (2-bit counters). History length and table size are configurable.

### TAGE (`tage.rs`)

TAGE (TAgged Geometric History length): multiple tables with different history lengths; tag match selects the table. Configured via Python `TageConfig`: `num_banks`, `table_size`, `loop_table_size`, `reset_interval`, `history_lengths`, `tag_widths`. See [configuration](../api/python/configuration.md).

### Perceptron (`perceptron.rs`)

Perceptron-based predictor with weights and history. Configured via Python `PerceptronConfig`: `history_length`, `table_bits`.

### Tournament (`tournament.rs`)

Meta-predictor that selects between two component predictors (e.g., local vs global). Configured via Python `TournamentConfig`: `global_size_bits`, `local_hist_bits`, `local_pred_bits`.

### Return Address Stack (`ras.rs`)

Stack for return-address prediction. Pushed on call (`jal`/`jalr`), popped on predicted return. Depth is `config.pipeline.ras_size`.

---

## Pipeline Integration

- **Fetch:** Uses `predict_branch` (and BTB/RAS) to compute next PC.
- **Execute:** Resolves branch; calls `update_branch`, `on_call`, `on_return`; on misprediction triggers flush and redirect.
- **Config:** Branch predictor type and sizes come from Rust `Config`, which is built from Python `SimConfig` (see [bindings](../api/rust/bindings.md), [configuration](../api/python/configuration.md)).

---

## See also

- [Pipeline](pipeline.md) — fetch and execute stages.
- [API: Python configuration](../api/python/configuration.md) — `TageConfig`, `PerceptronConfig`, `TournamentConfig`, `btb_size`, `ras_size`.
