#!/usr/bin/env python3
import subprocess
import os
import re
import math
import sys
import csv

try:
    import matplotlib.pyplot as plt
    HAS_MATPLOTLIB = True
except ImportError:
    HAS_MATPLOTLIB = False
    print("Warning: 'matplotlib' not found. Plot generation will be skipped.")

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(SCRIPT_DIR, ".."))

BIN_DIR = os.path.join(PROJECT_ROOT, "software/bin/benchmarks")
CONFIG_DIR = os.path.join(PROJECT_ROOT, "hardware/configs")
CARGO_MANIFEST = os.path.join(PROJECT_ROOT, "hardware/Cargo.toml")

BENCH_SRC_ROOT = os.path.join(PROJECT_ROOT, "software/benchmarks")
CATEGORIES = ["microbenchmarks", "kernels", "synthetic", "complete_prog"]

OUTPUT_DIR = os.path.join(SCRIPT_DIR, "output")
os.makedirs(OUTPUT_DIR, exist_ok=True)

OUTPUT_IMG = os.path.join(OUTPUT_DIR, "benchmark_dashboard.png")
OUTPUT_CSV = os.path.join(OUTPUT_DIR, "benchmark_results.csv")

def get_test_categories(tests):
    """
    Scans source directories to map each test name to a category.
    Returns: dict { category_name: [sorted_test_names] }
    """
    cat_map = {c: [] for c in CATEGORIES}
    cat_map["uncategorized"] = []

    test_to_cat = {}

    for cat in CATEGORIES:
        src_dir = os.path.join(BENCH_SRC_ROOT, cat)
        if os.path.exists(src_dir):
            for f in os.listdir(src_dir):
                if f.endswith(".c"):
                    test_name = f.replace(".c", "")
                    test_to_cat[test_name] = cat

    # Assign tests to categories
    for test in tests:
        cat = test_to_cat.get(test, "uncategorized")
        cat_map[cat].append(test)

    # Sort tests within categories
    for cat in cat_map:
        cat_map[cat].sort()

    return cat_map

def run_benchmarks():
    if not os.path.exists(BIN_DIR):
        print(f"Error: {BIN_DIR} does not exist.")
        sys.exit(1)

    # Get all binary tests
    tests = [f.replace(".bin", "") for f in os.listdir(BIN_DIR) if f.endswith(".bin")]
    tests.sort()

    # Filter out configs containing 'trace'
    configs = [f for f in os.listdir(CONFIG_DIR) if f.endswith(".toml") and "trace" not in f]
    configs.sort()

    # Ensure baseline is first
    if "baseline.toml" in configs:
        configs.insert(0, configs.pop(configs.index("baseline.toml")))

    results = {conf: {} for conf in configs}

    print(f"Found {len(tests)} benchmarks and {len(configs)} configurations.")
    print("-" * 60)

    for conf in configs:
        conf_path = os.path.join(CONFIG_DIR, conf)
        print(f"Running Config: {conf}")

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
                    results[conf][test] = None
                    continue

                metrics = {}

                m_iso = re.search(r"Benchmark Cycles:\s+(\d+)", proc.stdout)
                m_tot = re.search(r"sim_cycles\s+(\d+)", proc.stdout)

                if m_iso:
                    metrics['cycles'] = int(m_iso.group(1))
                    metrics['source'] = 'iso' # Track that we got isolated data
                elif m_tot:
                    metrics['cycles'] = int(m_tot.group(1))
                    metrics['source'] = 'tot' # Fallback to total
                else:
                    metrics['cycles'] = 0

                patterns = {
                    "cpi": r"sim_cpi\s+([\d\.]+)",
                    "ipc": r"sim_ipc\s+([\d\.]+)",
                    "bp_acc": r"bp\.accuracy\s+([\d\.]+)%",
                    "l1d_miss": r"L1-D\s+accesses.*miss_rate:\s+([\d\.]+)%",
                    "l1i_miss": r"L1-I\s+accesses.*miss_rate:\s+([\d\.]+)%",
                    "l2_miss": r"L2\s+accesses.*miss_rate:\s+([\d\.]+)%"
                }

                for key, pat in patterns.items():
                    match = re.search(pat, proc.stdout)
                    if match:
                        metrics[key] = float(match.group(1))
                    else:
                        metrics[key] = 0.0

                # Only count as success if we found cycles
                if metrics['cycles'] > 0:
                    results[conf][test] = metrics
                else:
                    results[conf][test] = None

            except subprocess.TimeoutExpired:
                print(f"  [TIMEOUT] {test}")
                results[conf][test] = None

    return tests, configs, results

