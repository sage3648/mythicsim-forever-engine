#!/usr/bin/env python3
"""Measure whole Quick Sim jobs in Go and in Rust: time, CPU, peak memory and process overhead.

The fight benchmarks time the iteration loop only. A routed job also starts processes, parses
the request, prepares the character with the Go exporter and checks the gate. This tool runs
each production request as the worker would, from a bundle that `tools/shadow.py build` made:

go          The pinned Go engine on the RaidSimRequest: one process, `EXPORTER sim`.
rust        `tools/route.py --request` from the bundle: Python, the exporter's prepare, the
            gate's check and the Rust simulation, as the worker runs it.

It also runs route.py's three steps on their own, so the record shows where the Rust job's time
and memory go. Every job of a case has the same request, seed, iteration count and report
depth. Rounds interleave the engines; each figure is the median over the rounds. Peak memory
is the largest resident set of the job's processes, from wait4. With --concurrency N, a second
phase runs every case's job N at a time in each engine and records the throughput.

    python3 tools/job_bench.py --bundle BUNDLE --output SCRATCH/report.json \\
        [--rounds 3] [--iterations 3000] [--concurrency 4] [CASE...]

Without cases it runs every single-target `production-*` request fixture. Uses only Python's
standard library.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FAMILY = ROOT / "fixtures/mage/prepared-v2"
ENGINE = "forever-engine"
EXPORTER = "forever-go-oracle-v2"
SEED = 7
# ru_maxrss is in bytes on macOS and in kilobytes on Linux.
RSS_BYTES = 1 if sys.platform == "darwin" else 1024


def measure(command, cwd=None):
    """Run one process to the end. Its wall time, its CPU time and peak resident set, each
    including the processes it waited for, and its exit status."""
    started = time.perf_counter()
    process = subprocess.Popen([str(part) for part in command], cwd=cwd,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    _, status, usage = os.wait4(process.pid, 0)
    wall = time.perf_counter() - started
    process.returncode = os.waitstatus_to_exitcode(status)
    return {
        "wall_s": wall,
        "cpu_s": usage.ru_utime + usage.ru_stime,
        "peak_rss_mb": usage.ru_maxrss * RSS_BYTES / 2**20,
        "status": process.returncode,
    }


def job_request(case, iterations, scratch):
    """The case's request with the iteration count and a fixed seed, as the worker sends it."""
    request = json.loads((FAMILY / f"{case}.request.json").read_text())
    request["simOptions"]["iterations"] = iterations
    request["simOptions"]["randomSeed"] = str(SEED)
    path = scratch / f"{case}.request.json"
    path.write_text(json.dumps(request))
    return path


def go_job(bundle, request, folder):
    folder.mkdir(parents=True)
    result = folder / "go-result.json"
    sample = measure([bundle / "bin" / EXPORTER, "sim", "--infile", request, "--outfile", result])
    document = json.loads(result.read_text())
    sample["engine_s"] = int(document["elapsedNs"]) / 1e9
    sample["dps"] = document["raidMetrics"]["dps"]["avg"]
    return sample


def rust_job(bundle, request, folder):
    sample = measure([sys.executable, bundle / "tools" / "route.py", "--request", request,
                      "--output", folder, "--bundle", bundle])
    decision = json.loads((folder / "decision.json").read_text())
    sample["decision"] = decision["status"]
    if decision["status"] == "rust":
        report = json.loads((folder / "rust-report.json").read_text())
        sample["engine_s"] = int(report["elapsed_ns"]) / 1e9
        sample["dps"] = report["result"]["raidMetrics"]["dps"]["avg"]
    return sample


def rust_steps(bundle, folder):
    """route.py's steps on the files it wrote, each as its own process."""
    engine, exporter = bundle / "bin" / ENGINE, bundle / "bin" / EXPORTER
    prepared = folder / "steps-prepared.json"
    return {
        "prepare": measure([exporter, "prepare", "--infile", folder / "request.json", "--outfile",
                            prepared, "--scenario", "route"]),
        "check": measure([engine, "check", "--infile", prepared]),
        "sim": measure([engine, "sim", "--infile", prepared, "--outfile", folder / "steps-report.json"]),
    }


def median(samples, key):
    return statistics.median(sample[key] for sample in samples)


def summarize(samples, keys=("wall_s", "cpu_s", "peak_rss_mb", "engine_s")):
    return {key: round(median(samples, key), 4 if key.endswith("_s") else 1)
            for key in keys if all(key in sample for sample in samples)}


