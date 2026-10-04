#!/usr/bin/env python3
"""Time the pinned Go engine and two Rust builds on the production prepared v2 fixtures.

For each fixture, writes copies of its request and prepared input with the iteration count
raised and debug logs off under the output directory, then runs the Go exporter's `sim`, the
base `forever-engine sim` and the current `forever-engine sim` in turn, ROUNDS times. Go runs
on one scheduler thread (GOMAXPROCS=1), as in the earlier benchmark snapshots. It records
each engine's minimum self-reported elapsed time (the iteration loop) and minimum process CPU
time, and checks that the two Rust results are identical apart from elapsed time.

    python3 tools/production_bench.py --base OLD_BINARY --current NEW_BINARY \\
        --output output/bench/report.json [--rounds 3] [--iterations 10000] [CASE...]

Without cases it times every `production-*` fixture. Needs the oracle built by
`tools/prepared_v2.py`; uses only Python's standard library.
"""
import argparse
import json
import os
import platform
import resource
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FAMILY = ROOT / "fixtures/mage/prepared-v2"
GO = ROOT / "oracle-cache/forever-go-oracle-v2"


def cpu_seconds():
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime + usage.ru_stime


def comparable(document):
    """The Rust result without its timing."""
    result = document["result"]
    return {key: value for key, value in result.items() if key != "elapsedNs"}


def raised(case, iterations, scratch):
    request = json.loads((FAMILY / f"{case}.request.json").read_text())
    prepared = json.loads((FAMILY / f"{case}.prepared.json").read_text())
    request["simOptions"]["iterations"] = iterations
    request["simOptions"]["debugFirstIteration"] = False
    request["simOptions"].pop("debug", None)
    prepared["sim"].update(iterations=iterations, debug_first_iteration=False, debug=False)
    request_path = scratch / f"{case}.request.json"
    prepared_path = scratch / f"{case}.prepared.json"
    request_path.write_text(json.dumps(request))
    prepared_path.write_text(json.dumps(prepared))
    return request_path, prepared_path


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--base", required=True, help="the forever-engine binary before")
    parser.add_argument("--current", required=True, help="the forever-engine binary after")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--iterations", type=int, default=10000)
    parser.add_argument("cases", nargs="*")
    args = parser.parse_args()
    cases = args.cases or sorted(
        path.name[: -len(".prepared.json")] for path in FAMILY.glob("production-*.prepared.json")
    )
    scratch = args.output.parent / "inputs"
    scratch.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, GOMAXPROCS="1")
    rows = []
    for case in cases:
        request_path, prepared_path = raised(case, args.iterations, scratch)
        engines = {
            "go": [str(GO), "sim", "--infile", str(request_path)],
            "base": [args.base, "sim", "--infile", str(prepared_path)],
            "current": [args.current, "sim", "--infile", str(prepared_path)],
        }
        elapsed = {name: [] for name in engines}
        cpu = {name: [] for name in engines}
        documents = {}
        for _ in range(args.rounds):
            for name, command in engines.items():
                target = scratch / f"{case}.{name}.json"
                before = cpu_seconds()
                subprocess.run(command + ["--outfile", str(target)], check=True, env=environment,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                cpu[name].append(cpu_seconds() - before)
                document = json.loads(target.read_text())
                result = document.get("result", document)
                elapsed[name].append(int(document.get("elapsed_ns") or result["elapsedNs"]) / 1e9)
                documents[name] = document
        row = {
            "case": case,
            "dps": documents["current"]["result"]["raidMetrics"]["dps"]["avg"],
            "go_dps": documents["go"]["raidMetrics"]["dps"]["avg"],
            "rust_results_identical": comparable(documents["base"]) == comparable(documents["current"]),
        }
        for name in engines:
            row[f"{name}_elapsed_s"] = round(min(elapsed[name]), 3)
            row[f"{name}_cpu_s"] = round(min(cpu[name]), 3)
        rows.append(row)
        print(
            f"{case:34} go {row['go_elapsed_s']:6.3f}s base {row['base_elapsed_s']:6.3f}s "
            f"current {row['current_elapsed_s']:6.3f}s  Go/current {row['go_elapsed_s'] / row['current_elapsed_s']:.2f}x "
            f"(Go/base {row['go_elapsed_s'] / row['base_elapsed_s']:.2f}x)"
            f"{'' if row['rust_results_identical'] else '  RUST RESULTS DIFFER'}",
            flush=True,
        )
    report = {
        "machine": platform.platform(),
        "rounds": args.rounds,
        "iterations": args.iterations,
        "rows": rows,
    }
    args.output.write_text(json.dumps(report, indent=1) + "\n")


if __name__ == "__main__":
    main()
