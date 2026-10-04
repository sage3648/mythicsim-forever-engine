#!/usr/bin/env python3
"""Shadow comparison of one production request against the pinned Go engine.

build  Build a self-contained bundle in --output: the release Rust engine, the pinned
       Go exporter, these tools, the pin and a manifest of revisions. The application's
       shadow worker runs requests from this bundle, so it needs neither Go nor Cargo.
run    Compare one RaidSimRequest. The pinned exporter prepares it, the Rust gate checks
       it, then the pinned Go engine and Rust run it with the same seed. Prints one JSON
       verdict and writes it to --output/verdict.json. The verdict status is match,
       mismatch, refused (the gate's reasons) or error (the failing stage).

The status is an exact check against the pinned Go engine. With --production, the
verdict also summarizes the production result, the Rust result and the pinned Go
result (DPS and the top abilities per fight), and compares Rust with production
statistically, since a production run uses its own unseeded random numbers.
"""

import argparse
import json
import math
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time

import prepared_v2
from compare import ROOT, PIN, CLIENT_BUILD

SCHEMA = 1
ENGINE = "forever-engine"
EXPORTER = "forever-go-oracle-v2"
# The tools a bundle needs to run a comparison, by path from the repository root.
BUNDLED_TOOLS = ("tools/compare.py", "tools/prepared_v2.py", "tools/shadow.py", "upstream/sources.json")
MAX_DIFFERENCES = 20
# Abilities kept in each result summary, by damage per second.
TOP_ABILITIES = 8


class StageError(Exception):
    def __init__(self, stage, message):
        super().__init__(message)
        self.stage = stage


