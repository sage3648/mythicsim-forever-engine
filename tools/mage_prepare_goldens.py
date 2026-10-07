#!/usr/bin/env python3
"""Write the Mage preparation goldens: stripped requests and the pinned Go exporter's output.

Each case is a Mage fixture request without buffs, debuffs, consumables, professions and gear,
a Human mage with one Frostbolt rotation action, so what it prepares is the Mage's own: spells,
talent auras, class effects and stats. The Go exporter prepares it and the goldens keep a
digest of each part of the answer, never the answer itself, so a failing test names the spell,
aura or effect that changed and the files stay small.

    python3 tools/mage_prepare_goldens.py --oracle /path/to/forever-go-oracle-v2

The digest is `canonical` below, which tests/classes/mage/prepare.rs implements again in Rust:
objects by sorted key, numbers as the IEEE 754 bits of their float64 value, so Go's `1` and
Rust's `1.0` agree.
"""

import argparse
import copy
import hashlib
import json
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures" / "mage" / "prepared-v2"
OUT = ROOT / "tests" / "classes" / "mage" / "prepare"

# name, fixture whose request is the base, talents string, armor (None keeps the fixture's)
CASES = [
    ("arcane", "arcane-blast", "055205003100311531-230500001-005", None),
    ("arcane-frost-armor", "arcane-blast", "055205003100311531-230500001-005", "MageArmorFrostArmor"),
    ("fire", "fire-ignite", "230205-03552020130133051-005", None),
    ("fire-three-targets", "fire-mage-3-targets-dot-target-index", "05020500300001-13550000130133051-004", None),
    ("frost", "reference-fingers-of-frost", "05020000300000-03-0555000301001301251", None),
    ("frost-three-targets", "frost-mage-3-targets-area-lines", "05020500300001-03-0555000301001301251", None),
    ("frostfire-two-targets", "frostfire-mage-2-targets-multidot", "-1355000013013304-00550003210013002", None),
    ("every-talent-at-max", "arcane-blast", "255225223122311531-23552333132133151-2555323331321331251", None),
    ("every-talent-at-one", "arcane-blast", "111111111111111111-11111111111111111-1111111111111111111", "MageArmorMoltenArmor"),
    ("mixed-build", "arcane-blast", "233112022120100020-20001021102022010-1415222321300231111", "MageArmorNone"),
    ("mixed-build-two", "arcane-blast", "025204101010001530-23142032102111020-0225112100121101020", "MageArmorFrostArmor"),
    ("mixed-build-three", "arcane-blast", "132100123010311111-01201030111102021-0204001011010000121", "MageArmorMageArmor"),
]


def strip(request, talents, armor):
    out = copy.deepcopy(request)
    raid = out["raid"]
    raid.pop("buffs", None)
    raid.pop("debuffs", None)
    party = raid["parties"][0]
    party.pop("buffs", None)
    player = party["players"][0]
    for key in ("buffs", "consumables", "bonusStats", "profession1", "profession2"):
        player.pop(key, None)
    player["race"] = "RaceHuman"
    player["equipment"] = {"items": []}
    player["rotation"] = {"type": "TypeAPL",
                          "priorityList": [{"action": {"castSpell": {"spellId": {"spellId": 116}}}}]}
    player["talentsString"] = talents
    if armor:
        player["mage"]["options"]["classOptions"]["defaultMageArmor"] = armor
    return out


def canonical(value):
    """The text the digests are taken of, the same in tests/classes/mage/prepare.rs."""
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
    return hashlib.sha256(canonical(value).encode()).hexdigest()


def label_spell(spell):
    return json.dumps(spell["action_id"], sort_keys=True)


def label_aura(aura):
    return aura["label"]


def label_effect(effect):
    return effect["kind"]


def label_mcd(mcd):
    return json.dumps(mcd["action_id"], sort_keys=True)


def sections(prepared):
    """Each listed part of the answer, as (label, digest) per item or one digest for a whole."""
    player, target = prepared["player"], prepared["target"]
    parts = {}
    for name, items, label in (
        ("player.spells", player["spells"], label_spell),
        ("player.auras", player["auras"], label_aura),
        ("target.auras", target["auras"], label_aura),
        ("effects", prepared["effects"], label_effect),
        ("player.major_cooldowns", player["major_cooldowns"], label_mcd),
    ):
        parts[name] = [[label(item), digest(item)] for item in items]
    for name, value in (
        ("player.stats", player["stats"]),
        ("player.pseudo_stats", player["pseudo_stats"]),
        ("player.mana", player["mana"]),
        ("player.talents", player["talents"]),
        ("encounter", prepared["encounter"]),
        ("unrepresented", prepared["unrepresented"]),
    ):
        parts[name] = digest(value)
    return parts


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--oracle", required=True, help="the pinned Go exporter, forever-go-oracle-v2")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    for name, fixture, talents, armor in CASES:
        base = json.loads((FIXTURES / f"{fixture}.request.json").read_text())
        request = strip(base, talents, armor)
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
        (OUT / f"{name}.golden.json").write_text(json.dumps(golden, indent=1, sort_keys=True) + "\n")
        print(f"wrote {name}")


if __name__ == "__main__":
    main()
