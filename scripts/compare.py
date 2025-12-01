#!/usr/bin/env python3
import subprocess
import os
import re
import math
import sys
import argparse

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(SCRIPT_DIR, ".."))

BIN_DIR = os.path.join(PROJECT_ROOT, "software/bin/tests")
CONFIG_DIR = os.path.join(PROJECT_ROOT, "hardware/configs")
CARGO_MANIFEST = os.path.join(PROJECT_ROOT, "hardware/Cargo.toml")

def resolve_config_path(name_or_path):
    """
    Tries to find the config file. Checks:
    1. Exact path provided.
    2. Inside hardware/configs/
    """
    if os.path.exists(name_or_path):
        return os.path.abspath(name_or_path)

    in_config_dir = os.path.join(CONFIG_DIR, name_or_path)
    if os.path.exists(in_config_dir):
        return in_config_dir

    print(f"Error: Configuration file '{name_or_path}' not found.")
    sys.exit(1)

def run_benchmarks(target_config_arg, baseline_config_arg):
    if not os.path.exists(BIN_DIR):
        print(f"Error: {BIN_DIR} does not exist.")
        sys.exit(1)

    baseline_path = resolve_config_path(baseline_config_arg)
    target_path = resolve_config_path(target_config_arg)

    baseline_name = os.path.basename(baseline_path)
    target_name = os.path.basename(target_path)

    if baseline_path == target_path:
        target_name = target_name + " (Target)"

    # List of (Display Name, Full Path)
    config_list = [
        (baseline_name, baseline_path),
        (target_name, target_path)
    ]

    tests = [f.replace(".bin", "") for f in os.listdir(BIN_DIR) if f.endswith(".bin")]
    tests.sort()

    results = {name: {} for name, _ in config_list}

    print(f"Found {len(tests)} benchmarks.")
    print(f"Baseline: {baseline_name}")
    print(f"Target:   {target_name}")
    print("-" * 60)

    for conf_name, conf_path in config_list:
        print(f"Running Config: {conf_name}")

        for test in tests:
            bin_path = os.path.join(BIN_DIR, test + ".bin")
            cmd = [
                "cargo", "run", "--quiet", "--release",
                "--manifest-path", CARGO_MANIFEST,
                "--", "--file", bin_path, "--config", conf_path
            ]

            try:
                proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=15)
                if proc.returncode != 0:
                    results[conf_name][test] = None
                    continue

                metrics = {}

                m_iso = re.search(r"Benchmark Cycles:\s+(\d+)", proc.stdout)
                m_tot = re.search(r"sim_cycles\s+(\d+)", proc.stdout)

                if m_iso:
                    metrics['cycles'] = int(m_iso.group(1))
                elif m_tot:
                    metrics['cycles'] = int(m_tot.group(1))
                else:
                    metrics['cycles'] = 0

                patterns = {
                    "cpi": r"sim_cpi\s+([\d\.]+)",
                    "bp_acc": r"bp\.accuracy\s+([\d\.]+)%",
                    "l1d_miss": r"L1-D\s+accesses.*miss_rate:\s+([\d\.]+)%",
                }

                for key, pat in patterns.items():
                    match = re.search(pat, proc.stdout)
                    if match:
                        metrics[key] = float(match.group(1))
                    else:
                        metrics[key] = 0.0

                if metrics['cycles'] > 0:
                    results[conf_name][test] = metrics
                else:
                    results[conf_name][test] = None

            except subprocess.TimeoutExpired:
                print(f"  [TIMEOUT] {test}")
                results[conf_name][test] = None

    return tests, [c[0] for c in config_list], results

def print_detailed_report(tests, configs, results):
    print("\n" + "="*100)
    print(f"{'DETAILED PERFORMANCE REPORT':^100}")
    print("="*100)

    baseline = configs[0]
    target = configs[1]

    print(f"{'Benchmark':<25} | {'Baseline Cyc':<12} | {'Target Cyc':<12} | {'Speedup':<8} | {'CPI (T)':<7} | {'L1D Miss (T)':<12}")
    print("-" * 100)

    for test in tests:
        base_m = results[baseline].get(test)
        targ_m = results[target].get(test)

        if not base_m or not targ_m:
            print(f"{test:<25} | {'ERR':<12} | {'ERR':<12} | {'-':<8} | {'-':<7} | {'-':<12}")
            continue

        base_cyc = base_m['cycles']
        targ_cyc = targ_m['cycles']

        speedup = 1.0
        if targ_cyc > 0:
            speedup = base_cyc / targ_cyc

        # Colorize Speedup
        sp_str = f"{speedup:.2f}x"

        print(f"{test:<25} | {base_cyc:<12} | {targ_cyc:<12} | {sp_str:<8} | {targ_m['cpi']:<7.2f} | {targ_m['l1d_miss']:<12.1f}")

def geomean(iterable):
    a = list(iterable)
    if not a: return 0.0
    return math.exp(math.fsum(math.log(x) for x in a) / len(a))

def print_suite_summary(tests, configs, results):
    print("\n" + "="*100)
    print(f"{'SUMMARY':^100}")
    print("="*100)

    baseline = configs[0]
    target = configs[1]

    speedups = []
    for test in tests:
        base_m = results[baseline].get(test)
        targ_m = results[target].get(test)
        if base_m and targ_m and targ_m['cycles'] > 0:
            speedups.append(base_m['cycles'] / targ_m['cycles'])

    if speedups:
        g_speedup = geomean(speedups)
        print(f"Baseline: {baseline}")
        print(f"Target:   {target}")
        print("-" * 40)
        print(f"Geometric Mean Speedup: {g_speedup:.4f}x")
    else:
        print("No valid data to calculate summary.")
    print("="*100)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run RISC-V benchmarks comparing a target config against a baseline.")
    parser.add_argument("--config", required=True, help="The target configuration file (path or filename in hardware/configs).")
    parser.add_argument("--baseline", default="default.toml", help="The baseline configuration file (default: default.toml).")

    args = parser.parse_args()

    tests, config_names, results = run_benchmarks(args.config, args.baseline)
    print_detailed_report(tests, config_names, results)
    print_suite_summary(tests, config_names, results)