def run_stage(stage, args, timeout):
    """Run one step, returning its wall time in milliseconds and its output."""
    started = time.perf_counter()
    try:
        completed = subprocess.run([str(arg) for arg in args], capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise StageError(stage, f"timed out after {timeout} seconds")
    except OSError as error:
        raise StageError(stage, str(error))
    elapsed = (time.perf_counter() - started) * 1000
    return elapsed, completed


def seeded(request, seed):
    """The request with a fixed random seed, so both engines draw the same numbers. A
    request that already carries a seed keeps it."""
    options = request.setdefault("simOptions", {})
    if int(options.get("randomSeed", 0) or 0) == 0:
        options["randomSeed"] = str(seed)
    return request


def compare_results(go_result, rust_result):
    differences = prepared_v2.leaf_differences(prepared_v2.comparable(go_result),
                                               prepared_v2.comparable(rust_result))
    log_difference = None
    if go_result.get("logs") or rust_result.get("logs"):
        log_difference = prepared_v2.first_log_difference(go_result.get("logs", ""), rust_result.get("logs", ""))
    return differences, log_difference


def summary(result):
    """One result in brief: the player's DPS, the fight and the abilities that did the most
    damage, per average fight. The engine records totals over every iteration."""
    player = result["raidMetrics"]["parties"][0]["players"][0]
    iterations = result.get("iterationsDone", 0)
    fight = result.get("avgIterationDuration", 0.0)
    dps = player.get("dps", {})
    brief = {"dps": dps.get("avg", 0.0), "stdev": prepared_v2.deviation(dps.get("stdev", 0.0)),
             "min": dps.get("min"), "max": dps.get("max"), "iterations": iterations,
             "fight_seconds": fight, "abilities": [], "pets": []}
    if iterations <= 0 or fight <= 0:
        return brief
    abilities = []
    for action in player.get("actions", []):
        targets = action.get("targets", [])
        damage = sum(target.get("damage", 0.0) for target in targets)
        if damage <= 0:
            continue
        hits = sum(target.get("hits", 0) for target in targets)
        crits = sum(target.get("crits", 0) for target in targets)
        abilities.append({"id": action["id"], "dps": damage / (iterations * fight),
                          "casts": sum(target.get("casts", 0) for target in targets) / iterations,
                          "crit_pct": 100.0 * crits / (hits + crits) if hits + crits else None})
    abilities.sort(key=lambda ability: -ability["dps"])
    brief["abilities"] = abilities[:TOP_ABILITIES]
    brief["pets"] = [{"name": pet.get("name"), "dps": pet["dps"]["avg"]}
                     for pet in player.get("pets", []) if pet.get("dps", {}).get("avg", 0) > 0]
    return brief


def standard_error(brief):
    return brief["stdev"] / math.sqrt(brief["iterations"]) if brief["iterations"] > 0 else None


def ability_key(identity):
    """An action ID with its zero fields left out. The production CLI writes protojson with
    EmitUnpopulated, so its IDs carry `"tag": 0` and `"rank": 0` where Rust and the pinned
    Go result omit them."""
    return json.dumps({k: v for k, v in identity.items() if v not in (0, "", None)}, sort_keys=True)


def versus_production(production, rust):
    """Rust against the production result. The runs use different random numbers, so the
    DPS difference is given in standard errors of the difference: a few either way is
    noise, a large value is a real difference. Abilities are matched by ID."""
    errors = [standard_error(production), standard_error(rust)]
    combined = math.sqrt(sum(e * e for e in errors)) if None not in errors else 0.0
    difference = rust["dps"] - production["dps"]
    by_id = {ability_key(a["id"]): a for a in production["abilities"]}
    abilities = []
    for ability in rust["abilities"]:
        other = by_id.pop(ability_key(ability["id"]), None)
        abilities.append({"id": ability["id"], "production_dps": other["dps"] if other else None,
                          "rust_dps": ability["dps"]})
    abilities += [{"id": a["id"], "production_dps": a["dps"], "rust_dps": None} for a in by_id.values()]
    return {"dps_difference": difference,
            "dps_difference_pct": 100.0 * difference / production["dps"] if production["dps"] else None,
            "standard_errors": difference / combined if combined > 0 else None,
            "abilities": abilities}


def shadow(request_path, output, bundle, seed, timeout, production_path=None):
    """Compare one request and return the verdict. Never raises for an engine failure."""
    output.mkdir(parents=True, exist_ok=False)
    engine, exporter = bundle / "bin" / ENGINE, bundle / "bin" / EXPORTER
    verdict = {"schema": SCHEMA, "seed": seed, "timings_ms": {}}
    manifest = bundle / "manifest.json"
    if manifest.exists():
        verdict["bundle"] = json.loads(manifest.read_text())
    production = None
    if production_path is not None:
        try:
            production = summary(json.loads(Path(production_path).read_text()))
            verdict["production"] = production
        except (OSError, ValueError, KeyError, IndexError, TypeError) as error:
            verdict["production_error"] = f"{type(error).__name__}: {error}"
    try:
        request = seeded(json.loads(Path(request_path).read_text()), seed)
        verdict["iterations"] = int(request["simOptions"].get("iterations", 0) or 0)
        request_file = output / "request.json"
        request_file.write_text(json.dumps(request) + "\n")
        prepared = output / "prepared.json"

        ms, done = run_stage("prepare", [exporter, "prepare", "--infile", request_file, "--outfile", prepared,
                                         "--scenario", "shadow"], timeout)
        verdict["timings_ms"]["prepare"] = round(ms, 1)
        if done.returncode != 0:
            raise StageError("prepare", done.stderr.strip()[-2000:])

        _, done = run_stage("check", [engine, "check", "--infile", prepared], timeout)
        if done.returncode != 0:
            raise StageError("check", done.stderr.strip()[-2000:])
        coverage = json.loads(done.stdout)
        if not coverage["supported"]:
            verdict.update(status="refused", reasons=coverage["reasons"])
            return verdict

        go_out, rust_out = output / "go.json", output / "rust.json"
        ms, done = run_stage("go", [exporter, "sim", "--infile", request_file, "--outfile", go_out], timeout)
        verdict["timings_ms"]["go"] = round(ms, 1)
        if done.returncode != 0:
            raise StageError("go", done.stderr.strip()[-2000:])
        ms, done = run_stage("rust", [engine, "sim", "--infile", prepared, "--outfile", rust_out], timeout)
        verdict["timings_ms"]["rust"] = round(ms, 1)
        if done.returncode != 0:
            raise StageError("rust", done.stderr.strip()[-2000:])

        go_result = json.loads(go_out.read_text())
        rust_result = json.loads(rust_out.read_text())["result"]
        if go_result.get("error"):
            raise StageError("go", json.dumps(go_result["error"]))
        differences, log_difference = compare_results(go_result, rust_result)
        verdict["go"], verdict["rust"] = summary(go_result), summary(rust_result)
        if production is not None:
            verdict["versus_production"] = versus_production(production, verdict["rust"])
        verdict.update(
            status="match" if not differences and log_difference is None else "mismatch",
            go_dps=prepared_v2.dps(go_result), rust_dps=prepared_v2.dps(rust_result),
            difference_count=len(differences), differences=differences[:MAX_DIFFERENCES],
            first_log_difference=log_difference)
        timings = verdict["timings_ms"]
        if timings["rust"] > 0:
            verdict["speedup"] = round(timings["go"] / timings["rust"], 3)
    except StageError as error:
        verdict.update(status="error", stage=error.stage, error=str(error))
    except (OSError, ValueError, KeyError, IndexError, TypeError) as error:
        verdict.update(status="error", stage="compare", error=f"{type(error).__name__}: {error}")
    return verdict


def git_revision():
    try:
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        dirty = bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=no"],
                                             cwd=ROOT, text=True).strip())
    except (OSError, subprocess.CalledProcessError):
        return None, None
    return revision, dirty


