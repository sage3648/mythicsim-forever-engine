#!/usr/bin/env python3
"""Build the pinned oracle in scratch, compare real engine results, and benchmark.

Uses only Python's standard library. Both engines run serial iterations with
labeled random streams. Startup and kernel timings are reported separately.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
PIN = "6823b49eb8aff741f197ef36d83766ef6a218285"


def command(args, *, cwd=None, timeout=300):
    subprocess.run([str(arg) for arg in args], cwd=cwd, check=True, timeout=timeout)


def load(path):
    return json.loads(path.read_text())


def build_oracle(cache, source):
    cache.mkdir(parents=True, exist_ok=True)
    checkout = cache / "source"
    if not checkout.exists():
        # A private checkout keeps generated protos and the helper out of the
        # user's engine repository. --no-hardlinks also isolates Git objects.
        command(["git", "clone", "--no-checkout", "--no-hardlinks", source, checkout])
        command(["git", "checkout", "--detach", PIN], cwd=checkout)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip()
    if revision != PIN:
        raise RuntimeError(f"oracle checkout must be {PIN}, found {revision}")
    # Never benchmark a previously edited source tree just because HEAD matches.
    command(["git", "diff", "--exit-code", "HEAD", "--"], cwd=checkout)
    helper = ROOT / "tools" / "oracle" / "main.go"
    digest = hashlib.sha256(helper.read_bytes()).hexdigest()
    binary = cache / "forever-go-oracle"
    stamp = cache / "build.json"
    compiler = subprocess.check_output(["go", "version"], text=True).strip()
    identity = {"revision": PIN, "helper_sha256": digest, "compiler": compiler}
    if binary.exists() and stamp.exists() and load(stamp) == identity:
        return binary, compiler
    plugin = cache / "protoc-gen-go"
    command(["go", "build", "-o", plugin, "google.golang.org/protobuf/cmd/protoc-gen-go"], cwd=checkout)
    command([
        "protoc", f"--plugin=protoc-gen-go={plugin}", "-I=./proto",
        "--go_opt=Mgoogle/protobuf/descriptor.proto=google.golang.org/protobuf/types/descriptorpb",
        "--go_out=./sim/core", *sorted(str(p.relative_to(checkout)) for p in (checkout / "proto").glob("*.proto")),
    ], cwd=checkout)
    target = checkout / "cmd" / "mythicsim-rust-oracle"
    target.mkdir(exist_ok=True)
    shutil.copy2(helper, target / "main.go")
    command(["go", "build", "-trimpath", "--tags=with_db", "-o", binary, "./cmd/mythicsim-rust-oracle"], cwd=checkout)
    stamp.write_text(json.dumps(identity, indent=2) + "\n")
    return binary, compiler


def measured(args):
    started = time.perf_counter_ns()
    command(args, timeout=120)
    return time.perf_counter_ns() - started


def compare(go, rust):
    differences = []
    for field in ("source_revision", "iterations", "seed", "counts"):
        if go[field] != rust[field]:
            differences.append(f"{field}: Go={go[field]!r}, Rust={rust[field]!r}")
    for field in ("dps_mean", "dps_stdev", "mana_delta_mean"):
        if not abs(go[field] - rust[field]) <= 1e-8:
            differences.append(f"{field}: Go={go[field]:.12f}, Rust={rust[field]:.12f}")
    return differences


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", default="https://github.com/sage3648/mythicsim-forever-engine-go.git", help="local Git repository or clone URL")
    parser.add_argument("--cache", type=Path, default=ROOT / "oracle-cache")
    parser.add_argument("--output", type=Path, default=ROOT / "output")
    parser.add_argument("--iterations", type=int, default=3000)
    parser.add_argument("--seeds", type=int, nargs="+", default=[42, 173, 9001])
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--case", help="run only case IDs containing this text")
    parser.add_argument("--trace", action="store_true", help="capture Go logs and Rust events on first repeat")
    args = parser.parse_args()
    if not 1 <= args.iterations <= 1_000_000 or not 1 <= args.repeats <= 20 or any(seed <= 0 or seed > 2**63 - 1 - args.iterations for seed in args.seeds):
        parser.error("invalid iteration count, repeat count or seed")
    cache, output = args.cache.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    oracle, go_version = build_oracle(cache, args.source)
    command(["cargo", "build", "--locked", "--release", "--manifest-path", ROOT / "Cargo.toml"])
    rust = ROOT / "target" / "release" / "forever-engine"
    results = []
    for seed in args.seeds:
        inputs = output / f"seed-{seed}" / "inputs"
        command([oracle, "prepare", "--outfile", inputs, "--iterations", args.iterations, "--seed", seed])
        manifest = load(inputs / "manifest.json")
        for case in manifest["cases"]:
            if args.case and args.case not in case["id"]:
                continue
            timings = {name: [] for name in ("go_kernel_ns", "rust_kernel_ns", "go_process_ns", "rust_process_ns")}
            directory = inputs.parent / case["id"]
            directory.mkdir(exist_ok=True)
            failures = []
            for repeat in range(args.repeats):
                go_out = directory / f"go-{repeat}.json"
                rust_out = directory / f"rust-{repeat}.json"
                trace = ["--trace"] if args.trace and repeat == 0 else []
                commands = [
                    ("go", [oracle, "sim", "--infile", inputs / case["go"], "--outfile", go_out, *trace]),
                    ("rust", [rust, "sim", "--infile", inputs / case["rust"], "--outfile", rust_out, *trace]),
                ]
                # Alternate execution order to reduce systematic warmup bias.
                if repeat % 2:
                    commands.reverse()
                for name, invocation in commands:
                    elapsed = measured(invocation)
                    if not trace:
                        timings[f"{name}_process_ns"].append(elapsed)
                go_result, rust_result = load(go_out), load(rust_out)
                failures.extend(compare(go_result, rust_result))
                if not trace:
                    timings["go_kernel_ns"].append(go_result["elapsed_ns"])
                    timings["rust_kernel_ns"].append(rust_result["elapsed_ns"])
            medians = {name: statistics.median(values) if values else None for name, values in timings.items()}
            speedup = medians["go_kernel_ns"] / medians["rust_kernel_ns"] if medians["rust_kernel_ns"] else None
            row = {
                "id": case["id"], "seed": seed, "passed": not failures,
                "differences": sorted(set(failures)), "dps_go": go_result["dps_mean"],
                "dps_rust": rust_result["dps_mean"], "counts": rust_result["counts"],
                "timing_samples": timings, "timing_medians": medians,
                "prepared_kernel_speedup": speedup,
            }
            results.append(row)
            print(f'{"PASS" if row["passed"] else "FAIL"} {case["id"]} seed={seed} Go={row["dps_go"]:.6f} Rust={row["dps_rust"]:.6f}', flush=True)
            for difference in row["differences"]:
                print(f"  {difference}", flush=True)
    if not results:
        raise RuntimeError("case filter matched no scenarios")
    summary = {
        "source_revision": PIN, "iterations": args.iterations, "repeats": args.repeats,
        "go_version": go_version, "rust_version": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "machine": {"platform": platform.platform(), "architecture": platform.machine(), "logical_cpus": os.cpu_count()},
        "method": "serial Go full-engine execution versus Rust prepared Frostbolt kernel; labeled RNG; wall-clock timings; setup and supported scope differ; no production speedup claim",
        "passed": all(row["passed"] for row in results), "results": results,
    }
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(f'Results: {output / "summary.json"}', flush=True)
    return 0 if summary["passed"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (RuntimeError, subprocess.SubprocessError, OSError) as error:
        print(f"comparison failed: {error}", file=sys.stderr)
        sys.exit(1)