def throughput(engine, cases, requests, bundle, scratch, concurrency, rounds):
    """Jobs per minute with `concurrency` jobs of one engine at a time."""
    work = [(case, round_index) for round_index in range(rounds) for case in cases]

    def run(item):
        case, round_index = item
        folder = scratch / f"throughput-{engine}" / f"{case}-{round_index}"
        if engine == "go":
            return go_job(bundle, requests[case], folder)
        folder.parent.mkdir(parents=True, exist_ok=True)
        return rust_job(bundle, requests[case], folder)

    started = time.perf_counter()
    with ThreadPoolExecutor(concurrency) as pool:
        samples = list(pool.map(run, work))
    wall = time.perf_counter() - started
    return {"jobs": len(work), "wall_s": round(wall, 2), "jobs_per_minute": round(len(work) * 60 / wall, 1),
            "median_job_wall_s": round(median(samples, "wall_s"), 3),
            "peak_rss_mb": round(max(sample["peak_rss_mb"] for sample in samples), 1)}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--bundle", required=True, type=Path, help="bundle from tools/shadow.py build")
    parser.add_argument("--output", required=True, type=Path, help="report file; inputs go beside it")
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--iterations", type=int, default=3000)
    parser.add_argument("--concurrency", type=int, default=0, help="also measure throughput N jobs at a time")
    parser.add_argument("cases", nargs="*")
    args = parser.parse_args()
    bundle = args.bundle.resolve()
    cases = args.cases or sorted(
        path.name[: -len(".request.json")] for path in FAMILY.glob("production-*.request.json")
        if "-targets" not in path.name)
    scratch = args.output.resolve().parent / "jobs"
    scratch.mkdir(parents=True, exist_ok=False)
    requests = {case: job_request(case, args.iterations, scratch) for case in cases}

    rows = []
    for case in cases:
        go, rust, steps = [], [], []
        for round_index in range(args.rounds):
            folder = scratch / case / str(round_index)
            go.append(go_job(bundle, requests[case], folder / "go"))
            rust.append(rust_job(bundle, requests[case], folder / "rust"))
            if rust[-1]["decision"] == "rust":
                steps.append(rust_steps(bundle, folder / "rust"))
        row = {"case": case, "decision": rust[-1]["decision"], "go": summarize(go)}
        if row["decision"] == "rust":
            row["rust"] = summarize(rust)
            row["rust_steps"] = {step: summarize([sample[step] for sample in steps],
                                                 ("wall_s", "cpu_s", "peak_rss_mb"))
                                 for step in ("prepare", "check", "sim")}
            row["same_dps"] = go[-1]["dps"] == rust[-1]["dps"]
            row["go_over_rust_wall"] = round(row["go"]["wall_s"] / row["rust"]["wall_s"], 2)
            # Everything a job spends outside the engine's own iteration loop.
            row["overhead_s"] = {name: round(row[name]["wall_s"] - row[name]["engine_s"], 4)
                                 for name in ("go", "rust")}
            print(f"{case:34} go {row['go']['wall_s']:6.3f}s {row['go']['peak_rss_mb']:6.1f} MB  "
                  f"rust {row['rust']['wall_s']:6.3f}s {row['rust']['peak_rss_mb']:6.1f} MB  "
                  f"Go/Rust {row['go_over_rust_wall']:.2f}x  overhead go {row['overhead_s']['go']:.3f}s "
                  f"rust {row['overhead_s']['rust']:.3f}s{'' if row['same_dps'] else '  DPS DIFFERS'}",
                  flush=True)
        else:
            print(f"{case:34} rust {row['decision']}, Go only", flush=True)
        rows.append(row)

    timed = [row for row in rows if row["decision"] == "rust"]
    report = {
        "machine": {"platform": platform.platform(), "cpu": platform.processor() or platform.machine(),
                    "cpus": os.cpu_count()},
        "bundle": json.loads((bundle / "manifest.json").read_text()),
        "iterations": args.iterations,
        "rounds": args.rounds,
        "seed": SEED,
        "summary": {
            "cases": len(rows),
            "routed_to_rust": len(timed),
            "go_over_rust_wall_median": round(statistics.median(row["go_over_rust_wall"] for row in timed), 2),
            "go_over_rust_wall_min": round(min(row["go_over_rust_wall"] for row in timed), 2),
            "go_over_rust_wall_max": round(max(row["go_over_rust_wall"] for row in timed), 2),
            "overhead_s_median": {name: round(statistics.median(row["overhead_s"][name] for row in timed), 4)
                                  for name in ("go", "rust")},
            "peak_rss_mb_median": {name: round(statistics.median(row[name]["peak_rss_mb"] for row in timed), 1)
                                   for name in ("go", "rust")},
            "rust_step_wall_s_median": {step: round(statistics.median(row["rust_steps"][step]["wall_s"]
                                                                      for row in timed), 4)
                                        for step in ("prepare", "check", "sim")},
            "all_same_dps": all(row["same_dps"] for row in timed),
        },
        "rows": rows,
    }
    if args.concurrency:
        routed = [row["case"] for row in timed]
        report["throughput"] = {
            "concurrency": args.concurrency,
            "cases": routed,
            "go": throughput("go", routed, requests, bundle, scratch, args.concurrency, args.rounds),
            "rust": throughput("rust", routed, requests, bundle, scratch, args.concurrency, args.rounds),
        }
        print(json.dumps(report["throughput"], indent=1), flush=True)
    args.output.write_text(json.dumps(report, indent=1) + "\n")
    print(json.dumps(report["summary"], indent=1))


if __name__ == "__main__":
    main()
