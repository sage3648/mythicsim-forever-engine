#!/usr/bin/env python3
"""Write the classic weapon and enchant preparation goldens: stripped requests and the pinned Go
exporter's output.

Each case is the stripped Retribution Paladin request of tools/paladin_prepare_goldens.py with
one weapon or enchant of sim/common/classic (items_weapons.go, enchants.go) equipped. The Go
exporter prepares it and the goldens keep a digest of each part of the answer, never the answer
itself, so a failing test names the spell, aura or effect that changed.

    python3 tools/classic_weapons_goldens.py --oracle /path/to/forever-go-oracle-v2

The digests are those of paladin_prepare_goldens.py, which tests/prepare_classic_weapons.rs
implements again in Rust.
"""

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import paladin_prepare_goldens as base

ROOT = base.ROOT
OUT = ROOT / "tests" / "prepare_classic_weapons"

# The Paladin's own spells follow the retribution case's rotation.
TALENTS = "52003-503-05225231001330321"
ROTATION = [20271, 20920, 20308, 24239, 10333, 20924, 10314]
HANDS, BACK, MAIN_HAND = 6, 3, 14
PLAIN_GLOVES, PLAIN_CLOAK, PLAIN_TWO_HAND = 14615, 13340, 35

# name, equipment by slot: item id and enchant
CASES = [
    ("crusader", {MAIN_HAND: (PLAIN_TWO_HAND, 1900)}),
    ("threat-enchants", {HANDS: (PLAIN_GLOVES, 2613), BACK: (PLAIN_CLOAK, 2621), MAIN_HAND: (PLAIN_TWO_HAND, 0)}),
    ("dragons-call", {MAIN_HAND: (10847, 0)}),
    ("ironfoe", {MAIN_HAND: (11684, 0)}),
    ("sulfuras", {MAIN_HAND: (17182, 0)}),
    ("ebon-hilt-of-marduk", {MAIN_HAND: (14576, 0)}),
    ("thunderfury", {MAIN_HAND: (19019, 0)}),
    ("runeblade-of-baron-rivendare", {MAIN_HAND: (13505, 0)}),
    ("headmasters-charge", {MAIN_HAND: (13937, 0)}),
]


def case_request(equipment):
    source = json.loads((base.FIXTURES / "production-retribution-paladin.request.json").read_text())
    request = base.strip(source, "retributionPaladin", TALENTS, {}, ROTATION)
    items = [{} for _ in range(max(equipment) + 1)]
    for slot, (item, enchant) in equipment.items():
        items[slot] = {"id": item}
        if enchant:
            items[slot]["enchant"] = enchant
    request["raid"]["parties"][0]["players"][0]["equipment"] = {"items": items}
    return request


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--oracle", required=True, help="the pinned Go exporter, forever-go-oracle-v2")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    for name, equipment in CASES:
        request_path = OUT / f"{name}.request.json"
        request_path.write_text(json.dumps(case_request(equipment), indent=1, sort_keys=True) + "\n")
        with tempfile.TemporaryDirectory() as scratch:
            result = Path(scratch) / "go.json"
            run = subprocess.run([args.oracle, "prepare", "--infile", str(request_path), "--scenario", name,
                                  "--outfile", str(result)], capture_output=True, text=True)
            if run.returncode != 0:
                sys.exit(f"{name}: the Go exporter failed: {run.stderr.strip()}")
            prepared = json.loads(result.read_text())
        golden = {"case": name, "request_sha256": prepared["request_sha256"], "sections": base.sections(prepared)}
        text = json.dumps(golden, indent=1, sort_keys=True)
        text = re.sub(r'\[\s+("(?:[^"\\\n]|\\.)*"),\s+("[0-9a-f]{16}")\s+\]', r"[\1, \2]", text)
        (OUT / f"{name}.golden.json").write_text(text + "\n")
        print(f"wrote {name}")


if __name__ == "__main__":
    main()
