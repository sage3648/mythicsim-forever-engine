#!/usr/bin/env python3
"""Write the Rogue preparation goldens: stripped requests and the pinned Go exporter's output.

Each case is a Rogue fixture request without buffs, debuffs, consumables but the poisons,
professions and effect gear, with a pair of weapons and, for the set cases, the pieces of a set.
What it prepares is the Rogue's own: its poisons, spells, talent auras and mods, set bonuses,
class effects and stats. The Go exporter prepares it and the goldens keep a digest of each part
of the answer, never the answer itself, so a failing test names the spell, aura or effect that
changed and the files stay small.

    python3 tools/rogue_prepare_goldens.py --oracle /path/to/forever-go-oracle-v2

The digest is `canonical` below, which tests/classes/rogue/prepare.rs implements again in Rust:
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
OUT = ROOT / "tests" / "classes" / "rogue" / "prepare"

# Weapons without effects, by equipment slot (14 main hand, 15 off hand).
DAGGERS = {14: 275836, 15: 18392}
SWORDS = {14: 22383, 15: 281587}
MACES = {14: 281586, 15: 279261}
# Set pieces without effects: head, shoulder, chest, wrist, hands, waist, legs, feet.
NIGHTSLAYER = {0: 16821, 2: 16823, 4: 16820, 5: 16825, 6: 16826, 7: 16827, 8: 16822, 9: 16824}
DARKMANTLE = {0: 22005, 2: 22008, 4: 22009, 5: 22004, 6: 22006, 7: 22002, 8: 22007, 9: 22003}
EVERY_TALENT = "32532312551521251-32533321221515231-5323223312213211551"

# name, fixture whose request is the base, talents, equipment by slot, mob type
CASES = [
    ("assassination", "production-assassination-rogue", None, DAGGERS, None),
    ("combat-wound-poison", "combat-wound-poison", None, SWORDS, None),
    ("subtlety", "production-subtlety-rogue", None, DAGGERS, None),
    ("every-talent-at-max", "production-assassination-rogue", EVERY_TALENT, MACES, "MobTypeHumanoid"),
    ("nightslayer", "production-combat-rogue", None, {**NIGHTSLAYER, **DAGGERS}, None),
    ("darkmantle", "production-subtlety-rogue", None, {**DARKMANTLE, **DAGGERS}, None),
]


def strip(request, talents, equipment, mob_type):
    out = copy.deepcopy(request)
    raid = out["raid"]
    raid.pop("buffs", None)
    raid.pop("debuffs", None)
    party = raid["parties"][0]
    party.pop("buffs", None)
    player = party["players"][0]
    for key in ("buffs", "bonusStats", "profession1", "profession2"):
        player.pop(key, None)
    consumables = player.get("consumables", {})
    player["consumables"] = {k: v for k, v in consumables.items() if k in ("mhImbueId", "ohImbueId")}
    items = [{} for _ in range(max(equipment) + 1)]
    for slot, item in equipment.items():
        items[slot] = {"id": item}
    player["equipment"] = {"items": items}
    if talents:
        player["talentsString"] = talents
    # One target: the others are copies of it.
    out["encounter"]["targets"] = out["encounter"]["targets"][:1]
    if mob_type:
        out["encounter"]["targets"][0]["mobType"] = mob_type
    return out


def canonical(value):
    """The text the digests are taken of, the same in tests/classes/rogue/prepare.rs."""
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
        ("player.energy", player["energy"]),
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
    for name, fixture, talents, equipment, mob_type in CASES:
        base = json.loads((FIXTURES / f"{fixture}.request.json").read_text())
        request = strip(base, talents, equipment, mob_type)
        request_path = OUT / f"{name}.request.json"
        request_path.write_text(json.dumps(request, indent=1, sort_keys=True) + "\n")
        with tempfile.TemporaryDirectory() as scratch:
            result = Path(scratch) / "go.json"
            run = subprocess.run([args.oracle, "prepare", "--infile", str(request_path), "--scenario", name,
                                  "--outfile", str(result)], capture_output=True, text=True)
            if run.returncode != 0:
                sys.exit(f"{name}: the Go exporter failed: {run.stderr.strip()}")
            prepared = json.loads(result.read_text())
        if prepared["unrepresented"]:
            sys.exit(f"{name}: the exporter reports {prepared['unrepresented']}")
        golden = {"case": name, "request_sha256": prepared["request_sha256"], "sections": sections(prepared)}
        text = json.dumps(golden, indent=1, sort_keys=True)
        # One [label, digest] pair per line keeps a golden small and its diffs readable.
        text = re.sub(r'\[\s+("(?:[^"\\\n]|\\.)*"),\s+("[0-9a-f]{16}")\s+\]', r"[\1, \2]", text)
        (OUT / f"{name}.golden.json").write_text(text + "\n")
        print(f"wrote {name}")


if __name__ == "__main__":
    main()
