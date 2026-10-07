#!/usr/bin/env python3
"""Write the Paladin preparation goldens: stripped requests and the pinned Go exporter's output.

Each case is a Paladin fixture request without buffs, debuffs, consumables, professions and
effect gear, a Human paladin with a few weapons, one libram or set for the item effects, and a
short rotation naming the spells whose effects depend on it. What it prepares is the Paladin's
own: spells, seals, talent auras, class effects and stats. The Go exporter prepares it and the
goldens keep a digest of each part of the answer, never the answer itself, so a failing test
names the spell, aura or effect that changed and the files stay small.

    python3 tools/paladin_prepare_goldens.py --oracle /path/to/forever-go-oracle-v2

The digest is `canonical` below, which tests/classes/paladin/prepare.rs implements again in Rust:
objects by sorted key, numbers as the IEEE 754 bits of their float64 value, so Go's `1` and
Rust's `1.0` agree.
"""

import argparse
import copy
import hashlib
import json
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures" / "mage" / "prepared-v2"
OUT = ROOT / "tests" / "classes" / "paladin" / "prepare"

# Items without effects, by equipment slot: a two-hander, a one-hander and a shield.
TWO_HAND, ONE_HAND, SHIELD = 35, 36, 876
LIBRAM_OF_HOLY_ALACRITY, LIBRAM_OF_FERVOR = 228175, 23203
LAWBRINGER = {0: 16854, 2: 16856, 4: 16853, 6: 16860, 8: 16855}


def cast(spell_id):
    return {"action": {"castSpell": {"spellId": {"spellId": spell_id}}}}


# name, spec, fixture whose request is the base, talents, equipment by slot, rotation spell ids
CASES = [
    ("retribution", "retributionPaladin", "production-retribution-paladin",
     "52003-503-05225231001330321", {14: TWO_HAND, 16: LIBRAM_OF_FERVOR},
     [20271, 20920, 20308, 24239, 10333, 20924, 10314]),
    ("retribution-lawbringer", "retributionPaladin", "ret-pursuit-of-justice",
     "52003-503-05225231201330321", {**LAWBRINGER, 14: TWO_HAND}, [20271, 20293, 21084]),
    ("shockadin", "holyPaladin", "production-shockadin-paladin",
     "550031032001012--052252300012303", {14: ONE_HAND, 15: SHIELD, 16: LIBRAM_OF_HOLY_ALACRITY},
     [20930, 25292, 10310, 19943, 20216]),
    ("protection", "protectionPaladin", "production-retribution-paladin",
     "50003-0530213301301551-50203", {14: ONE_HAND, 15: SHIELD}, [20928, 20927, 25780, 1311015, 1310994]),
    ("every-talent-at-max", "retributionPaladin", "production-retribution-paladin",
     "55323213225131251-5532513321331551-55225331221331321", {14: ONE_HAND, 15: SHIELD},
     [20271, 20928, 10310, 20930, 1311595]),
]


def strip(request, spec, talents, equipment, rotation):
    out = copy.deepcopy(request)
    raid = out["raid"]
    raid.pop("buffs", None)
    raid.pop("debuffs", None)
    party = raid["parties"][0]
    party.pop("buffs", None)
    player = party["players"][0]
    for key in ("buffs", "consumables", "bonusStats", "profession1", "profession2",
                "retributionPaladin", "protectionPaladin", "holyPaladin"):
        player.pop(key, None)
    player[spec] = {"options": {"classOptions": {}}}
    player["race"] = "RaceHuman"
    items = [{} for _ in range(max(equipment) + 1)] if equipment else []
    for slot, item in equipment.items():
        items[slot] = {"id": item}
    player["equipment"] = {"items": items}
    player["rotation"] = {"type": "TypeAPL", "priorityList": [cast(spell) for spell in rotation]}
    player["talentsString"] = talents
    return out


def canonical(value):
    """The text the digests are taken of, the same in tests/classes/paladin/prepare.rs."""
    if value is None:
        return "z"
    if value is True:
        return "t"
    if value is False:
        return "f"
    if isinstance(value, (int, float)):
        return "n" + struct.pack(">d", float(value)).hex()
    if isinstance(value, str):
        return f"s{len(value.encode())}:{value}"
    if isinstance(value, list):
        return "[" + ",".join(canonical(item) for item in value) + "]"
    return "{" + ",".join(f"{canonical(key)}:{canonical(value[key])}" for key in sorted(value)) + "}"


def digest(value):
    """The first 64 bits of the SHA-256: enough to see a change, small enough for a golden."""
    return hashlib.sha256(canonical(value).encode()).hexdigest()[:16]


def label_spell(spell):
    return json.dumps(spell["action_id"], sort_keys=True)


def label_aura(aura):
    return aura["label"]


def label_effect(effect):
    return effect["kind"]


def sections(prepared):
    """Each listed part of the answer, as (label, digest) per item or one digest for a whole."""
    player, target = prepared["player"], prepared["target"]
    parts = {}
    for name, items, label in (
        ("player.spells", player["spells"], label_spell),
        ("player.auras", player["auras"], label_aura),
        ("target.auras", target["auras"], label_aura),
        ("effects", prepared["effects"], label_effect),
        ("player.major_cooldowns", player["major_cooldowns"], label_spell),
    ):
        parts[name] = [[label(item), digest(item)] for item in items]
    for name, value in (
        ("player.stats", player["stats"]),
        ("player.pseudo_stats", player["pseudo_stats"]),
        ("player.mana", player["mana"]),
        ("player.talents", player["talents"]),
        ("player.attack_table", player["attack_table"]),
        ("melee", prepared["melee"]),
        ("unrepresented", prepared["unrepresented"]),
    ):
        parts[name] = digest(value)
    return parts


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--oracle", required=True, help="the pinned Go exporter, forever-go-oracle-v2")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    for name, spec, fixture, talents, equipment, rotation in CASES:
        base = json.loads((FIXTURES / f"{fixture}.request.json").read_text())
        request = strip(base, spec, talents, equipment, rotation)
        request_path = OUT / f"{name}.request.json"
        request_path.write_text(json.dumps(request, indent=1, sort_keys=True) + "\n")
        with tempfile.TemporaryDirectory() as scratch:
            result = Path(scratch) / "go.json"
            run = subprocess.run([args.oracle, "prepare", "--infile", str(request_path), "--scenario", name,
                                  "--outfile", str(result)], capture_output=True, text=True)
            if run.returncode != 0:
                sys.exit(f"{name}: the Go exporter failed: {run.stderr.strip()}")
            prepared = json.loads(result.read_text())
        golden = {"case": name, "request_sha256": prepared["request_sha256"], "sections": sections(prepared)}
        text = json.dumps(golden, indent=1, sort_keys=True)
        # One [label, digest] pair per line keeps a golden small and its diffs readable.
        text = re.sub(r'\[\s+("(?:[^"\\\n]|\\.)*"),\s+("[0-9a-f]{16}")\s+\]', r"[\1, \2]", text)
        (OUT / f"{name}.golden.json").write_text(text + "\n")
        print(f"wrote {name}")


if __name__ == "__main__":
    main()
