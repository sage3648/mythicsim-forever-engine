#!/usr/bin/env python3
"""Audit the frozen first-build inventory; capture new evidence into scratch only."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / "inventory" / "first-frost"


def load(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def leaves(value, prefix=""):
    """Include empty containers: enabling an empty APL operator still changes scope."""
    if isinstance(value, dict) and value:
        return {p: v for k, child in value.items()
                for p, v in leaves(child, prefix + "/" + k).items()}
    if isinstance(value, list) and value:
        return {p: v for i, child in enumerate(value)
                for p, v in leaves(child, prefix + "/" + str(i)).items()}
    return {prefix: value}


def action_ids(value):
    found = set()
    if isinstance(value, dict):
        for kind in ("spellId", "itemId", "otherId"):
            if kind in value and isinstance(value[kind], (int, str)):
                found.add((kind, str(value[kind]), value.get("tag", 0)))
        for child in value.values():
            found.update(action_ids(child))
    elif isinstance(value, list):
        for child in value:
            found.update(action_ids(child))
    return found


def check(directory=DIRECTORY, app_source=None, engine_source=None):
    manifest = load(directory / "manifest.json")
    snapshot = load(directory / "snapshot.json")
    observation = load(directory / "observation.json")
    errors = []

    def require(condition, message):
        if not condition:
            errors.append(message)

    require(manifest["schema_version"] == 1, "unknown inventory schema")
    require(manifest["kind"] == "planning_inventory", "inventory must remain planning evidence")
    require(manifest["production_eligible"] is False, "inventory is not runtime support")
    for filename, sha in manifest["artifacts"].items():
        require(digest(directory / filename) == sha, f"changed frozen artifact: {filename}")
    # Hashes catch value changes; this separate coverage audit catches missing entries.
    require(set(leaves(snapshot["request"])) == set(manifest["request_paths"]),
            "request field coverage differs")
    player = snapshot["request"]["raid"]["parties"][0]["players"][0]
    require(player["talentsString"] == snapshot["export"]["talents"], "mapped talents differ")
    expected_talents = {(tree, index, int(rank))
                        for tree, digits in enumerate(player["talentsString"].split("-"))
                        for index, rank in enumerate(digits) if int(rank)}
    covered = [(t["tree"], t["index"], t["rank"]) for t in manifest["talents"]]
    require(set(covered) == expected_talents and len(covered) == len(expected_talents),
            "talent coverage differs")
    require([x["input"] for x in manifest["equipment"]] == snapshot["export"]["gear"]["items"],
            "equipment slot or enchant coverage differs")
    require(len(manifest["equipment"]) == 17, "export needs all 17 slots, including empty trinkets")
    declared_ids = [(x["kind"], str(x["value"]), x.get("tag", 0)) for x in manifest["action_ids"]]
    actual_ids = action_ids(player["rotation"]) | action_ids(observation)
    require(set(declared_ids) == actual_ids and len(declared_ids) == len(actual_ids),
            "rotation or observed action identity coverage differs")
    require(player["class"] == "ClassMage" and player["race"] == "RaceHuman",
            "reference character identity differs")
    require(snapshot["request"]["simOptions"] ==
            {"iterations": 3000, "randomSeed": "42", "debugFirstIteration": True},
            "reference run options differ")
    require(observation["summary"]["IterationsDone"] == 3000, "reference did not finish")
    require(observation["first_fight_timeline"] is not None, "missing first-fight consumer evidence")
    require(observation["summary"]["DPSMean"] > 0, "reference has no damage")
    references = set(manifest["source_files"])
    sections = ("talents", "equipment", "mechanics", "action_ids", "known_gaps", "consumers")
    for section in sections:
        for entry in manifest[section]:
            require(bool(entry.get("evidence")) and set(entry["evidence"]) <= references,
                    f"missing source evidence in {section}: {entry.get('name', entry.get('slot'))}")
    for source_name, root in (("app", app_source), ("engine", engine_source)):
        if root is None:
            continue
        root = Path(root).resolve()
        head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
        require(head == manifest["sources"][source_name]["revision"], f"{source_name} revision differs")
        for key, source in manifest["source_files"].items():
            if source["repository"] == source_name:
                path = root / source["path"]
                require(path.is_file() and digest(path) == source["sha256"], f"source drift: {key}")
    if errors:
        raise ValueError("\n".join(errors))
    return manifest


def compact_observation(report):
    """Keep exercised rows, not the engine's many registered but unused spell ranks."""
    result = report["result"]
    player = result["raidMetrics"]["parties"][0]["players"][0]

    def exercised(row):
        return any(isinstance(v, (int, float)) and v != 0 and k != "unitIndex"
                   for target in row.get("targets", []) for k, v in target.items())

    def order(rows):
        return sorted(rows, key=lambda row: json.dumps(row["id"], sort_keys=True))

    return {
        "kind": "historical_go_observation",
        "summary": report["summary"],
        "raw_result_fields": sorted(result),
        "player_fields": sorted(player),
        "active_actions": order([a for a in player.get("actions", []) if exercised(a)]),
        "active_auras": order([a for a in player.get("auras", []) if a.get("procsAvg", 0) > 0]),
        "active_resources": order([a for a in player.get("resources", []) if a.get("events", 0) > 0]),
        "active_target_auras": order([a for a in result["encounterMetrics"]["targets"][0].get("auras", [])
                                      if a.get("procsAvg", 0) > 0]),
        "first_fight_timeline": report.get("timeline"),
    }


