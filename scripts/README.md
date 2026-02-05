# Scripts

Configuration is **Python-first**: the library provides only a base config; you define machines in **machine directories** (e.g. `scripts/p550/`, `scripts/m1/`).

## Directory Structure

- **p550/**: P550-style machine (3-wide, 32KB L1, 256KB L2).
  - `config.py`: The machine definition (edit this).
  - `run.py`: Runner script.
- **m1/**: M1-style machine (4-wide, 128KB L1, 4MB L2).
  - `config.py`: The machine definition.
  - `run.py`: Runner script.
- **setup/**: Setup scripts.
  - `boot_linux.py`: Downloads Buildroot, builds Linux, and boots it.
- **tests/**: Tests and comparisons.
  - `compare_p550_m1.py`: Runs a benchmark on both configs and compares stats.
  - `smoke_test.py`: Minimal check.

---

## Usage

**Run a machine script:**
```bash
./target/release/sim script scripts/p550/run.py
./target/release/sim script scripts/m1/run.py [binary]
```

**Run a comparison:**
```bash
./target/release/sim script scripts/tests/compare_p550_m1.py
```

**Boot Linux:**
```bash
./target/release/sim script scripts/setup/boot_linux.py
```

---

## Python API

You can import configs from the machine packages if `scripts/` is on your PYTHONPATH (or if running via `sim script` which adds it).

```python
from p550.config import p550_config
from m1.config import m1_config

# Use with Environment
env = Environment(binary="...", config=p550_config())
```
