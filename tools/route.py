#!/usr/bin/env python3
"""Run one production request in Rust, or tell the worker to use Go.

The application worker calls this once per request, from a bundle that `tools/shadow.py
build` made. The pinned Go exporter prepares the RaidSimRequest, and then one Rust process
gates the prepared input and runs it when the Rust coverage gate supports it, or the decision
names why Go must:

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
without a random seed gets a fresh one, as Go would draw, recorded as `seed`.

With --batch, the requests of one Best Gear, stat weights or ranking job are decided
together, so their small differences never come from two engines. Every request is
prepared and checked first, and Rust runs only when the gate supports all of them:

rust      Every request ran in Rust. Each entry of `requests` names its result.json.
fallback  The gate refused at least one request: run the whole batch in Go. The refused
          entries carry their refusals; `codes` gathers the distinct codes of all of them.
          Rust runs none of the requests.
fault     A step failed for at least one request, which the entry's `stage` and `error`
          name. Run the whole batch in Go. No result.json is left for any request.

Every request without its own seed gets the same seed, --seed or one drawn for the batch,
so the candidates share their random numbers. Uses only Python's standard library.
"""

import argparse
import json
import os
from pathlib import Path
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


def run_step(stage, args, timeout, timings, accept=(0,)):
    """Run one step, recording its wall time, and return what it finished with. A failure is a
    fault at that stage; an exit status in `accept` is not a failure."""
    started = time.perf_counter()
    try:
        done = subprocess.run([str(arg) for arg in args], capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise Fault(stage, f"timed out after {timeout} seconds")
    except OSError as error:
        raise Fault(stage, str(error))
    finally:
        timings[stage] = round((time.perf_counter() - started) * 1000, 1)
    if done.returncode not in accept:
        # A negative status is the signal that ended the process, for example a kill.
        reason = f"killed by signal {-done.returncode}" if done.returncode < 0 else f"exit status {done.returncode}"
        raise Fault(stage, done.stderr.strip()[-2000:] or reason)
    return done


def write_atomically(path, text):
    """Write a file so a reader sees all of it or none: a sibling file renamed over the path."""
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    try:
        temporary.write_text(text)
        os.replace(temporary, path)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


def discard_partial_output(output):
    """Remove what a failed Rust step may have left: its report and the engine's temporary files."""
    for path in [output / "rust-report.json", *output.glob(".rust-report.json.*.tmp")]:
        path.unlink(missing_ok=True)


def draw_seed():
    """A fresh random seed from 1 to MAX_SEED. MAX_SEED is a power of two, so eight random bytes
    reduce to it without bias."""
    return int.from_bytes(os.urandom(8), "big") % MAX_SEED + 1


def seed_request(request, seed):
    """The request's own seed, or the given or a fresh one written into it."""
    options = request.setdefault("simOptions", {})
    own = int(options.get("randomSeed", 0) or 0)
    if own != 0:
        return own, "request"
    if seed is None:
        seed, source = draw_seed(), "drawn"
    else:
        source = "argument"
    options["randomSeed"] = str(seed)
    return seed, source


def bundle_manifest(bundle):
    manifest = bundle / "manifest.json"
    return json.loads(manifest.read_text()) if manifest.exists() else None


def prepare(request_path, output, bundle, timeout, seed, decision):
    """Seed and prepare one request into `decision`. Returns the prepared file. Raises Fault."""
    exporter = bundle / "bin" / EXPORTER
    try:
        request = json.loads(Path(request_path).read_text())
        decision["seed"], decision["seed_source"] = seed_request(request, seed)
    except (OSError, ValueError, AttributeError, TypeError) as error:
        raise Fault("request", f"{type(error).__name__}: {error}")
    request_file = output / "request.json"
    request_file.write_text(json.dumps(request) + "\n")
    prepared = output / "prepared.json"
    run_step("prepare", [exporter, "prepare", "--infile", request_file, "--outfile", prepared,
                         "--scenario", "route"], timeout, decision["timings_ms"])
    return prepared


def refuse(coverage_text, decision):
    """Record the gate's refusal, as `check` and `sim --gate` print it, as a fallback in `decision`.
    Raises Fault when the output is unreadable or refuses without a reason."""
    try:
        coverage = json.loads(coverage_text)
        supported = coverage["supported"]
        refusals = coverage["refusals"]
    except (ValueError, KeyError, TypeError) as error:
        raise Fault("check", f"unreadable gate output: {type(error).__name__}: {error}")
    if supported is True:
        return False
    if not refusals:
        raise Fault("check", "the gate refused the input without a reason")
    decision.update(status="fallback", refusals=refusals,
                    codes=sorted({refusal["code"] for refusal in refusals}))
    return True


def gate(request_path, output, bundle, timeout, seed, decision):
    """Prepare and check one request into `decision`, for a batch, whose requests are all gated
    before any runs. Returns the prepared file when the gate supports it; otherwise the decision
    holds a fallback. Raises Fault."""
    prepared = prepare(request_path, output, bundle, timeout, seed, decision)
    done = run_step("check", [bundle / "bin" / ENGINE, "check", "--infile", prepared], timeout,
                    decision["timings_ms"])
    return None if refuse(done.stdout, decision) else prepared


def run_rust(prepared, output, bundle, timeout, decision, gated=False):
    """Run a supported request in Rust and record its result in `decision`. Raises Fault and
    then leaves no partial output behind. With `gated`, the same process first gates the input,
    and a refusal is recorded in `decision` as a fallback, with no result."""
    try:
        run_rust_step(prepared, output, bundle, timeout, decision, gated)
    except Fault:
        discard_partial_output(output)
        raise


# The exit statuses of `forever-engine sim --gate` besides success and error: the gate
# refused the input, or the prepared input failed validation.
GATE_REFUSED, GATE_REJECTED = 3, 4


def run_rust_step(prepared, output, bundle, timeout, decision, gated):
    report_file = output / "rust-report.json"
    command = [bundle / "bin" / ENGINE, "sim", "--infile", prepared, "--outfile", report_file]
    timings = decision["timings_ms"]
    if gated:
        done = run_step("rust", command + ["--gate"], timeout, timings, accept=(0, GATE_REFUSED, GATE_REJECTED))
        if done.returncode == GATE_REJECTED:
            # What a separate `check` step reported: an input that is invalid is a fault of the gate.
            timings["check"] = timings.pop("rust")
            raise Fault("check", done.stderr.strip()[-2000:] or f"exit status {done.returncode}")
        if done.returncode == GATE_REFUSED:
            # The gate ran and nothing else did: its time is the check's, as with a separate step.
            timings["check"] = timings.pop("rust")
            if not refuse(done.stdout, decision):
                raise Fault("check", "the gate refused the input but reported it supported")
            return
    else:
        run_step("rust", command, timeout, timings)
    try:
        report = json.loads(report_file.read_text())
        result = report["result"]
        if not isinstance(result.get("raidMetrics"), dict) or result.get("iterationsDone") is None:
            raise ValueError("the result lacks raidMetrics or iterationsDone")
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        raise Fault("rust", f"incomplete result: {type(error).__name__}: {error}")
    result_file = output / "result.json"
    write_atomically(result_file, json.dumps(result) + "\n")
    decision.update(status="rust", result=str(result_file), identity=report.get("identity"),
                    request_sha256=report.get("request_sha256"))


def route(request_path, output, bundle, timeout, seed=None):
    """Decide one request and return the decision. Never raises for a failing step."""
    output.mkdir(parents=True, exist_ok=False)
    decision = {"schema": SCHEMA, "timings_ms": {}}
    manifest = bundle_manifest(bundle)
    if manifest is not None:
        decision["bundle"] = manifest
    try:
        # One engine process gates the prepared input and simulates it when it is supported.
        prepared = prepare(request_path, output, bundle, timeout, seed, decision)
        run_rust(prepared, output, bundle, timeout, decision, gated=True)
    except Fault as fault:
        decision.update(status="fault", stage=fault.stage, error=str(fault))
    return decision


def route_batch(request_paths, output, bundle, timeout, seed=None):
    """Decide a batch in one engine and return the decision. Never raises for a failing step."""
    output.mkdir(parents=True, exist_ok=False)
    source = "argument"
    if seed is None:
        seed, source = draw_seed(), "drawn"
    decision = {"schema": SCHEMA, "batch": True, "seed": seed, "seed_source": source}
    manifest = bundle_manifest(bundle)
    if manifest is not None:
        decision["bundle"] = manifest
    entries = [{"index": index, "request": str(path), "timings_ms": {}}
               for index, path in enumerate(request_paths)]
    decision["requests"] = entries
    folders = [output / f"{index:03d}" for index in range(len(entries))]

    # Gate every request before running any, so a refused candidate costs no Rust run.
    supported = []
    for entry, folder, request_path in zip(entries, folders, request_paths):
        folder.mkdir()
        try:
            prepared = gate(request_path, folder, bundle, timeout, seed, entry)
        except Fault as fault:
            entry.update(status="fault", stage=fault.stage, error=str(fault))
            continue
        if prepared is not None:
            supported.append((entry, folder, prepared))
    if len(supported) == len(entries):
        for entry, folder, prepared in supported:
            try:
                run_rust(prepared, folder, bundle, timeout, entry)
            except Fault as fault:
                entry.update(status="fault", stage=fault.stage, error=str(fault))
                break

    statuses = {entry.get("status") for entry in entries}
    if "fault" in statuses:
        # A batch is served whole or not at all: never leave a result of a failed batch.
        for entry in entries:
            if entry.get("status") == "rust":
                Path(entry.pop("result")).unlink()
                entry["status"] = "discarded"
        decision["status"] = "fault"
    elif "fallback" in statuses:
        decision.update(status="fallback",
                        codes=sorted({code for entry in entries for code in entry.get("codes", [])}))
    else:
        decision["status"] = "rust"
    return decision


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    requests = parser.add_mutually_exclusive_group(required=True)
    requests.add_argument("--request", type=Path, help="RaidSimRequest JSON")
    requests.add_argument("--batch", type=Path, nargs="+", metavar="REQUEST",
                          help="the RaidSimRequest JSON files of one batch job, decided together")
    parser.add_argument("--output", type=Path, required=True, help="new folder to write")
    parser.add_argument("--bundle", type=Path, default=Path(__file__).resolve().parents[1],
                        help="bundle folder (default: the one holding this tool)")
    parser.add_argument("--seed", type=int,
                        help="seed for an unseeded request (default: a fresh one, shared by a batch)")
    parser.add_argument("--timeout", type=int, default=300, help="seconds per step")
    args = parser.parse_args()
    if args.seed is not None and not 0 < args.seed <= MAX_SEED:
        parser.error(f"--seed must be from 1 to {MAX_SEED}")
    if args.output.exists():
        parser.error(f"{args.output} exists; each request writes a new folder")
    if args.batch:
        decision = route_batch(args.batch, args.output, args.bundle.resolve(), args.timeout, args.seed)
    else:
        decision = route(args.request, args.output, args.bundle.resolve(), args.timeout, args.seed)
    write_atomically(args.output / "decision.json", json.dumps(decision, indent=2) + "\n")
    print(json.dumps(decision))
    return 2 if decision["status"] == "fault" else 0


if __name__ == "__main__":
    sys.exit(main())
