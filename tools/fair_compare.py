#!/usr/bin/env python3
"""Compare equivalent Go and Rust prepared kernels, retaining the real engine oracle."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import random
import statistics
import subprocess
import sys

from compare import ROOT, PIN, build_oracle, command, compare, load


def matched_differences(go, rust):
    differences = compare(go, rust)
    for field in ("scenario_id", "work"):
        if go[field] != rust[field]:
            differences.append(f"{field} differs")
    for field in ("dps_standard_error", "mana_end_mean"):
        if not math.isclose(go[field], rust[field], abs_tol=1e-8, rel_tol=0):
            differences.append(f"{field} differs")
    return differences


def run_with_env(args, env, cwd=None):
    subprocess.run([str(arg) for arg in args], env=env, cwd=cwd, check=True, timeout=180)


def source_digest(paths):
    digest = hashlib.sha256()
    for path in sorted(paths):
        digest.update(str(path.relative_to(ROOT)).encode())
        digest.update(path.read_bytes())
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", default="https://github.com/sage3648/mythicsim-forever-engine.git")
    parser.add_argument("--output", type=Path, default=ROOT / "output" / "fair")
    parser.add_argument("--iterations", type=int, default=30000)
    parser.add_argument("--seeds", type=int, nargs="+", default=[42, 173, 9001])
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--batches", type=int, default=3)
    parser.add_argument("--case", help="restrict scenario IDs by substring")
    args = parser.parse_args()
    if not 1 <= args.iterations <= 1_000_000 or not 1 <= args.samples <= 100 or not 0 <= args.warmups <= 20 or not 1 <= args.batches <= 20 or not args.seeds or any(seed <= 0 or seed > 2**63 - 1 - args.iterations for seed in args.seeds):
        parser.error("invalid benchmark bounds")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    # Identical logical simulation concurrency: one thread, including Go GC work.
    env = {**os.environ, "GOMAXPROCS": "1", "GOGC": "100", "GOMEMLIMIT": "off", "GOWORK": "off"}
    go_version = subprocess.check_output(["go", "version"], text=True).strip()
    rust_version = subprocess.check_output(["rustc", "--version"], text=True).strip()
    oracle, _ = build_oracle(ROOT / "oracle-cache", args.source)
    go = output / "matched-go"
    run_with_env(["go", "build", "-trimpath", "-o", go, "."], env, ROOT / "tools" / "matched-go")
    command(["cargo", "build", "--locked", "--release", "--manifest-path", ROOT / "Cargo.toml"])
    rust = ROOT / "target" / "release" / "forever-engine"
    order_rng = random.Random(42)
    results = []
    for seed in args.seeds:
        inputs = output / f"seed-{seed}" / "inputs"
        run_with_env([oracle, "prepare", "--outfile", inputs, "--iterations", args.iterations, "--seed", seed], env)
        for case in load(inputs / "manifest.json")["cases"]:
            if args.case and args.case not in case["id"]:
                continue
            directory = inputs.parent / case["id"]
            directory.mkdir(exist_ok=True)
            request = load(inputs / case["rust"])
            # The pinned engine verifies the model at its original 3k tolerance.
            # Larger timed runs are checked against each other on every sample.
            verification_rust = directory / "verify.rust.json"
            verification_go = directory / "verify.go.json"
            verification_request = {**request, "iterations": 3000}
            verification_rust.write_text(json.dumps(verification_request))
            oracle_request = load(inputs / case["go"])
            oracle_request["simOptions"]["iterations"] = 3000
            verification_go.write_text(json.dumps(oracle_request))
            outputs = {}
            for name, binary, infile in [("oracle", oracle, verification_go), ("go", go, verification_rust), ("rust", rust, verification_rust)]:
                path = directory / f"verify-{name}.json"
                run_with_env([binary, "sim", "--infile", infile, "--outfile", path], env)
                outputs[name] = load(path)
            differences = compare(outputs["oracle"], outputs["go"]) + compare(outputs["oracle"], outputs["rust"])
            differences += matched_differences(outputs["go"], outputs["rust"])
            timings = {"go": [], "rust": []}
            orders = []
            batch_ratios = []
            reference = None
            for batch in range(args.batches):
                names = ["go", "rust"]
                if order_rng.getrandbits(1):
                    names.reverse()
                orders.append(names)
                batch_reports = {}
                for name in names:
                    path = directory / f"{name}-batch-{batch}.json"
                    binary = go if name == "go" else rust
                    run_with_env([binary, "bench", "--infile", inputs / case["rust"], "--outfile", path, "--warmups", args.warmups, "--samples", args.samples], env)
                    measured = load(path)
                    if measured["warmups"] != args.warmups or measured["samples"] != args.samples or len(measured["reports"]) != args.samples or len(measured["elapsed_ns_samples"]) != args.samples:
                        raise RuntimeError("incomplete benchmark samples")
                    batch_reports[name] = measured
                    timings[name].extend(measured["elapsed_ns_samples"])
                    for report in measured["reports"]:
                        if report["elapsed_ns"] <= 0 or report["iterations"] != args.iterations or report["seed"] != seed or report["scenario_id"] != case["id"]:
                            raise RuntimeError("invalid benchmark report identity or timing")
                        expected_engine = "forever-matched-go-kernel" if name == "go" else "forever-rust-prototype-0.1.0"
                        if report["engine"] != expected_engine:
                            raise RuntimeError("wrong benchmark implementation")
                        if reference is None:
                            reference = report
                        differences += matched_differences(reference, report)
                batch_ratios.append(statistics.median(batch_reports["go"]["elapsed_ns_samples"]) / statistics.median(batch_reports["rust"]["elapsed_ns_samples"]))
            go_median, rust_median = statistics.median(timings["go"]), statistics.median(timings["rust"])
            row = {
                "id": case["id"], "seed": seed, "passed": not differences,
                "differences": sorted(set(differences)), "dps_mean": reference["dps_mean"],
                "counts": reference["counts"], "work": reference["work"],
                "kernel_samples_ns": timings, "go_median_ns": go_median, "rust_median_ns": rust_median,
                "go_over_rust_ratio": go_median / rust_median,
                "paired_batch_ratios": batch_ratios, "execution_orders": orders,
            }
            results.append(row)
            print(f'{"PASS" if row["passed"] else "FAIL"} {case["id"]} seed={seed} Go={go_median/1e6:.3f}ms Rust={rust_median/1e6:.3f}ms Go/Rust={row["go_over_rust_ratio"]:.2f}', flush=True)
            for difference in row["differences"]:
                print(f"  {difference}", flush=True)
    if not results:
        raise RuntimeError("case filter matched no scenarios")
    metadata = {
        "method": "same prepared Frostbolt model, binary event heap, skipped mana polls, labeled RNG, work counters and Welford reporting; single simulation thread; in-process warmup; kernel-only timings; real pinned engine verifies each case at 3000 iterations",
        "source_revision": PIN, "iterations": args.iterations, "verification_iterations": 3000,
        "warmups_per_process": args.warmups, "samples_per_process": args.samples, "batches": args.batches,
        "go_version": go_version, "rust_version": rust_version,
        "go_runtime": {name: env[name] for name in ("GOMAXPROCS", "GOGC", "GOMEMLIMIT")},
        "go_build": "go build -trimpath, default optimizations",
        "rust_build": "cargo release, thin LTO, one codegen unit",
        "kernel_source_sha256": source_digest([*ROOT.glob("src/*.rs"), ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "tools/matched-go/main.go", ROOT / "tools/matched-go/go.mod"]),
        "machine": {"platform": platform.platform(), "architecture": platform.machine(), "logical_cpus": os.cpu_count()},
        "limitations": "shared host, no CPU affinity; matched Go kernel is a new implementation, not upstream optimization; single-spell prepared scope; no full-engine or production claim",
        "passed": all(row["passed"] for row in results), "results": results,
    }
    (output / "summary.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(f'Results: {output / "summary.json"}', flush=True)
    return 0 if metadata["passed"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (RuntimeError, subprocess.SubprocessError, OSError, KeyError, ValueError) as error:
        print(f"fair comparison failed: {error}", file=sys.stderr)
        sys.exit(1)
