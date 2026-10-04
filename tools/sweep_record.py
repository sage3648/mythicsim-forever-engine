#!/usr/bin/env python3
"""Write a compatibility sweep record from a `prepared_v2.py compare` run.

Reads the compare output directory's summary and each variant request, and writes the
validation record: reference pin, generator and rerun commands, criteria, outcome
counts and one row per variant. A variant whose Rust run refused the input is counted
as rejected, with its reasons; any other failure is a mismatch. Uses only Python's
standard library.
"""

import argparse
from datetime import date
import json
from pathlib import Path

from compare import CLIENT_BUILD, PIN, ROOT, load

CRITERIA = {
    "comparison": "full RaidSimResult except elapsedNs, lists keyed by ID",
    "integer_counts": "exact",
    "floats": "relative 1e-9",
    "deviations": "as variances at the scale of mean^2",
    "first_fight_debug_log": "line identical",
}


def relative(path):
    path = Path(path).resolve()
    return str(path.relative_to(ROOT)) if ROOT in path.parents else str(path)


def row(request_path, result, prepared):
    request = load(request_path)
    player = request["raid"]["parties"][0]["players"][0]
    encounter = request["encounter"]
    out = {
        "scenario": result["scenario"],
        "race": player.get("race"),
        "request_sha256": prepared.get("request_sha256") if prepared else None,
        "iterations": int(request["simOptions"]["iterations"]),
        "labeled_rng": bool(request["simOptions"].get("useLabeledRands", False)),
        "duration_s": encounter.get("duration"),
        "duration_variation_s": encounter.get("durationVariation", 0),
        "target_level": encounter["targets"][0].get("level"),
        "talents": player.get("talentsString"),
        "equipped_items": sum(1 for item in player["equipment"]["items"] if item),
        "go_dps": result.get("go_dps"),
        "rust_dps": result.get("rust_dps"),
        "passed": result["passed"],
    }
    if not result["passed"]:
        if result.get("rust_error", "").startswith("prepared input unsupported"):
            out["rejected"] = [line.strip() for line in result["rust_error"].splitlines()[1:]]
        else:
            out["differences"] = result.get("differences", [])[:20]
            out["first_log_difference"] = result.get("first_log_difference")
    return out


def record(compare_dir, requests, scope, generator, notes):
    summary = load(compare_dir / "summary.json")
    by_scenario = {Path(path).name.removesuffix(".json").removesuffix(".request"): path for path in requests}
    cases = []
    for result in summary["results"]:
        prepared_path = compare_dir / result["scenario"] / "prepared.json"
        prepared = load(prepared_path) if prepared_path.is_file() else None
        cases.append(row(by_scenario[result["scenario"]], result, prepared))
    rejected = sum(1 for case in cases if "rejected" in case)
    matched = sum(1 for case in cases if case["passed"])
    directories = sorted({relative(Path(path).parent) for path in requests})
    return {
        "kind": "compatibility_sweep",
        "date": date.today().isoformat(),
        "reference": {"engine_revision": PIN, "client_build": CLIENT_BUILD},
        "scope": scope,
        "generator": generator,
        "rerun": "python3 tools/prepared_v2.py compare --output <scratch> "
                 + " ".join(f"{directory}/*.json" for directory in directories),
        "criteria": CRITERIA,
        "matched": matched,
        "rejected": rejected,
        "mismatched": len(cases) - matched - rejected,
        "notes": notes,
        "cases": cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("compare_dir", type=Path, help="the --output directory of a compare run")
    parser.add_argument("requests", nargs="+", type=Path, help="the variant requests that run compared")
    parser.add_argument("--scope", required=True)
    parser.add_argument("--generator", required=True, help="the command that generated the variants")
    parser.add_argument("--note", action="append", default=[])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    value = record(args.compare_dir, args.requests, args.scope, args.generator, args.note)
    args.output.write_text(json.dumps(value, indent=2) + "\n")
    print(f"{value['matched']} matched, {value['rejected']} rejected, {value['mismatched']} mismatched")
    return 0 if value["mismatched"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
