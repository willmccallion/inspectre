#!/usr/bin/env python3
"""
Time-travel debugger for the Linux boot crash.

Assumes Linux artifacts already exist in software/linux/output/ (built by boot_linux.py).
Catches the instruction access fault at ~cycle 362,638,054 where PC and RA jump to
garbage address 0xf70ad87c52fed200.

Strategy:
  1. Fast-forward to just before the crash (~362,630,000 cycles).
  2. Single-step, recording a rolling trace of the last N instructions.
  3. When PC becomes garbage (leaves kernel address space), stop and dump
     the full register file plus the instruction trace leading up to it.

Run from repo root:

  sim script scripts/setup/debug_linux_boot.py
  sim script scripts/setup/debug_linux_boot.py --target 362630000 --window 16000
"""

import argparse
import collections
import os
import sys
import time


# ── RISC-V register ABI names ────────────────────────────────────────────────
REG_NAMES = [
    "zero", "ra",   "sp",   "gp",   "tp",   "t0",   "t1",   "t2",
    "s0",   "s1",   "a0",   "a1",   "a2",   "a3",   "a4",   "a5",
    "a6",   "a7",   "s2",   "s3",   "s4",   "s5",   "s6",   "s7",
    "s8",   "s9",   "s10",  "s11",  "t3",   "t4",   "t5",   "t6",
]

# Kernel virtual addresses live in the upper region for Sv39.
# Physical kernel starts at 0x80000000; virtual mapping is typically
# 0xffffffc000000000+.  Anything outside both ranges is "garbage".
PHYS_LO = 0x80000000
PHYS_HI = 0x90000000          # 256 MB window
VIRT_KERN_LO = 0xFFFFFF8000000000  # conservative lower bound for Sv39 upper half
VIRT_KERN_HI = 0xFFFFFFFFFFFFFFFF


def is_valid_kernel_pc(pc: int) -> bool:
    """Return True if pc looks like a legitimate kernel address."""
    return (PHYS_LO <= pc < PHYS_HI) or (VIRT_KERN_LO <= pc <= VIRT_KERN_HI)


def repo_root():
    return os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def dump_registers(cpu):
    """Print all 32 general-purpose registers."""
    print("\n╔══════════════════════════════════════════════════════════╗")
    print("║                   REGISTER DUMP                         ║")
    print("╠══════════════════════════════════════════════════════════╣")
    print(f"║  PC  = 0x{cpu.get_pc():016x}                          ║")
    print("╠══════════════════════════════════════════════════════════╣")
    for i in range(0, 32, 2):
        v0 = cpu.read_register(i)
        v1 = cpu.read_register(i + 1)
        n0 = REG_NAMES[i].ljust(4)
        n1 = REG_NAMES[i + 1].ljust(4)
        print(f"║  x{i:<2} ({n0}) = 0x{v0:016x}  "
              f"x{i+1:<2} ({n1}) = 0x{v1:016x} ║")
    print("╚══════════════════════════════════════════════════════════╝")


def dump_trace(trace):
    """Print the rolling instruction trace."""
    print(f"\n── Instruction trace ({len(trace)} entries, oldest first) ──")
    for i, entry in enumerate(trace):
        cyc, pc, ra, sp = entry
        marker = " <<<"  if not is_valid_kernel_pc(pc) else ""
        print(f"  [{i:>4}] cycle={cyc:>12}  pc=0x{pc:016x}  "
              f"ra=0x{ra:016x}  sp=0x{sp:016x}{marker}")


