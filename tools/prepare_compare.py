#!/usr/bin/env python3
"""Prepare requests both in Rust and with the pinned Go exporter, and compare the states.

Every request Rust prepares must give exactly the exporter's prepared state; the others must be
refused with a code. The report counts the matches, the refusals by code and every mismatch with
its first differing paths, and writes it to --output/report.json. Exit status 1 when any request
was prepared differently or Rust preparation failed.

usage: python3 tools/prepare_compare.py --output NEW_FOLDER [--engine BIN] [--exporter BIN] [--jobs N] REQUESTS_OR_DIRS...

The engine defaults to target/release/forever-engine and the exporter to the one
tools/prepared_v2.py builds into oracle-cache/. Uses only Python's standard library.
"""

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
NOT_PREPARED = 5
MAX_DIFFERENCES = 12


def differences(go, rust, path="", out=None):
    """Every path at which two prepared states differ, with no tolerance."""
    out = [] if out is None else out
    if len(out) >= MAX_DIFFERENCES:
        return out
    if isinstance(go, dict) and isinstance(rust, dict):
        for key in sorted(set(go) | set(rust)):
            if key not in go or key not in rust:
                out.append(f"{path}/{key}: only in {'Go' if key in go else 'Rust'}")
            else:
                differences(go[key], rust[key], f"{path}/{key}", out)
    elif isinstance(go, list) and isinstance(rust, list):
        if len(go) != len(rust):
            out.append(f"{path}: {len(go)} Go entries, {len(rust)} Rust entries")
        for i, (a, b) in enumerate(zip(go, rust)):
            differences(a, b, f"{path}/{i}", out)
    elif isinstance(go, (int, float)) and isinstance(rust, (int, float)) and not isinstance(go, bool) and not isinstance(rust, bool):
        if float(go) != float(rust):
            out.append(f"{path}: Go {go!r}, Rust {rust!r}")
    elif go != rust:
        out.append(f"{path}: Go {json.dumps(go)[:120]}, Rust {json.dumps(rust)[:120]}")
    return out


def requests(paths):
    for path in paths:
        if path.is_dir():
            yield from sorted(p for p in path.rglob("*.json") if not p.name.endswith(".prepared.json"))
        else:
            yield path


def compare(request, engine, exporter, folder):
    """One request's verdict: match, mismatch, refused (with its code) or error."""
    go_file, rust_file = folder / "go.json", folder / "rust.json"
    go = subprocess.run([exporter, "prepare", "--infile", request, "--outfile", go_file, "--scenario", "compare"],
                        capture_output=True, text=True)
    rust = subprocess.run([engine, "prepare", "--request", request, "--outfile", rust_file, "--scenario", "compare"],
                          capture_output=True, text=True)
    if rust.returncode == NOT_PREPARED:
        refusal = json.loads(rust.stdout)["refusal"]
        return {"status": "refused", "code": refusal["code"], "reason": refusal["reason"]}
    if rust.returncode != 0:
        return {"status": "error", "stage": "rust", "error": rust.stderr.strip()[-1000:]}
    if go.returncode != 0:
        return {"status": "error", "stage": "go", "error": go.stderr.strip()[-1000:]}
    found = differences(json.loads(go_file.read_text()), json.loads(rust_file.read_text()))
    if found:
        return {"status": "mismatch", "differences": found}
    return {"status": "match"}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("paths", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--engine", type=Path, default=ROOT / "target" / "release" / "forever-engine")
    parser.add_argument("--exporter", type=Path, default=ROOT / "oracle-cache" / "forever-go-oracle-v2")
    parser.add_argument("--jobs", type=int, default=1, help="requests compared at a time")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)

    def one(item):
        index, request = item
        folder = args.output / f"{index:05d}"
        folder.mkdir()
        verdict = compare(request, args.engine, args.exporter, folder)
        if verdict["status"] == "match":
            for name in ("go.json", "rust.json"):
                (folder / name).unlink(missing_ok=True)
            folder.rmdir()
        return str(request), verdict

    with ThreadPoolExecutor(max(args.jobs, 1)) as pool:
        verdicts = dict(pool.map(one, enumerate(requests(args.paths))))
    statuses = Counter(verdict["status"] for verdict in verdicts.values())
    codes = Counter(verdict["code"] for verdict in verdicts.values() if verdict["status"] == "refused")
    report = {"requests": len(verdicts), "statuses": dict(statuses), "refusal_codes": dict(codes), "verdicts": verdicts}
    (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ("requests", "statuses", "refusal_codes")}))
    return 1 if statuses["mismatch"] or statuses["error"] else 0


if __name__ == "__main__":
    sys.exit(main())
