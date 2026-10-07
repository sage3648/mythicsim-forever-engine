#!/usr/bin/env python3
"""Write the Hunter preparation goldens: stripped requests and the pinned Go exporter's output.

Each case is a Hunter fixture request without buffs, debuffs, consumables and professions, a
Human hunter with weapons that carry no item effect and one Aimed Shot rotation action, so what
it prepares is the Hunter's own: spells, talent auras, class effects, the pet and stats. The Go
exporter prepares it and the goldens keep a digest of each part of the answer, never the answer
itself, so a failing test names the spell, aura, effect or pet that changed and the files stay
small.

    python3 tools/hunter_prepare_goldens.py --oracle /path/to/forever-go-oracle-v2

The digest is `canonical` below, which tests/classes/hunter/prepare.rs implements again in Rust:
objects by sorted key, numbers as the IEEE 754 bits of their float64 value, so Go's `1` and
Rust's `1.0` agree.
"""

import argparse
import copy
import json
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures" / "mage" / "prepared-v2"
OUT = ROOT / "tests" / "classes" / "hunter" / "prepare"

# Weapons without item effects: Huhuran's Stinger (bow), a Grand Marshal's polearm, swords and a
# crossbow, and Larvae of the Great Worm (gun).
BOW, POLEARM, SWORD_MH, SWORD_OH, CROSSBOW, GUN = 21616, 234570, 234579, 234553, 234586, 23557
MAIN_HAND, OFF_HAND, RANGED = 14, 15, 16

BEAST_MASTERY = "5320001505101251-00503520014"
MARKSMANSHIP = "50032005021-0050552011503051-"
SURVIVAL = "5023000501-0050550501503051"
SURVIVAL_RANGED = "5-005005001-503200230250222151"

# name, fixture whose request is the base, talents, class options, weapons by slot, targets
CASES = [
    ("beast-mastery-cat", BEAST_MASTERY,
     {"petType": "Cat", "petAttackSpeed": "OneTwo", "petUptime": 1, "ammo": "ThoriumHeadedArrow",
      "quiverBonus": "Speed15"}, {MAIN_HAND: POLEARM, RANGED: BOW}, 1),
    ("beast-mastery-bear-half-uptime", BEAST_MASTERY,
     {"petType": "Bear", "petAttackSpeed": "TwoFour", "petUptime": 0.5, "ammo": "RazorArrow",
      "quiverBonus": "Speed10"}, {MAIN_HAND: POLEARM, RANGED: BOW}, 1),
    ("beast-mastery-scorpid-gun", BEAST_MASTERY,
     {"petType": "Scorpid", "petAttackSpeed": "OneFive", "petUptime": 1, "ammo": "MithrilGyroShot",
      "quiverBonus": "Speed12"}, {MAIN_HAND: SWORD_MH, OFF_HAND: SWORD_OH, RANGED: GUN}, 1),
    ("beast-mastery-gorilla", BEAST_MASTERY,
     {"petType": "Gorilla", "petAttackSpeed": "One", "petUptime": 1, "ammo": "Doomshot",
      "quiverBonus": "QuiverNone"}, {MAIN_HAND: POLEARM, RANGED: CROSSBOW}, 1),
    ("beast-mastery-tallstrider-three-targets", BEAST_MASTERY,
     {"petType": "Tallstrider", "petAttackSpeed": "OneTwo", "petUptime": 1, "ammo": "ThoriumHeadedArrow",
      "quiverBonus": "Speed15"}, {MAIN_HAND: POLEARM, RANGED: BOW}, 3),
    ("beast-mastery-wind-serpent", BEAST_MASTERY,
     {"petType": "WindSerpent", "petAttackSpeed": "OneTwo", "petUptime": 1, "ammo": "ThoriumHeadedArrow",
      "quiverBonus": "Speed15"}, {MAIN_HAND: POLEARM, RANGED: BOW}, 1),
    ("beast-mastery-raptor", BEAST_MASTERY,
     {"petType": "Raptor", "petAttackSpeed": "OneTwo", "petUptime": 1, "ammo": "ThoriumHeadedArrow",
      "quiverBonus": "Speed15"}, {MAIN_HAND: POLEARM, RANGED: BOW}, 1),
    ("beast-mastery-crab", BEAST_MASTERY,
     {"petType": "Crab", "petAttackSpeed": "OneTwo", "petUptime": 1, "ammo": "ThoriumHeadedArrow",
      "quiverBonus": "Speed15"}, {MAIN_HAND: POLEARM, RANGED: BOW}, 1),
    ("marksmanship-no-pet", MARKSMANSHIP,
     {"petType": "PetNone", "petAttackSpeed": "OneTwo", "petUptime": 1, "ammo": "ThoriumHeadedArrow",
      "quiverBonus": "Speed15"}, {MAIN_HAND: POLEARM, RANGED: BOW}, 1),
    ("marksmanship-hyena", MARKSMANSHIP,
     {"petType": "Hyena", "petAttackSpeed": "OneTwo", "petUptime": 1, "ammo": "ThoriumHeadedArrow",
      "quiverBonus": "Speed15"}, {MAIN_HAND: POLEARM, RANGED: BOW}, 1),
    ("survival-melee-cat", SURVIVAL,
     {"petType": "Cat", "petAttackSpeed": "OneTwo", "petUptime": 1, "ammo": "ThoriumHeadedArrow",
      "quiverBonus": "Speed15"}, {MAIN_HAND: SWORD_MH, OFF_HAND: SWORD_OH, RANGED: BOW}, 1),
    ("survival-ranged-five-targets", SURVIVAL_RANGED,
     {"petType": "PetNone", "petAttackSpeed": "OneTwo", "petUptime": 1, "ammo": "ThoriumHeadedArrow",
      "quiverBonus": "Speed15"}, {MAIN_HAND: POLEARM, RANGED: BOW}, 5),
    ("survival-no-weapons", SURVIVAL,
     {"petType": "Turtle", "petAttackSpeed": "OneTwo", "petUptime": 1, "ammo": "AmmoNone",
      "quiverBonus": "QuiverNone"}, {}, 1),
]


def strip(request, talents, options, weapons, targets):
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
    items = [{} for _ in range(17)]
    for slot, item in weapons.items():
        items[slot] = {"id": item}
    player["equipment"] = {"items": items}
    player["rotation"] = {"type": "TypeAPL",
                          "priorityList": [{"action": {"castSpell": {"spellId": {"spellId": 20904}}}}]}
    player["talentsString"] = talents
    player["hunter"]["options"]["classOptions"] = options
    encounter = out["encounter"]
    encounter["targets"] = [copy.deepcopy(encounter["targets"][0]) for _ in range(targets)]
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


def fnv1a(data, basis):
    h = basis
    for byte in data:
        h = ((h ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def digest(value):
    """Two FNV-1a streams, which only have to notice a change, not resist one. The same in
    tests/classes/hunter/prepare.rs."""
    data = canonical(value).encode()
    return f"{fnv1a(data, 0xCBF29CE484222325):016x}{fnv1a(data, 0x84222325CBF29CE4):016x}"


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
    parts["pets"] = [[pet["label"], digest(pet)] for pet in prepared.get("pets", [])]
    for name, value in (
        ("player.stats", player["stats"]),
        ("player.pseudo_stats", player["pseudo_stats"]),
        ("player.mana", player["mana"]),
        ("player.talents", player["talents"]),
        ("encounter", prepared["encounter"]),
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
    base = json.loads((FIXTURES / "production-beast-mastery-hunter.request.json").read_text())
    for name, talents, options, weapons, targets in CASES:
        request = strip(base, talents, options, weapons, targets)
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
