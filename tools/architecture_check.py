#!/usr/bin/env python3
"""Check the accepted Go goldens against the pinned Go engine built for another architecture.

The goldens come from the pinned Go engine built for the machine that accepted them. Go's
arm64 compiler fuses some multiply and add steps into one rounding, and Rust follows that
with mul_add, so the engine on another architecture may differ in the last bits. This
cross-builds the pinned exporter for linux/GOARCH, runs every accepted case that has a Go
result in a Docker container of that platform, and compares each result and first-fight
log with the golden exactly as `prepared_v2.py` derives them. It writes a validation record
to --record: every case matches, mismatches with its first differences, or errors.

Needs Go, Docker and the oracle cache `prepared_v2.py` builds. Another architecture than the
host's runs under emulation, which is slower.
"""

import argparse
from datetime import date
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import prepared_v2
from compare import CLIENT_BUILD, PIN, ROOT, go_pin_flags, load

RENAMED = {"go": "golden", "rust": "platform", "go_context": "golden_context",
           "rust_context": "platform_context", "go_lines": "golden_lines", "rust_lines": "platform_lines"}

def cross_build(cache, source, goarch, output):
    """The pinned exporter built for linux/goarch, from the checkout prepared_v2 keeps."""
    prepared_v2.build_exporter(cache, source)
    binary = output / f"forever-go-oracle-v2-linux-{goarch}"
    environment = dict(os.environ, GOOS="linux", GOARCH=goarch, CGO_ENABLED="0")
    subprocess.run(["go", "build", "-trimpath", "--tags=with_db", *go_pin_flags(), "-o", str(binary),
                    "./cmd/mythicsim-rust-oracle-v2"], cwd=cache / "source", env=environment, check=True)
    return binary


def run_cases(binary, goarch, image, cases, output):
    """Run every case's request in one container. Each writes its result or an error file."""
    work = output / "work"
    (work / "requests").mkdir(parents=True)
    (work / "results").mkdir()
    shutil.copy2(binary, work / "oracle")
    for case in cases:
        (work / "requests" / f"{case['id']}.json").write_text(json.dumps(prepared_v2.request_of(case)))
    script = ("for request in /work/requests/*.json; do name=$(basename \"$request\"); "
              "/work/oracle sim --infile \"$request\" --outfile \"/work/results/$name\" "
              "2> \"/work/results/$name.err\" || echo failed >> \"/work/results/$name.err\"; done")
    subprocess.run(["docker", "run", "--rm", "--platform", f"linux/{goarch}", "--entrypoint", "sh",
                    "-v", f"{work}:/work", image, "-c", script], check=True)
    return work / "results"


def compare_case(case, results, family):
    """The case's outcome against its accepted goldens."""
    result_path = results / f"{case['id']}.json"
    error_path = results / f"{case['id']}.json.err"
    error = error_path.read_text().strip() if error_path.exists() else ""
    if not result_path.exists() or "failed" in error.splitlines():
        return {"id": case["id"], "error": error[-2000:] or "no result"}
    result = load(result_path)
    differences = prepared_v2.leaf_differences(load(family / case["go_result"]["file"]),
                                               prepared_v2.comparable(result))
    row = {"id": case["id"], "matched": not differences}
    if "go_log" in case:
        golden_log = (family / case["go_log"]["file"]).read_text()
        # The accepted log leaves out the stat recalculation lines, as go_golden does.
        lines = [line for line in result.get("logs", "").splitlines()
                 if not any(skip in line for skip in prepared_v2.SKIPPED_LOG_LINES)]
        log = prepared_v2.first_log_difference(golden_log, "".join(line + "\n" for line in lines))
        if log is not None:
            row["matched"] = False
            # first_log_difference names its sides go and rust; here they are the accepted
            # golden and this platform's run.
            row["first_log_difference"] = {RENAMED.get(key, key): value for key, value in log.items()}
    if differences:
        row["differences"] = differences[:20]
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--goarch", required=True, choices=["arm64", "amd64"])
    parser.add_argument("--image", required=True,
                        help="a local Docker image of linux/GOARCH with a shell, ideally the production "
                             "engine's own, so the check runs where production does")
    parser.add_argument("--output", type=Path, required=True, help="new scratch folder")
    parser.add_argument("--record", type=Path, help="validation record to write")
    parser.add_argument("--scope", default="", help="the record's scope")
    parser.add_argument("--case", action="append", default=[], help="check only these cases")
    parser.add_argument("--source", default="https://github.com/sage3648/mythicsim-forever-engine-go.git")
    parser.add_argument("--cache", type=Path, default=ROOT / "oracle-cache")
    args = parser.parse_args()
    if args.output.exists():
        parser.error(f"{args.output} exists; the check writes a new folder")
    args.output.mkdir(parents=True)
    family = prepared_v2.FAMILY
    cases = [case for case in prepared_v2.check(family)["cases"]
             if "go_result" in case and (not args.case or case["id"] in args.case)]
    binary = cross_build(args.cache.resolve(), args.source, args.goarch, args.output.resolve())
    results = run_cases(binary, args.goarch, args.image, cases, args.output.resolve())
    rows = [compare_case(case, results, family) for case in cases]
    matched = sum(1 for row in rows if row.get("matched"))
    errors = sum(1 for row in rows if "error" in row)
    record = {
        "kind": "architecture_check",
        "date": date.today().isoformat(),
        "reference": {"engine_revision": PIN, "client_build": CLIENT_BUILD},
        "scope": args.scope or f"Every accepted Go golden against the pinned Go engine built for linux/{args.goarch}.",
        "platform": f"linux/{args.goarch}",
        "image": args.image,
        "compiler": subprocess.check_output(["go", "version"], text=True).strip(),
        "rerun": f"python3 tools/architecture_check.py --goarch {args.goarch} --image {args.image} "
                 "--output <scratch>",
        "criteria": {"comparison": "the comparable result and the filtered first-fight log, as prepared_v2.py accepts them"},
        "matched": matched,
        "mismatched": len(rows) - matched - errors,
        "errors": errors,
        "cases": rows,
    }
    if args.record:
        args.record.write_text(json.dumps(record, indent=2) + "\n")
    for row in rows:
        if not row.get("matched"):
            print(f"{row['id']}: {row.get('error') or row.get('differences', [])[:3] or row.get('first_log_difference')}")
    print(f"linux/{args.goarch}: {matched} matched, {record['mismatched']} mismatched, {errors} errors "
          f"of {len(rows)} cases")
    return 0 if matched == len(rows) else 1


if __name__ == "__main__":
    sys.exit(main())
