"""
rvsim simulator Python API.

A Python-first interface to the cycle-accurate RISC-V simulator:
1. **Configuration:** ``Config``, ``Cache``, ``BranchPredictor``, etc.
2. **Execution:** ``System``, ``Cpu``, ``Simulator``.
3. **Experiments:** ``Environment``, ``Result``.
4. **Statistics:** ``Stats``, ``Table``.
5. **ISA:** ``reg``, ``csr``, ``Disassemble``.
"""

from importlib.metadata import version as _metadata_version

from .config import Config
from .experiment import Environment, Result
from .isa import Disassemble, csr, reg
from .objects import Cpu, Instruction, Simulator, System
from .stats import Stats, Table
from .types import (
    Backend,
    BranchPredictor,
    Cache,
    MemoryController,
    Prefetcher,
    ReplacementPolicy,
)

__version__ = _metadata_version("rvsim")


def version() -> str:
    """Return the installed rvsim version string."""
    return __version__


__all__ = [
    "__version__",
    "version",
    "Config",
    "BranchPredictor",
    "ReplacementPolicy",
    "Prefetcher",
    "MemoryController",
    "Backend",
    "Cache",
    "System",
    "Cpu",
    "Simulator",
    "Instruction",
    "Environment",
    "Result",
    "Stats",
    "Table",
    "reg",
    "csr",
    "Disassemble",
]