def print_detailed_report(tests, configs, results):
    # Get Categorized Map
    cat_map = get_test_categories(tests)

    # Define Print Order
    print_order = ["microbenchmarks", "kernels", "synthetic", "complete_prog", "uncategorized"]

    print("\n" + "="*110)
    print(f"{'DETAILED PERFORMANCE REPORT':^110}")
    print("="*110)

    baseline = configs[0]

    for cat in print_order:
        cat_tests = cat_map.get(cat, [])
        if not cat_tests:
            continue

        # Print Category Header
        header = f"CATEGORY: {cat.upper().replace('_', ' ')}"
        print(f"\n{header:^110}")
        print("-" * 110)

        for test in cat_tests:
            print(f"\nBenchmark: {test}")
            # Split headers into Latency (Time) and Throughput (Work/Time)
            print(f"{'Config':<20} | {'LATENCY (Cycles)':<18} | {'Speedup':<8} || {'THROUGHPUT (IPC)':<18} | {'CPI':<6} | {'BP Acc%':<8}")
            print("-" * 110)

            base_metrics = results[baseline].get(test)
            base_cycles = base_metrics['cycles'] if base_metrics else None

            for conf in configs:
                m = results[conf].get(test)
                if not m:
                    print(f"{conf:<20} | {'CRASH':<18} | {'-':<8} || {'-':<18} | {'-':<6} | {'-':<8}")
                    continue

                speedup = 1.0
                if base_cycles and m['cycles'] > 0:
                    speedup = base_cycles / m['cycles']

                cyc_str = str(int(m['cycles']))
                if m.get('source') == 'tot':
                    cyc_str += "*"

                ipc = m.get('ipc', 0.0)

                # Format: Config | Cycles | Speedup || IPC | CPI | BP
                print(f"{conf.replace('.toml',''):<20} | {cyc_str:<18} | {speedup:<8.2f} || {ipc:<18.3f} | {m['cpi']:<6.2f} | {m['bp_acc']:<8.1f}")

    print("\n(*) Indicates fallback to Total Simulation Cycles (Benchmark isolation failed or not implemented)")

def geomean(iterable):
    a = list(iterable)
    if not a: return 0.0
    return math.exp(math.fsum(math.log(x) for x in a) / len(a))

def print_suite_summary(tests, configs, results):
    print("\n" + "="*110)
    print(f"{'SUITE SUMMARY':^110}")
    print("="*110)
    print(f"Baseline Config: {configs[0]}")
    print("-" * 110)
    print(f"{'Configuration':<25} | {'Geomean Speedup':<20} | {'Geomean IPC':<20}")
    print(f"{'':<25} | {'(Latency Improvement)':<20} | {'(Avg Throughput)':<20}")
    print("-" * 110)

    baseline = configs[0]

    for conf in configs:
        speedups = []
        ipcs = []

        for test in tests:
            base_m = results[baseline].get(test)
            conf_m = results[conf].get(test)

            if base_m and conf_m and conf_m['cycles'] > 0:
                # Speedup (Latency metric)
                s = base_m['cycles'] / conf_m['cycles']
                speedups.append(s)

                # IPC (Throughput metric)
                if conf_m['ipc'] > 0:
                    ipcs.append(conf_m['ipc'])

        if speedups and ipcs:
            g_speedup = geomean(speedups)
            g_ipc = geomean(ipcs)
            print(f"{conf.replace('.toml',''):<25} | {g_speedup:<20.4f} | {g_ipc:<20.4f}")
        else:
            print(f"{conf.replace('.toml',''):<25} | {'N/A':<20} | {'N/A':<20}")
    print("="*110)

