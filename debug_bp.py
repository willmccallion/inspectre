#!/usr/bin/env python3
"""Debug script to investigate branch prediction accuracy."""

from rvsim import BranchPredictor, Config, Environment, Stats

CYCLE_LIMIT = 50_000_000

predictors = [
    ("static", BranchPredictor.Static()),
    ("gshare", BranchPredictor.GShare()),
    ("tage", BranchPredictor.TAGE()),
    ("tournament", BranchPredictor.Tournament()),
    ("perceptron", BranchPredictor.Perceptron()),
]

programs = [
    "mandelbrot",
    "qsort",
    "merge_sort",
    "raytracer",
]

for program in programs:
    binary = f"software/bin/programs/{program}.bin"
    rows = {}
    for bp_name, bp in predictors:
        config = Config(branch_predictor=bp, uart_quiet=True)
        env = Environment(binary=binary, config=config)
        result = env.run(quiet=True, limit=CYCLE_LIMIT)
        rows[bp_name] = result.stats.query("branch")
    print(Stats.tabulate(rows, title=f"{program} — Branch Prediction"))
    print()
