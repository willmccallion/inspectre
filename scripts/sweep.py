#!/usr/bin/env python3
import os
import toml
import subprocess
import re
import matplotlib.pyplot as plt
import sys
import argparse
import math

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
BIN_DIR = os.path.join(PROJECT_ROOT, "software/bin/benchmarks")
CONFIG_DIR = os.path.join(PROJECT_ROOT, "hardware/configs")
TEMP_CONFIG = os.path.join(CONFIG_DIR, "temp_sweep.toml")
CARGO_MANIFEST = os.path.join(PROJECT_ROOT, "hardware/Cargo.toml")
OUTPUT_DIR = os.path.join(PROJECT_ROOT, "scripts/output")

os.makedirs(OUTPUT_DIR, exist_ok=True)

def run_simulation(config_path, binary_path):
    cmd = [
        "cargo", "run", "--quiet", "--release",
        "--manifest-path", CARGO_MANIFEST,
        "--", "--file", binary_path, "--config", config_path
    ]
    try:
        # 45s timeout because some configs might be slow
        result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=45)
        match = re.search(r"sim_ipc\s+([\d\.]+)", result.stdout)
        if match:
            return float(match.group(1))
    except subprocess.TimeoutExpired:
        return 0.0
    except KeyboardInterrupt:
        sys.exit(1)
    return 0.0

def update_nested_key(data, key_path, value):
    keys = key_path.split('.')
    d = data
    for k in keys[:-1]:
        d = d.setdefault(k, {})
    d[keys[-1]] = value

def geomean(iterable):
    a = [x for x in iterable if x > 0]
    if not a: return 0.0
    return math.exp(math.fsum(math.log(x) for x in a) / len(a))

def parse_values(val_str):
    vals = []
    for v in val_str.split(','):
        v = v.strip()
        try:
            vals.append(int(v))
        except ValueError:
            try:
                vals.append(float(v))
            except ValueError:
                vals.append(v)
    return vals

def main():
    parser = argparse.ArgumentParser(description="Sweep a hardware parameter across all benchmarks.")
    parser.add_argument("param", help="The TOML key to sweep (e.g. cache.l1_d.size_bytes)")
    parser.add_argument("values", help="Comma-separated list of values (e.g. 4096,8192,16384)")
    parser.add_argument("--base", default="tournament.toml", help="Base config file name (default: tournament.toml)")

    args = parser.parse_args()

    values = parse_values(args.values)
    param_path = args.param
    base_config_name = args.base

    print(f"Sweeping {param_path}...")
    print(f"Values: {values}")

    base_config_path = os.path.join(CONFIG_DIR, base_config_name)
    if not os.path.exists(base_config_path):
        print(f"Error: Base config {base_config_path} not found.")
        return

    with open(base_config_path, 'r') as f:
        base_data = toml.load(f)

    if not os.path.exists(BIN_DIR):
        print("Error: Benchmark binary directory not found.")
        return
    benchmarks = [f.replace(".bin", "") for f in os.listdir(BIN_DIR) if f.endswith(".bin")]
    benchmarks.sort()

    print(f"Found {len(benchmarks)} benchmarks. This might take a while...")

    # Data structure: results[benchmark] = [ipc_val1, ipc_val2, ...]
    results = {b: [] for b in benchmarks}

    for val in values:
        print(f"\n--- Setting {param_path} = {val} ---")

        # Write Temp Config
        update_nested_key(base_data, param_path, val)
        with open(TEMP_CONFIG, 'w') as f:
            toml.dump(base_data, f)

        for bench in benchmarks:
            bin_path = os.path.join(BIN_DIR, bench + ".bin")
            ipc = run_simulation(TEMP_CONFIG, bin_path)
            results[bench].append(ipc)

            # Print brief status
            print(f"  {bench:<20} IPC: {ipc:.4f}")

    # Cleanup
    if os.path.exists(TEMP_CONFIG):
        os.remove(TEMP_CONFIG)

    print("\nCalculating Statistics...")

    geomeans = []
    for i in range(len(values)):
        column = [results[b][i] for b in benchmarks]
        gm = geomean(column)
        geomeans.append(gm)

    # Print Summary Table
    print("\n" + "="*60)
    print(f"{'Value':<15} | {'Geomean IPC':<15} | {'Gain vs Base':<15}")
    print("-" * 60)
    base_ipc = geomeans[0] if geomeans[0] > 0 else 1.0
    for v, gm in zip(values, geomeans):
        gain = (gm / base_ipc - 1.0) * 100.0
        print(f"{str(v):<15} | {gm:<15.4f} | {gain:+.2f}%")
    print("="*60)

    # Plotting
    plt.figure(figsize=(12, 8))

    # Plot faint lines for individual benchmarks
    for b in benchmarks:
        # Only plot if benchmark has valid data
        if any(x > 0 for x in results[b]):
            plt.plot(values, results[b], color='gray', alpha=0.3, linewidth=1)

    # Plot thick line for Geomean
    plt.plot(values, geomeans, color='#e41a1c', linewidth=3, marker='o', label='Suite Geomean')

    plt.title(f"Sensitivity Sweep: {param_path}", fontsize=16)
    plt.xlabel(f"Parameter Value: {param_path.split('.')[-1]}", fontsize=14)
    plt.ylabel("IPC (Instructions Per Cycle)", fontsize=14)
    plt.grid(True, which="both", ls="--", alpha=0.7)
    plt.legend()

    # Log scale for X axis if values span orders of magnitude (like cache sizes)
    if isinstance(values[0], (int, float)) and values[-1] / values[0] > 10:
        plt.xscale('log', base=2)

    plot_file = os.path.join(OUTPUT_DIR, f"sweep_{param_path.replace('.','_')}.png")
    plt.savefig(plot_file, dpi=150)
    print(f"\nPlot saved to: {plot_file}")
    plt.show()

if __name__ == "__main__":
    main()
