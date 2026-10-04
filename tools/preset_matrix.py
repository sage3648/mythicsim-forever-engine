#!/usr/bin/env python3
"""Cross the upstream UI presets of every class and spec into RaidSimRequests.

Each spec under the pinned engine's ui/specs ships preset rotations, talents and gear sets,
which is what users load in the Forever UI. For every spec this crosses each preset rotation
with each preset talent string and each preset gear set, and writes one request per
combination: the matching production request with its rotation, talents and equipment
replaced. The production request supplies race, buffs, consumables and class options. As the
UI's lookupEquipmentSpec does, an item the pinned database lacks is left out, its slot empty.
Compare the requests with `tools/prepared_v2.py compare` and record them with
`tools/sweep_record.py`. Uses only Python's standard library.
"""

import argparse
import copy
import json
from pathlib import Path
import re

# The production request each spec's rotations start from, by the rotation file's stem; the
# empty key covers the spec's other rotations.
SPEC_BASES = {
    "druid/balance": {"": "balance-druid"},
    "druid/feralbear": {"": "feral-bear-druid"},
    "druid/feralcat": {"": "feral-druid"},
    "hunter/dps": {"bm": "hunter", "sv_melee": "survival-hunter", "": "marksmanship-hunter"},
    "mage/dps": {"arcane": "arcane-mage", "frost": "frost-mage", "frostfire": "frostfire-mage",
                 "": "fire-mage"},
    "paladin/protection": {"": "protection-paladin"},
    "paladin/retribution": {"": "retribution-paladin"},
    "priest/dps": {"shadow": "shadow-priest", "": "smite-priest"},
    "rogue/dps": {"forever_mutilate": "assassination-rogue", "forever_hemorrhage": "subtlety-rogue",
                  "": "combat-rogue"},
    "shaman/elemental": {"": "elemental-shaman"},
    "shaman/enhancement": {"": "enhancement-shaman"},
    "warlock/dps": {"affliction": "affliction-warlock", "demonic_pact": "demonology-warlock",
                    "": "destruction-warlock"},
    "warrior/dps": {"": "warrior"},
    "warrior/protection": {"": "protection-warrior"},
}

IMPORT = re.compile(r"import\s+(\w+)\s+from\s+'\./((?:apls|gear_sets)/[^']+\.json)'")
ROTATION = re.compile(r"makePresetAPLRotation\(\s*'([^']*)'\s*,\s*(\w+)")
GEAR = re.compile(r"makePresetGear\(\s*'([^']*)'\s*,\s*(\w+)")
TALENTS = re.compile(r"makePresetTalents\(\s*'([^']*)'\s*,\s*SavedTalents\.create\(\{\s*talentsString:\s*'([^']*)'")


def slug(text):
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


def presets(spec_dir):
    """The spec's preset rotations and gear sets as (name, file) and its talents as (name,
    talents string), in source order. A preset whose identifier no file import names, as a
    blank one written inline, is left out."""
    source = (spec_dir / "presets.ts").read_text()
    files = dict(IMPORT.findall(source))
    rotations = [(name, files[ident]) for name, ident in ROTATION.findall(source) if ident in files]
    gear = [(name, files[ident]) for name, ident in GEAR.findall(source) if ident in files]
    return rotations, TALENTS.findall(source), gear


def base_name(spec, rotation_file):
    bases = SPEC_BASES[spec]
    return bases.get(Path(rotation_file).name.split(".")[0], bases[""])


def labels(entries):
    """A slug per (name, file or talents string) preset. Presets sharing a name, as the
    Priest Launch and Smite Launch gear sets both called Launch, are told apart by the file
    stem, or by their position when they have no file."""
    slugs = [slug(name) for name, _ in entries]
    out = []
    for index, (label, (_, source)) in enumerate(zip(slugs, entries)):
        if slugs.count(label) > 1:
            label = slug(Path(source).name.split(".")[0]) if source.endswith(".json") else f"{label}-{index + 1}"
        out.append(label)
    return out


def combinations(specs_root, spec):
    """(name, base, rotation file, talents string, gear file) for each combination."""
    rotations, talents, gear = presets(specs_root / spec)
    out = []
    for rotation_label, (_, rotation_file) in zip(labels(rotations), rotations):
        for talents_label, (_, talents_string) in zip(labels(talents), talents):
            for gear_label, (_, gear_file) in zip(labels(gear), gear):
                name = "-".join([slug(spec), rotation_label, talents_label, gear_label])
                out.append((name, base_name(spec, rotation_file), rotation_file, talents_string, gear_file))
    return out


def known_gear(gear, items):
    """The gear set with each item the database lacks left out, and those items' IDs."""
    known = copy.deepcopy(gear)
    missing = []
    for index, item in enumerate(known.get("items", [])):
        if item.get("id") and item["id"] not in items:
            missing.append(item["id"])
            known["items"][index] = {}
    return known, missing


def build(base, rotation, talents, gear):
    request = copy.deepcopy(base)
    player = request["raid"]["parties"][0]["players"][0]
    player["rotation"] = rotation
    player["talentsString"] = talents
    player["equipment"] = gear
    return request


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--specs", required=True, type=Path, help="the pinned engine's ui/specs")
    parser.add_argument("--bases", required=True, type=Path,
                        help="directory of production requests, NAME.request.json")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--database", required=True, type=Path,
                        help="the pinned engine's assets/database/db.json")
    parser.add_argument("--only", action="append", help="limit to these specs, as class/spec")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    items = {item["id"] for item in json.loads(args.database.read_text())["items"]}
    dropped = {}
    count = 0
    for spec in SPEC_BASES:
        if args.only and spec not in args.only:
            continue
        spec_dir = args.specs / spec
        for name, base, rotation_file, talents, gear_file in combinations(args.specs, spec):
            gear, missing = known_gear(json.loads((spec_dir / gear_file).read_text()), items)
            if missing:
                dropped[f"{spec}/{gear_file}"] = missing
            request = build(json.loads((args.bases / f"{base}.request.json").read_text()),
                            json.loads((spec_dir / rotation_file).read_text()), talents, gear)
            # Compact, as the requests are many: a full matrix runs to hundreds.
            (args.output / f"{name}.request.json").write_text(json.dumps(request, separators=(",", ":")) + "\n")
            count += 1
    print(f"Wrote {count} requests to {args.output}")
    for gear_file, missing in sorted(dropped.items()):
        print(f"{gear_file}: left out items {sorted(set(missing))} the database lacks")


if __name__ == "__main__":
    main()