def compare_observations(reference, actual):
    """Compare all saved metrics and timeline fields, with exact integer counts."""
    expected, observed = leaves(reference), leaves(actual)
    differences = []
    numeric_fields = 0
    for path in sorted(set(expected) | set(observed)):
        if path not in expected or path not in observed:
            differences.append({"path": path, "reason": "missing or added field"})
            continue
        a, b = expected[path], observed[path]
        if type(a) is int and type(b) is int:
            numeric_fields += 1
            matches = a == b
        elif type(a) in (int, float) and type(b) in (int, float):
            numeric_fields += 1
            matches = math.isfinite(a) and math.isfinite(b) and math.isclose(
                a, b, abs_tol=1e-8, rel_tol=1e-12)
        else:
            matches = type(a) is type(b) and a == b
        if not matches:
            differences.append({"path": path, "expected": a, "actual": b})
    return {"passed": not differences, "compared_fields": len(set(expected) | set(observed)),
            "numeric_fields": numeric_fields,
            "float_tolerance": {"absolute": 1e-8, "relative": 1e-12},
            "integer_counts": "exact", "differences": differences}


def go_module(directory, module, source, helper):
    directory.mkdir()
    # JSON strings are also valid quoted Go module paths and protect whitespace.
    (directory / "go.mod").write_text(
        "module inventorycapture\n\ngo 1.25.0\n\n"
        f"require {module} v0.0.0\nreplace {module} => {json.dumps(str(source))}\n")
    shutil.copy(ROOT / "tools" / "reference-capture" / helper, directory / "main.go")


def capture(app_source, engine_source, output):
    app_source, engine_source = Path(app_source).resolve(), Path(engine_source).resolve()
    check(app_source=app_source, engine_source=engine_source)
    output = Path(output).resolve()
    if output == DIRECTORY.resolve() or DIRECTORY.resolve() in output.parents:
        raise ValueError("capture must use scratch storage, not accepted inventory")
    # Never overwrite an existing observation directory or modify supplied checkouts.
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="forever-inventory-") as temporary:
        work = Path(temporary)
        app = work / "app"
        engine = work / "engine"
        go_module(app, "github.com/mythicsim/mythicsim/worker", app_source / "worker", "request/main.go")
        go_module(engine, "github.com/wowsims/forever", engine_source, "observe/main.go")
        snapshot = subprocess.check_output(["go", "run", "-mod=mod", "."], cwd=app)
        (output / "snapshot.json").write_bytes(snapshot)
        subprocess.run(["go", "run", "-mod=mod", "-tags", "with_db", ".",
                        str(output / "snapshot.json"), str(work / "result.json")], cwd=engine, check=True)
        report = subprocess.check_output(["go", "run", "-mod=mod", ".", "--result",
                                          str(work / "result.json")], cwd=app)
        observation = compact_observation(json.loads(report))
        (output / "observation.json").write_text(json.dumps(observation, indent=2, sort_keys=True) + "\n")
        snapshot_matches = json.loads(snapshot) == load(DIRECTORY / "snapshot.json")
        identities_match = action_ids(observation) == action_ids(load(DIRECTORY / "observation.json"))
        metrics = compare_observations(load(DIRECTORY / "observation.json"), observation)
        comparison = {"snapshot_matches": snapshot_matches, "action_identities_match": identities_match,
                      "output_comparison": metrics,
                      "reference_engine_revision": load(DIRECTORY / "manifest.json")["sources"]["engine"]["revision"],
                      "note": "Frozen Go output versus fresh Go execution. This is not a full-build Rust comparison."}
        (output / "comparison.json").write_text(json.dumps(comparison, indent=2) + "\n")
        if not snapshot_matches or not identities_match or not metrics["passed"]:
            raise ValueError(f"reference capture differs; review {output / 'comparison.json'}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["check", "capture"], nargs="?", default="check")
    parser.add_argument("--app-source", type=Path)
    parser.add_argument("--engine-source", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "capture":
            if not all((args.app_source, args.engine_source, args.output)):
                parser.error("capture needs --app-source, --engine-source and --output")
            capture(args.app_source, args.engine_source, args.output)
            print(f"Captured reference evidence in {args.output}")
        else:
            manifest = check(app_source=args.app_source, engine_source=args.engine_source)
            print(f"Inventory checked: {len(manifest['talents'])} active talents, "
                  f"17 equipment slots, {len(manifest['action_ids'])} action identities. "
                  "Rust production eligibility: false.")
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Inventory audit failed: {error}\n")


if __name__ == "__main__":
    main()
