#!/usr/bin/env python3
"""Run one production request in Rust, or tell the worker to use Go.

The application worker calls this once per request, from a bundle that `tools/shadow.py
build` made. The pinned Go exporter prepares the RaidSimRequest, the Rust coverage gate
checks it, and then either Rust runs it or the decision names why Go must:

rust      Rust ran the request. --output/result.json holds its RaidSimResult, in the JSON
          Go prints. Exit status 0.
fallback  The gate refused the input. `refusals` lists each reason with its stable code
          and `codes` the distinct codes. No result is written; run the request in Go.
          Exit status 0.
fault     A step failed: the request could not be read, the exporter, the gate or Rust
          exited with an error or timed out, or Rust printed no complete result. `stage`
          and `error` say where. A fault is never a fallback and never writes a result.
          Exit status 2.

The decision is printed as one JSON line and written to --output/decision.json. A request
without a random seed gets a fresh one, as Go would draw, recorded as `seed`. Uses only
Python's standard library.
"""

import argparse
import json
from pathlib import Path
import secrets
import subprocess
import sys
import time

SCHEMA = 1
ENGINE = "forever-engine"
EXPORTER = "forever-go-oracle-v2"
# Go seeds are int64; Rust needs the seed plus the iteration count to stay below the maximum.
MAX_SEED = 2**62


class Fault(Exception):
    def __init__(self, stage, message):
        super().__init__(message)
        self.stage = stage


def run_step(stage, args, timeout, timings):
    """Run one step, recording its wall time. A failure is a fault at that stage."""
    started = time.perf_counter()
    try:
        done = subprocess.run([str(arg) for arg in args], capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise Fault(stage, f"timed out after {timeout} seconds")
    except OSError as error:
        raise Fault(stage, str(error))
    finally:
        timings[stage] = round((time.perf_counter() - started) * 1000, 1)
    if done.returncode != 0:
        raise Fault(stage, done.stderr.strip()[-2000:] or f"exit status {done.returncode}")
    return done.stdout


def seed_request(request, seed):
    """The request's own seed, or the given or a fresh one written into it."""
    options = request.setdefault("simOptions", {})
    own = int(options.get("randomSeed", 0) or 0)
    if own != 0:
        return own, "request"
    if seed is None:
        seed, source = secrets.randbelow(MAX_SEED) + 1, "drawn"
    else:
        source = "argument"
    options["randomSeed"] = str(seed)
    return seed, source


def route(request_path, output, bundle, timeout, seed=None):
    """Decide one request and return the decision. Never raises for a failing step."""
    output.mkdir(parents=True, exist_ok=False)
    engine, exporter = bundle / "bin" / ENGINE, bundle / "bin" / EXPORTER
    timings = {}
    decision = {"schema": SCHEMA, "timings_ms": timings}
    manifest = bundle / "manifest.json"
    if manifest.exists():
        decision["bundle"] = json.loads(manifest.read_text())
    try:
        try:
            request = json.loads(Path(request_path).read_text())
            decision["seed"], decision["seed_source"] = seed_request(request, seed)
        except (OSError, ValueError, AttributeError, TypeError) as error:
            raise Fault("request", f"{type(error).__name__}: {error}")
        request_file = output / "request.json"
        request_file.write_text(json.dumps(request) + "\n")
        prepared = output / "prepared.json"
        run_step("prepare", [exporter, "prepare", "--infile", request_file, "--outfile", prepared,
                             "--scenario", "route"], timeout, timings)

        stdout = run_step("check", [engine, "check", "--infile", prepared], timeout, timings)
        try:
            coverage = json.loads(stdout)
            supported = coverage["supported"]
            refusals = coverage["refusals"]
        except (ValueError, KeyError, TypeError) as error:
            raise Fault("check", f"unreadable gate output: {type(error).__name__}: {error}")
        if supported is not True:
            if not refusals:
                raise Fault("check", "the gate refused the input without a reason")
            decision.update(status="fallback", refusals=refusals,
                            codes=sorted({refusal["code"] for refusal in refusals}))
            return decision

        report_file = output / "rust-report.json"
        run_step("rust", [engine, "sim", "--infile", prepared, "--outfile", report_file], timeout, timings)
        try:
            report = json.loads(report_file.read_text())
            result = report["result"]
            if not isinstance(result.get("raidMetrics"), dict) or result.get("iterationsDone") is None:
                raise ValueError("the result lacks raidMetrics or iterationsDone")
        except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
            raise Fault("rust", f"incomplete result: {type(error).__name__}: {error}")
        result_file = output / "result.json"
        result_file.write_text(json.dumps(result) + "\n")
        decision.update(status="rust", result=str(result_file), identity=report.get("identity"),
                        request_sha256=report.get("request_sha256"))
    except Fault as fault:
        decision.update(status="fault", stage=fault.stage, error=str(fault))
    return decision


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--request", type=Path, required=True, help="RaidSimRequest JSON")
    parser.add_argument("--output", type=Path, required=True, help="new folder to write")
    parser.add_argument("--bundle", type=Path, default=Path(__file__).resolve().parents[1],
                        help="bundle folder (default: the one holding this tool)")
    parser.add_argument("--seed", type=int, help="seed for an unseeded request (default: a fresh one)")
    parser.add_argument("--timeout", type=int, default=300, help="seconds per step")
    args = parser.parse_args()
    if args.seed is not None and not 0 < args.seed <= MAX_SEED:
        parser.error(f"--seed must be from 1 to {MAX_SEED}")
    if args.output.exists():
        parser.error(f"{args.output} exists; each request writes a new folder")
    decision = route(args.request, args.output, args.bundle.resolve(), args.timeout, args.seed)
    (args.output / "decision.json").write_text(json.dumps(decision, indent=2) + "\n")
    print(json.dumps(decision))
    return 2 if decision["status"] == "fault" else 0


if __name__ == "__main__":
    sys.exit(main())
