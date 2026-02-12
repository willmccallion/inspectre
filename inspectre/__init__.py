"""
Inspectre simulator Python API.

This package provides a Python-first interface to the cycle-accurate RISC-V simulator. It provides:
1. **Configuration:** `SimConfig` and `config_to_dict` for building machine models (cache, BP, pipeline).
2. **Execution:** `System`, `Cpu`, `simulate`, and `Simulator` for running binaries and scripts.
3. **Experiments:** `Environment`, `ExperimentResult`, and `run_experiment` for reproducible sweeps.
4. **Statistics:** `StatsObject` for performance metrics and sectioned output.
5. **Rust bindings:** `PySystem`, `CPU` (PyCpu), and `Memory` from the inspectre extension.
"""

from importlib.metadata import version as _metadata_version

from . import _core
from .config import SimConfig, config_to_dict
from .objects import System, Cpu, simulate, get_default_config, Simulator
from .experiment import Environment, ExperimentResult, run_experiment
from .stats import StatsObject

PySystem = _core.PySystem
CPU = _core.PyCpu
Memory = _core.PyMemory

__version__ = _metadata_version("inspectre-sim")


def version() -> str:
    """Return the installed inspectre version string."""
    return __version__


__all__ = [
    "__version__",
    "version",
    "SimConfig",
    "config_to_dict",
    "System",
    "Cpu",
    "simulate",
    "get_default_config",
    "Environment",
    "ExperimentResult",
    "run_experiment",
    "StatsObject",
    "PySystem",
    "CPU",
    "Memory",
    "Simulator",
]