def optimized_config():
    """Same config used by boot_linux.py."""
    from riscv_sim import SimConfig

    c = SimConfig.default()

    c.general.trace_instructions = False
    c.general.start_pc = 0x80000000

    c.system.ram_base = 0x80000000
    c.system.uart_base = 0x10000000
    c.system.disk_base = 0x10001000
    c.system.clint_base = 0x02000000
    c.system.syscon_base = 0x00100000
    c.system.kernel_offset = 0x200000
    c.system.bus_width = 8
    c.system.bus_latency = 1
    c.system.clint_divider = 100

    c.memory.ram_size = 256 * 1024 * 1024
    c.memory.controller = "Simple"
    c.memory.row_miss_latency = 10
    c.memory.tlb_size = 64

    c.cache.l1_i.enabled = True
    c.cache.l1_i.size_bytes = 65536
    c.cache.l1_i.line_bytes = 64
    c.cache.l1_i.ways = 8
    c.cache.l1_i.policy = "PLRU"
    c.cache.l1_i.latency = 1
    c.cache.l1_i.prefetcher = "NextLine"
    c.cache.l1_i.prefetch_degree = 2

    c.cache.l1_d.enabled = True
    c.cache.l1_d.size_bytes = 65536
    c.cache.l1_d.line_bytes = 64
    c.cache.l1_d.ways = 8
    c.cache.l1_d.policy = "PLRU"
    c.cache.l1_d.latency = 1
    c.cache.l1_d.prefetcher = "Stride"
    c.cache.l1_d.prefetch_table_size = 128
    c.cache.l1_d.prefetch_degree = 2

    c.cache.l2.enabled = True
    c.cache.l2.size_bytes = 1048576
    c.cache.l2.line_bytes = 64
    c.cache.l2.ways = 16
    c.cache.l2.policy = "PLRU"
    c.cache.l2.latency = 8
    c.cache.l2.prefetcher = "NextLine"
    c.cache.l2.prefetch_degree = 1

    c.cache.l3.enabled = True
    c.cache.l3.size_bytes = 8 * 1024 * 1024
    c.cache.l3.line_bytes = 64
    c.cache.l3.ways = 16
    c.cache.l3.policy = "PLRU"
    c.cache.l3.latency = 28
    c.cache.l3.prefetcher = "None"

    c.pipeline.branch_predictor = "TAGE"
    c.pipeline.width = 1
    c.pipeline.btb_size = 4096
    c.pipeline.ras_size = 48

    c.pipeline.tage.num_banks = 4
    c.pipeline.tage.table_size = 2048
    c.pipeline.tage.loop_table_size = 256
    c.pipeline.tage.reset_interval = 2000
    c.pipeline.tage.history_lengths = [5, 15, 44, 130]
    c.pipeline.tage.tag_widths = [9, 9, 10, 10]

    return c