def export_csv(tests, configs, results):
    print(f"\nExporting CSV: {OUTPUT_CSV}")
    cat_map = get_test_categories(tests)

    # Invert cat_map for O(1) lookup
    test_cat = {}
    for cat, t_list in cat_map.items():
        for t in t_list:
            test_cat[t] = cat

    with open(OUTPUT_CSV, 'w', newline='') as csvfile:
        writer = csv.writer(csvfile)
        writer.writerow(['Category', 'Benchmark', 'Config', 'Cycles', 'Cycle_Source', 'Speedup', 'CPI', 'IPC', 'BP_Accuracy', 'L1D_Miss_Rate'])

        baseline = configs[0]

        for test in tests:
            base_metrics = results[baseline].get(test)
            base_cycles = base_metrics['cycles'] if base_metrics else None
            cat = test_cat.get(test, "uncategorized")

            for conf in configs:
                m = results[conf].get(test)
                if m:
                    speedup = base_cycles / m['cycles'] if base_cycles and m['cycles'] > 0 else 0
                    writer.writerow([
                        cat, test, conf, m['cycles'], m.get('source', 'unk'), speedup, m['cpi'], m['ipc'], 
                        m['bp_acc'], m['l1d_miss']
                    ])
                else:
                    writer.writerow([cat, test, conf, 'ERR', 'ERR', 'ERR', 'ERR', 'ERR', 'ERR', 'ERR'])

def generate_dashboard(tests, configs, results):
    if not HAS_MATPLOTLIB: return

    print(f"Generating Dashboard: {OUTPUT_IMG}")
    plt.style.use('default')

    baseline = configs[0]
    valid_tests = [t for t in tests if results[baseline].get(t)]

    # Sort valid_tests by category for the plot
    cat_map = get_test_categories(valid_tests)
    sorted_tests = []
    print_order = ["microbenchmarks", "kernels", "synthetic", "complete_prog", "uncategorized"]
    for cat in print_order:
        sorted_tests.extend(cat_map.get(cat, []))

    data = {
        'speedup': {c: [] for c in configs},
        'cpi':     {c: [] for c in configs},
        'l1d':     {c: [] for c in configs},
        'bp':      {c: [] for c in configs}
    }

    for test in sorted_tests:
        base_cyc = results[baseline][test]['cycles']

        for conf in configs:
            m = results[conf].get(test)
            if m:
                data['speedup'][conf].append(base_cyc / m['cycles'] if m['cycles'] > 0 else 0)
                data['cpi'][conf].append(m['cpi'])
                data['l1d'][conf].append(m['l1d_miss'])
                data['bp'][conf].append(m['bp_acc'])
            else:
                for k in data: data[k][conf].append(0)

    fig, axs = plt.subplots(2, 2, figsize=(20, 12), facecolor='white')

    x = range(len(sorted_tests))
    width = 0.8 / len(configs)
    colors = ['#333333', '#e41a1c', '#377eb8', '#4daf4a', '#984ea3', '#ff7f00']

    def plot_bars(ax, metric_key, title, ylabel, hline=None):
        for i, conf in enumerate(configs):
            offset = width * i
            pos = [p + offset - (0.8/2) + (width/2) for p in x]
            lbl = conf.replace(".toml", "")
            ax.bar(pos, data[metric_key][conf], width, label=lbl, color=colors[i % len(colors)])

        ax.set_title(title, fontsize=12, fontweight='bold')
        ax.set_ylabel(ylabel)
        ax.set_xticks(x)
        ax.set_xticklabels(sorted_tests, rotation=45, ha='right', fontsize=8)
        ax.grid(axis='y', linestyle='--', alpha=0.3)
        if hline: ax.axhline(y=hline, color='black', linestyle='--', linewidth=1)

    plot_bars(axs[0, 0], 'speedup', f'Speedup vs {baseline.replace(".toml","")}', 'Speedup (Higher is Better)', hline=1.0)
    axs[0, 0].legend(loc='upper left', fontsize='small')
    plot_bars(axs[0, 1], 'cpi', 'Cycles Per Instruction (CPI)', 'CPI (Lower is Better)')
    plot_bars(axs[1, 0], 'l1d', 'L1 Data Cache Miss Rate', 'Miss Rate % (Lower is Better)')
    plot_bars(axs[1, 1], 'bp', 'Branch Prediction Accuracy', 'Accuracy % (Higher is Better)')

    plt.tight_layout()
    plt.savefig(OUTPUT_IMG, dpi=150, facecolor='white')
    print("Done.")

    try:
        plt.show()
    except:
        pass

if __name__ == "__main__":
    tests, configs, results = run_benchmarks()
    print_detailed_report(tests, configs, results)
    print_suite_summary(tests, configs, results)
    export_csv(tests, configs, results)
    generate_dashboard(tests, configs, results)
