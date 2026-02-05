# Python API (User Interface)

Documentation for the Python simulation layer, configuration, and scripting.

| Document | Description |
|----------|-------------|
| [Simulation objects](simulation_objects.md) | CPU models (Base, Atomic, O3), `Simulator`, `Environment`, `run_experiment`. |
| [Configuration](configuration.md) | `SimConfig`, params, branch predictor and cache config; mapping to Rust. |
| [Stats Reference](stats_keys.md) | Detailed list of available statistics keys and filtering. |
| [Scripting](scripting.md) | Script layout, P550/M1 runs, `boot_linux.py`, comparison script, writing your own. |

All of these ultimately use the [Rust bindings](../rust/bindings.md).