def main():
    root = repo_root()
    linux_dir = os.path.join(root, "software", "linux")
    out_dir = os.path.join(linux_dir, "output")
    image_path = os.path.join(out_dir, "Image")
    disk_path = os.path.join(out_dir, "disk.img")
    dtb_path = os.path.join(linux_dir, "system.dtb")

    ap = argparse.ArgumentParser(description="Time-travel debug the Linux boot crash")
    ap.add_argument(
        "--target", type=int, default=362_630_000,
        help="Cycle to fast-forward to before single-stepping (default: 362630000)",
    )
    ap.add_argument(
        "--window", type=int, default=16_000,
        help="Max cycles to single-step after fast-forward (default: 16000)",
    )
    ap.add_argument(
        "--trace-depth", type=int, default=64,
        help="Number of recent instructions to keep in rolling trace (default: 64)",
    )
    ap.add_argument(
        "--progress", type=int, default=1000,
        help="Print progress every N single-steps (default: 1000)",
    )
    args = ap.parse_args()

    # ── Verify artifacts ─────────────────────────────────────────────────
    for path, name in [(image_path, "Kernel Image"), (disk_path, "Disk image"),
                       (dtb_path, "Device tree blob")]:
        if not os.path.isfile(path):
            print(f"Error: {name} not found at {path}")
            print("Run boot_linux.py first to build Linux artifacts.")
            return 1

    # ── Import simulator ─────────────────────────────────────────────────
    sys.path.insert(0, os.path.join(root, "python"))
    from riscv_sim import SimConfig
    from riscv_sim.objects import System, P550Cpu

    os.chdir(root)

    # ── Build the CPU directly (same config as boot_linux.py) ────────────
    print("[debug] Building simulator with optimized config...")
    config = optimized_config()
    config.system.uart_to_stderr = True

    sys_obj = System(ram_size=config.memory.ram_size)
    sys_obj.instantiate(disk_image=disk_path, config=config)

    cpu_obj = P550Cpu(sys_obj, config=config)
    cpu = cpu_obj.create()

    print(f"[debug] Loading kernel: {image_path}")
    cpu_obj.load_kernel(image_path, dtb_path)

    # ── Phase 1: Fast-forward ────────────────────────────────────────────
    target = args.target
    print(f"[debug] Fast-forwarding to cycle {target:,} ...")
    t0 = time.time()
    result = cpu.run(limit=target)
    elapsed = time.time() - t0

    if result is not None:
        print(f"[debug] Simulation exited (code {result}) before reaching target cycle.")
        return result

    stats = cpu.get_stats()
    print(f"[debug] Reached cycle {stats.cycles:,} in {elapsed:.1f}s")
    print(f"[debug] PC = 0x{cpu.get_pc():016x}  RA = 0x{cpu.read_register(1):016x}")

    # ── Phase 2: Single-step with trace ──────────────────────────────────
    window = args.window
    trace_depth = args.trace_depth
    trace = collections.deque(maxlen=trace_depth)

    print(f"[debug] Single-stepping up to {window:,} cycles, trace depth = {trace_depth}")
    print(f"[debug] Looking for PC outside kernel address space...")

    caught = False
    for step in range(window):
        pc = cpu.get_pc()
        ra = cpu.read_register(1)   # x1 = ra
        sp = cpu.read_register(2)   # x2 = sp
        cycle = stats.cycles + step  # approximate

        trace.append((cycle, pc, ra, sp))

        # ── Trap condition: PC is garbage ────────────────────────────────
        if not is_valid_kernel_pc(pc):
            print(f"\n{'='*70}")
            print(f"  CAUGHT: PC left kernel address space at step {step}")
            print(f"  PC = 0x{pc:016x}   RA = 0x{ra:016x}")
            print(f"  Approx cycle = {cycle:,}")
            print(f"{'='*70}")

            dump_trace(trace)
            dump_registers(cpu)

            stats = cpu.get_stats()
            print(f"\n  Actual cycles: {stats.cycles:,}")
            print(f"  Instructions retired: {stats.instructions_retired:,}")
            caught = True
            break

        # ── Secondary trap: RA matches the known garbage value ───────────
        if ra == 0xf70ad87c52fed200 and pc != ra:
            print(f"\n{'='*70}")
            print(f"  CAUGHT: RA contains garbage at step {step}")
            print(f"  PC = 0x{pc:016x}   RA = 0x{ra:016x}")
            print(f"  The corrupted RA has been loaded but not jumped to yet.")
            print(f"  Approx cycle = {cycle:,}")
            print(f"{'='*70}")

            dump_trace(trace)
            dump_registers(cpu)

            stats = cpu.get_stats()
            print(f"\n  Actual cycles: {stats.cycles:,}")
            print(f"  Instructions retired: {stats.instructions_retired:,}")
            caught = True
            break

        # ── Step one cycle ───────────────────────────────────────────────
        result = cpu.run(limit=1)
        if result is not None:
            print(f"[debug] Simulation exited (code {result}) at step {step}")
            dump_trace(trace)
            dump_registers(cpu)
            return result

        if step > 0 and step % args.progress == 0:
            curr_pc = cpu.get_pc()
            print(f"  step {step:>6}/{window}  pc=0x{curr_pc:016x}")

    if not caught:
        print(f"\n[debug] Completed {window:,} steps without catching the crash.")
        print("[debug] The crash may occur later. Try increasing --target or --window.")
        print(f"[debug] Final PC = 0x{cpu.get_pc():016x}")
        stats = cpu.get_stats()
        print(f"[debug] Actual cycles: {stats.cycles:,}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
