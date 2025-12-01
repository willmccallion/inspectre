#!/usr/bin/env python3
import os
import toml
import subprocess
import re
import random
import math
import sys
import copy
import argparse
import uuid
from concurrent.futures import ProcessPoolExecutor

SPACE = {
    "bp": ["GShare", "Tournament", "TAGE", "Perceptron"],
    "btb": [512, 1024, 2048, 4096, 8192, 16384],
    "ras": [8, 16, 32, 48, 64],

    # (Size, Ways, Latency) - Ordered by size
    "l1": [
        (4096, 4, 1), (8192, 4, 1), (16384, 4, 1), 
        (32768, 8, 1), (65536, 8, 2), (131072, 8, 3)
    ],
    "l2": [
        (65536, 8, 6), (131072, 8, 8), (262144, 8, 10), 
        (524288, 16, 12), (1048576, 16, 14), (2097152, 16, 20)
    ],
    "prefetch": ["None", "NextLine", "Stride"],
}

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
BIN_DIR = os.path.join(PROJECT_ROOT, "software/bin/benchmarks")
CONFIG_DIR = os.path.join(PROJECT_ROOT, "hardware/configs")
EMULATOR_BIN = os.path.join(PROJECT_ROOT, "hardware/target/release/riscv-emulator")
BEST_CONFIG = os.path.join(CONFIG_DIR, "best_evolved.toml")

class Genome:
    def __init__(self, params=None):
        if params:
            self.p = params
        else:
            # Random initialization
            self.p = {k: random.choice(v) for k, v in SPACE.items()}
        self.score = 0.0

    def mutate(self):
        """Randomly change one gene."""
        key = random.choice(list(SPACE.keys()))
        options = SPACE[key]
        # 80% chance to pick a neighbor value (small step), 20% random jump
        if isinstance(options[0], (int, tuple)) and random.random() < 0.8:
            curr_idx = options.index(self.p[key])
            move = random.choice([-1, 1])
            new_idx = max(0, min(len(options)-1, curr_idx + move))
            self.p[key] = options[new_idx]
        else:
            self.p[key] = random.choice(options)

    def crossover(self, other):
        """Mix genes with another parent."""
        child_p = {}
        for k in SPACE.keys():
            child_p[k] = self.p[k] if random.random() < 0.5 else other.p[k]
        return Genome(child_p)

    def to_toml(self, base_template):
        cfg = copy.deepcopy(base_template)

        cfg['pipeline']['branch_predictor'] = self.p['bp']
        cfg['pipeline']['btb_size'] = self.p['btb']
        cfg['pipeline']['ras_size'] = self.p['ras']

        for lvl, key in [('l1', 'l1'), ('l2', 'l2')]:
            size, ways, lat = self.p[key]
            # Set L1 Instruction & Data identically for simplicity
            targets = ['l1_i', 'l1_d'] if lvl == 'l1' else ['l2']

            for t in targets:
                cfg['cache'][t]['size_bytes'] = size
                cfg['cache'][t]['ways'] = ways
                cfg['cache'][t]['latency'] = lat

                # Only set prefetcher for L1/L2
                if lvl == 'l1':
                    cfg['cache'][t]['prefetcher'] = self.p['prefetch']
                    if self.p['prefetch'] != "None":
                        cfg['cache'][t]['prefetch_degree'] = 2
                        cfg['cache'][t]['prefetch_table_size'] = 64
                elif lvl == 'l2':
                    cfg['cache'][t]['enabled'] = True

        return cfg

    def __str__(self):
        return f"Score: {self.score:.4f} | {self.p['bp']}, BTB={self.p['btb']}, L1={self.p['l1'][0]//1024}k, L2={self.p['l2'][0]//1024}k, {self.p['prefetch']}"

def prebuild_emulator():
    print("Building Emulator (Release Mode)...")
    subprocess.check_call(["cargo", "build", "--release", "--manifest-path", os.path.join(PROJECT_ROOT, "hardware/Cargo.toml")])

def worker_eval(args):
    genome, benchmarks, base_template = args

    # Create Config File
    config_data = genome.to_toml(base_template)
    unique_id = uuid.uuid4().hex
    temp_path = os.path.join(CONFIG_DIR, f"evo_{unique_id}.toml")

    ipcs = []
    try:
        with open(temp_path, 'w') as f:
            toml.dump(config_data, f)

        for b_path in benchmarks:
            cmd = [EMULATOR_BIN, "--file", b_path, "--config", temp_path]
            try:
                res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=20)
                m = re.search(r"sim_ipc\s+([\d\.]+)", res.stdout)
                ipcs.append(float(m.group(1)) if m else 0.0)
            except:
                ipcs.append(0.0)
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

    # Calculate Geomean
    valid = [x for x in ipcs if x > 0]
    if not valid: return 0.0
    return math.exp(math.fsum(math.log(x) for x in valid) / len(valid))

def main():
    parser = argparse.ArgumentParser(description="Evolutionary Design Space Exploration")
    parser.add_argument("--pop", type=int, default=20, help="Population size")
    parser.add_argument("--gens", type=int, default=10, help="Generations")
    parser.add_argument("--workers", type=int, default=os.cpu_count(), help="Parallel threads")
    args = parser.parse_args()

    prebuild_emulator()

    # Load Base Template (for static fields like RAM/UART)
    with open(os.path.join(CONFIG_DIR, "default.toml"), 'r') as f:
        base_template = toml.load(f)

    # Load Benchmarks
    benchmarks = [os.path.join(BIN_DIR, f) for f in os.listdir(BIN_DIR) if f.endswith(".bin")]

    # Initialize Population
    print(f"\nEvolution start...")
    print(f"Population: {args.pop}, Generations: {args.gens}, Workers: {args.workers}")
    population = [Genome() for _ in range(args.pop)]

    global_best = None

    for gen in range(1, args.gens + 1):
        print(f"\nGeneration {gen}/{args.gens} Evaluating...")
        # Evaluate Parallel
        tasks = [(g, benchmarks, base_template) for g in population]
        with ProcessPoolExecutor(max_workers=args.workers) as exe:
            scores = list(exe.map(worker_eval, tasks))

        for g, s in zip(population, scores):
            g.score = s

        # Sort by Score
        population.sort(key=lambda x: x.score, reverse=True)
        top = population[0]

        print(f"  Best of Gen {gen}: {top}")

        if global_best is None or top.score > global_best.score:
            print(f"  >>> NEW GLOBAL RECORD! Saving...")
            global_best = copy.deepcopy(top)
            with open(BEST_CONFIG, 'w') as f:
                toml.dump(global_best.to_toml(base_template), f)

        # Selection & Evolution (Elitism: Keep top 20%)
        elite_count = int(args.pop * 0.2)
        next_gen = population[:elite_count] # Keep Elites

        while len(next_gen) < args.pop:
            parent1 = random.choice(population[:elite_count])
            parent2 = random.choice(population[:elite_count])

            # Crossover
            child = parent1.crossover(parent2)

            if random.random() < 0.4:
                child.mutate()

            next_gen.append(child)

        population = next_gen

    print("\n" + "="*60)
    print("EVOLUTION COMPLETE")
    print(f"Champion: {global_best}")
    print(f"Config saved to: {BEST_CONFIG}")

if __name__ == "__main__":
    main()
