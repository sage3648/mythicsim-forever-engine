#!/usr/bin/env python3
"""Audit, capture and compare prepared v2 fixtures against the pinned Go engine.

check   Offline audit: manifest, file digests and exporter identity. Needs Python only.
capture Rebuild the exporter in scratch, re-export every case into --output and compare
        with the accepted fixtures. Never writes accepted files.
compare Export requests with the pinned Go engine, run Go and Rust on each and compare every
        exercised metric and the first-fight debug log. Writes only to --output.
accept  Maintainer command: register a new case and derive its Go goldens. Refuses to
        replace existing cases or files.
refresh Maintainer command after a reviewed exporter change: re-export prepared inputs
        and require every Go golden to stay byte-identical.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import subprocess
import sys

from compare import ROOT, PIN, build_oracle, command, load

FAMILY = ROOT / "fixtures" / "mage" / "frost" / "prepared-v2"
HELPER = ROOT / "tools" / "oracle-v2" / "main.go"


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def request_of(case):
    source = case["request"]
    value = load(ROOT / source["path"])
    for part in [p for p in source.get("pointer", "").split("/") if p]:
        value = value[part]
    return value


def check(family=FAMILY):
    manifest = load(family / "manifest.json")
    errors = []

    def require(condition, message):
        if not condition:
            errors.append(message)

    require(manifest["schema_version"] == 1, "unknown manifest schema")
    require(manifest["prepared_schema_version"] == 2, "family must contain prepared v2 inputs")
    require(manifest["engine_revision"] == PIN, "engine revision differs from the comparison pin")
    require(manifest["exporter_sha256"] == digest(HELPER),
            "tools/oracle-v2/main.go changed: recapture and review the prepared fixtures")
    ids = [case["id"] for case in manifest["cases"]]
    require(len(ids) == len(set(ids)), "duplicate case identifiers")
    for case in manifest["cases"]:
        prepared_path = family / case["prepared"]
        require(prepared_path.is_file(), f"missing prepared file for {case['id']}")
        if not prepared_path.is_file():
            continue
        require(digest(prepared_path) == case["prepared_sha256"], f"changed prepared file: {case['id']}")
        prepared = load(prepared_path)
        require(prepared["scenario_id"] == case["id"], f"scenario identifier differs: {case['id']}")
        require(prepared["reference"]["engine_revision"] == PIN, f"reference revision differs: {case['id']}")
        require(prepared["unrepresented"] == [], f"unrepresented features in accepted case {case['id']}")
        for key in ("go_result", "go_log"):
            if key in case:
                path = family / case[key]["file"]
                require(path.is_file() and digest(path) == case[key]["sha256"], f"changed {key} for {case['id']}")
                require(case["expected_coverage"]["supported"], f"{case['id']}: goldens need a supported case")
        request = request_of(case)
        require(int(request["simOptions"]["iterations"]) == prepared["sim"]["iterations"],
                f"iterations differ from the request: {case['id']}")
        require(str(request["simOptions"].get("randomSeed", "0")) == str(prepared["sim"]["seed"]),
                f"seed differs from the request: {case['id']}")
    if errors:
        raise ValueError("\n".join(errors))
    return manifest


def build_exporter(cache, source):
    """Reuse the v1 oracle checkout, generated protos and pin checks; add the v2 helper."""
    build_oracle(cache, source)
    checkout = cache / "source"
    identity = {"revision": PIN, "helper_sha256": digest(HELPER),
                "compiler": subprocess.check_output(["go", "version"], text=True).strip()}
    binary = cache / "forever-go-oracle-v2"
    stamp = cache / "build-v2.json"
    if binary.exists() and stamp.exists() and load(stamp) == identity:
        return binary
    target = checkout / "cmd" / "mythicsim-rust-oracle-v2"
    target.mkdir(exist_ok=True)
    shutil.copy2(HELPER, target / "main.go")
    command(["go", "build", "-trimpath", "--tags=with_db", "-o", binary, "./cmd/mythicsim-rust-oracle-v2"], cwd=checkout)
    stamp.write_text(json.dumps(identity, indent=2) + "\n")
    return binary


def go_golden(exporter, request_path, directory):
    """Run the pinned Go engine and return its compact result and filtered first-fight log."""
    directory.mkdir(parents=True, exist_ok=True)
    result_path = directory / "go-result.raw.json"
    command([exporter, "sim", "--infile", request_path, "--outfile", result_path])
    result = load(result_path)
    lines = [line for line in result.get("logs", "").splitlines()
             if not any(skip in line for skip in SKIPPED_LOG_LINES)]
    return compact(result), "".join(line + "\n" for line in lines)


def accept(cache, source, case_id, request, description, keep_log, family=FAMILY):
    """Register a new accepted case. Never replaces an existing case or file."""
    manifest = check(family)
    if any(case["id"] == case_id for case in manifest["cases"]):
        raise ValueError(f"case {case_id} already exists")
    request = request.resolve()
    if family.resolve() not in request.parents:
        raise ValueError("place the request in the fixture family first")
    prepared_path = family / f"{case_id}.prepared.json"
    if prepared_path.exists():
        raise ValueError(f"{prepared_path} already exists")
    exporter = build_exporter(cache.resolve(), source)
    command([exporter, "prepare", "--infile", request, "--outfile", prepared_path, "--scenario", case_id])
    completed = subprocess.run(["cargo", "run", "--locked", "--quiet", "--manifest-path", ROOT / "Cargo.toml", "--",
                                "check", "--infile", prepared_path], capture_output=True, text=True, check=True)
    coverage = json.loads(completed.stdout)
    case = {"id": case_id, "description": description,
            "request": {"path": str(request.relative_to(ROOT))},
            "prepared": prepared_path.name, "prepared_sha256": digest(prepared_path),
            "expected_coverage": {"supported": coverage["supported"], "reasons": coverage["reasons"]}}
    if coverage["supported"]:
        import tempfile
        with tempfile.TemporaryDirectory() as scratch:
            result, log = go_golden(exporter, request, Path(scratch))
        result_path = family / f"{case_id}.go-result.json"
        result_path.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
        case["go_result"] = {"file": result_path.name, "sha256": digest(result_path)}
        if keep_log:
            log_path = family / f"{case_id}.go-log.txt"
            log_path.write_text(log)
            case["go_log"] = {"file": log_path.name, "sha256": digest(log_path)}
    manifest["cases"].append(case)
    (family / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return case


def refresh(cache, source, family=FAMILY):
    """Maintainer command after a reviewed exporter change: re-export accepted prepared
    inputs, record their digests and require every Go golden to stay byte-identical."""
    manifest = load(family / "manifest.json")
    exporter = build_exporter(cache.resolve(), source)
    import tempfile
    with tempfile.TemporaryDirectory() as scratch:
        scratch = Path(scratch)
        for case in manifest["cases"]:
            request_path = scratch / f"{case['id']}.request.json"
            request_path.write_text(json.dumps(request_of(case), indent=2) + "\n")
            if "go_result" in case:
                result, log = go_golden(exporter, request_path, scratch / case["id"])
                if result != load(family / case["go_result"]["file"]):
                    raise ValueError(f"{case['id']}: the Go result changed; this is not an exporter-only change")
                if "go_log" in case and log != (family / case["go_log"]["file"]).read_text():
                    raise ValueError(f"{case['id']}: the Go log changed; this is not an exporter-only change")
            prepared_path = family / case["prepared"]
            command([exporter, "prepare", "--infile", request_path, "--outfile", prepared_path, "--scenario", case["id"]])
            case["prepared_sha256"] = digest(prepared_path)
    manifest["exporter_sha256"] = digest(HELPER)
    (family / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def capture(cache, source, output, family=FAMILY):
    manifest = check(family)
    output = output.resolve()
    if output == family.resolve() or family.resolve() in output.parents:
        raise ValueError("capture must use scratch storage, not accepted fixtures")
    output.mkdir(parents=True, exist_ok=False)
    exporter = build_exporter(cache.resolve(), source)
    differences = []
    for case in manifest["cases"]:
        request_path = output / f"{case['id']}.request.json"
        request_path.write_text(json.dumps(request_of(case), indent=2) + "\n")
        prepared_path = output / case["prepared"]
        command([exporter, "prepare", "--infile", request_path, "--outfile", prepared_path,
                 "--scenario", case["id"]])
        if load(prepared_path) != load(family / case["prepared"]):
            differences.append(case["id"])
        if "go_result" in case:
            result, log = go_golden(exporter, request_path, output / case["id"])
            if result != load(family / case["go_result"]["file"]):
                differences.append(f"{case['id']} Go result")
            if "go_log" in case and log != (family / case["go_log"]["file"]).read_text():
                differences.append(f"{case['id']} Go log")
    (output / "comparison.json").write_text(json.dumps({"differences": differences}, indent=2) + "\n")
    if differences:
        raise ValueError(f"re-exported cases differ: {', '.join(differences)}; review {output}")


TOLERANCE = 1e-9
# Go log lines Rust does not reproduce: stat recalculation details from preparation.
SKIPPED_LOG_LINES = ("Dynamic stat change:", "Dynamic dep enabled", "Dynamic dep disabled")


def id_key(identity):
    return json.dumps(identity, sort_keys=True)


def exercised(row):
    return any(isinstance(v, (int, float)) and v != 0 and k != "unitIndex"
               for target in row.get("targets", []) for k, v in target.items())


def compact(result):
    """The comparable view of a Go RaidSimResult or a Rust prepared report result."""
    player = result["raidMetrics"]["parties"][0]["players"][0]
    target = result["encounterMetrics"]["targets"][0]

    def distribution(value):
        return {k: value.get(k, 0) for k in ("avg", "stdev", "max", "min")}

    return {
        "summary": {
            "iterationsDone": result.get("iterationsDone", 0),
            "avgIterationDuration": result.get("avgIterationDuration", 0),
            "firstIterationDuration": result.get("firstIterationDuration", 0),
            "dps": distribution(player.get("dps", {})),
            "threat": distribution(player.get("threat", {})),
            "secondsOomAvg": player.get("secondsOomAvg", 0),
        },
        "actions": {id_key(a["id"]): a["targets"] for a in player.get("actions", []) if exercised(a)},
        "auras": {id_key(a["id"]): {k: a.get(k, 0) for k in ("uptimeSecondsAvg", "uptimeSecondsStdev", "procsAvg")}
                  for a in player.get("auras", []) if a.get("procsAvg", 0) > 0},
        "target_auras": {id_key(a["id"]): {k: a.get(k, 0) for k in ("uptimeSecondsAvg", "uptimeSecondsStdev", "procsAvg")}
                         for a in target.get("auras", []) if a.get("procsAvg", 0) > 0},
        "resources": {id_key(r["id"]): {k: r.get(k, 0) for k in ("events", "gain", "actualGain")}
                      for r in player.get("resources", []) if r.get("events", 0) > 0},
    }


def leaf_differences(go, rust, path=""):
    if isinstance(go, dict) or isinstance(rust, dict):
        go, rust = go if isinstance(go, dict) else {}, rust if isinstance(rust, dict) else {}
        differences = []
        for key in sorted(set(go) | set(rust)):
            if key not in go or key not in rust:
                differences.append(f"{path}/{key}: only in {'Go' if key in go else 'Rust'}")
            else:
                differences.extend(leaf_differences(go[key], rust[key], f"{path}/{key}"))
        return differences
    if isinstance(go, list) or isinstance(rust, list):
        go, rust = list(go or []), list(rust or [])
        if len(go) != len(rust):
            return [f"{path}: {len(go)} Go entries, {len(rust)} Rust entries"]
        return [d for i, (a, b) in enumerate(zip(go, rust)) for d in leaf_differences(a, b, f"{path}/{i}")]
    if isinstance(go, int) and isinstance(rust, int) and not isinstance(go, bool):
        return [] if go == rust else [f"{path}: Go {go}, Rust {rust}"]
    if isinstance(go, (int, float)) and isinstance(rust, (int, float)):
        scale = max(1.0, abs(go), abs(rust))
        if math.isfinite(go) and math.isfinite(rust) and abs(go - rust) <= TOLERANCE * scale:
            return []
        return [f"{path}: Go {go!r}, Rust {rust!r}"]
    return [] if go == rust else [f"{path}: Go {go!r}, Rust {rust!r}"]


def first_log_difference(go_logs, rust_logs):
    go = [line for line in go_logs.splitlines() if not any(skip in line for skip in SKIPPED_LOG_LINES)]
    rust = rust_logs.splitlines()
    for index, (a, b) in enumerate(zip(go, rust)):
        if a != b:
            return {"line": index + 1, "go": a, "rust": b, "go_context": go[max(0, index - 5):index + 3],
                    "rust_context": rust[max(0, index - 5):index + 3]}
    if len(go) != len(rust):
        index = min(len(go), len(rust))
        return {"line": index + 1, "go": go[index] if index < len(go) else None,
                "rust": rust[index] if index < len(rust) else None, "go_lines": len(go), "rust_lines": len(rust)}
    return None


def compare_request(exporter, rust, request_path, scenario, directory):
    directory.mkdir(parents=True, exist_ok=True)
    prepared = directory / "prepared.json"
    command([exporter, "prepare", "--infile", request_path, "--outfile", prepared, "--scenario", scenario])
    go_out, rust_out = directory / "go.json", directory / "rust.json"
    command([exporter, "sim", "--infile", request_path, "--outfile", go_out])
    completed = subprocess.run([str(rust), "sim", "--infile", str(prepared), "--outfile", str(rust_out)],
                               capture_output=True, text=True, timeout=600)
    if completed.returncode != 0:
        return {"scenario": scenario, "passed": False, "rust_error": completed.stderr.strip()}
    go_result, rust_report = load(go_out), load(rust_out)
    differences = leaf_differences(compact(go_result), compact(rust_report["result"]))
    log_difference = None
    if go_result.get("logs") or rust_report["result"].get("logs"):
        log_difference = first_log_difference(go_result.get("logs", ""), rust_report["result"].get("logs", ""))
    row = {"scenario": scenario, "passed": not differences and log_difference is None,
           "differences": differences, "first_log_difference": log_difference,
           "go_dps": compact(go_result)["summary"]["dps"]["avg"],
           "rust_dps": compact(rust_report["result"])["summary"]["dps"]["avg"]}
    (directory / "comparison.json").write_text(json.dumps(row, indent=2) + "\n")
    return row


def compare_cases(cache, source, output, requests):
    output = output.resolve()
    if FAMILY.resolve() == output or FAMILY.resolve() in output.parents:
        raise ValueError("compare must use scratch storage")
    exporter = build_exporter(cache.resolve(), source)
    command(["cargo", "build", "--locked", "--release", "--manifest-path", ROOT / "Cargo.toml"])
    rust = ROOT / "target" / "release" / "forever-engine"
    rows = []
    for path in requests:
        scenario = Path(path).name.removesuffix(".json").removesuffix(".request").removesuffix(".go")
        row = compare_request(exporter, rust, Path(path).resolve(), scenario, output / scenario)
        rows.append(row)
        status = "PASS" if row["passed"] else "FAIL"
        print(f"{status} {scenario} Go={row.get('go_dps')} Rust={row.get('rust_dps')}", flush=True)
        for difference in row.get("differences", [])[:20]:
            print(f"  {difference}", flush=True)
        if row.get("first_log_difference"):
            first = row["first_log_difference"]
            print(f"  log line {first['line']}:\n    Go:   {first['go']}\n    Rust: {first['rust']}", flush=True)
        if row.get("rust_error"):
            print(f"  Rust: {row['rust_error']}", flush=True)
    (output / "summary.json").write_text(json.dumps({"passed": all(r["passed"] for r in rows), "results": rows}, indent=2) + "\n")
    return all(row["passed"] for row in rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["check", "capture", "compare", "accept", "refresh"], nargs="?", default="check")
    parser.add_argument("--case", help="accept: new case identifier")
    parser.add_argument("--description", help="accept: what the case covers")
    parser.add_argument("--keep-log", action="store_true", help="accept: keep the Go first-fight log golden")
    parser.add_argument("requests", nargs="*", type=Path, help="RaidSimRequest files to compare")
    parser.add_argument("--source", default="https://github.com/sage3648/mythicsim-forever-engine-go.git",
                        help="local Git repository or clone URL")
    parser.add_argument("--cache", type=Path, default=ROOT / "oracle-cache")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "refresh":
            manifest = refresh(args.cache, args.source)
            print(f"Refreshed {len(manifest['cases'])} prepared inputs; Go goldens unchanged.")
            return 0
        if args.command == "accept":
            if not (args.case and args.description and len(args.requests) == 1):
                parser.error("accept needs --case, --description and one request")
            case = accept(args.cache, args.source, args.case, args.requests[0], args.description, args.keep_log)
            print(f"Accepted {case['id']}: supported={case['expected_coverage']['supported']}")
            return 0
        if args.command == "compare":
            if args.output is None or not args.requests:
                parser.error("compare needs --output and at least one request")
            return 0 if compare_cases(args.cache, args.source, args.output, args.requests) else 1
        if args.command == "capture":
            if args.output is None:
                parser.error("capture needs --output")
            capture(args.cache, args.source, args.output)
            print(f"Re-exported prepared v2 cases match the accepted fixtures in {args.output}")
        else:
            manifest = check()
            print(f"Prepared v2 fixtures checked: {len(manifest['cases'])} cases.")
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"prepared v2 fixtures failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
