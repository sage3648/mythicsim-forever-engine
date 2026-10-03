#!/usr/bin/env python3
"""Audit, capture and compare prepared v2 fixtures against the pinned Go engine.

check   Offline audit: manifest, file digests and exporter identity. Needs Python only.
capture Rebuild the exporter in scratch, re-export every case into --output and compare
        with the accepted fixtures. Never writes accepted files.
"""

import argparse
import hashlib
import json
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
    (output / "comparison.json").write_text(json.dumps({"differences": differences}, indent=2) + "\n")
    if differences:
        raise ValueError(f"re-exported cases differ: {', '.join(differences)}; review {output}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["check", "capture"], nargs="?", default="check")
    parser.add_argument("--source", default="https://github.com/sage3648/mythicsim-forever-engine-go.git",
                        help="local Git repository or clone URL")
    parser.add_argument("--cache", type=Path, default=ROOT / "oracle-cache")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
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