def build(cache, source, output):
    output = output.resolve()
    if output.exists():
        raise ValueError(f"{output} exists; a bundle is built into a new folder")
    exporter = prepared_v2.build_exporter(cache.resolve(), source)
    subprocess.run(["cargo", "build", "--locked", "--release", "--manifest-path", str(ROOT / "Cargo.toml")],
                   check=True)
    engine = ROOT / "target" / "release" / ENGINE
    (output / "bin").mkdir(parents=True)
    shutil.copy2(engine, output / "bin" / ENGINE)
    shutil.copy2(exporter, output / "bin" / EXPORTER)
    for path in BUNDLED_TOOLS:
        (output / path).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / path, output / path)
    revision, dirty = git_revision()
    version = subprocess.check_output([str(output / "bin" / ENGINE), "version"], text=True).strip()
    manifest = {"schema": SCHEMA, "rust_revision": revision, "rust_dirty": dirty, "engine_version": version,
                "reference_pin": PIN, "client_build": CLIENT_BUILD,
                "exporter_digest": prepared_v2.exporter_digest(),
                "platform": f"{platform.system()}-{platform.machine()}"}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["build", "run"])
    parser.add_argument("--output", type=Path, required=True, help="new folder to write")
    parser.add_argument("--request", type=Path, help="run: RaidSimRequest JSON")
    parser.add_argument("--production", type=Path, help="run: the production RaidSimResult to compare with")
    parser.add_argument("--seed", type=int, default=1, help="run: seed for an unseeded request")
    parser.add_argument("--timeout", type=int, default=300, help="run: seconds per step")
    parser.add_argument("--bundle", type=Path, default=ROOT, help="run: bundle folder (default: this one)")
    parser.add_argument("--source", default="https://github.com/sage3648/mythicsim-forever-engine-go.git",
                        help="build: local Git repository or clone URL of the reference")
    parser.add_argument("--cache", type=Path, default=ROOT / "oracle-cache", help="build: oracle cache")
    args = parser.parse_args()
    if args.command == "build":
        try:
            manifest = build(args.cache, args.source, args.output)
        except (ValueError, OSError, subprocess.CalledProcessError) as error:
            print(f"shadow build failed: {error}", file=sys.stderr)
            return 1
        print(json.dumps(manifest, indent=2))
        return 0
    if args.request is None:
        parser.error("run needs --request")
    if args.seed == 0:
        parser.error("--seed must not be 0, which Go reads as unseeded")
    if args.output.exists():
        parser.error(f"{args.output} exists; each run writes a new folder")
    verdict = shadow(args.request, args.output, args.bundle.resolve(), args.seed, args.timeout, args.production)
    (args.output / "verdict.json").write_text(json.dumps(verdict, indent=2) + "\n")
    print(json.dumps(verdict))
    return 0


if __name__ == "__main__":
    sys.exit(main())
