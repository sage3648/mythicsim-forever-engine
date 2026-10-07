#!/usr/bin/env python3
"""Write the Priest preparation goldens: stripped requests and the pinned Go exporter's output.

Each case is a Priest fixture request without buffs, debuffs, consumables, professions and gear
and with one cast action in its rotation, so what it prepares is the Priest's own: spells,
talent auras, class effects, the Shadowfiend and stats. The Go exporter prepares it and the
goldens keep a digest of each part of the answer, never the answer itself, so a failing test
names the spell, aura or effect that changed and the files stay small.

    python3 tools/priest_prepare_goldens.py --oracle /path/to/forever-go-oracle-v2

The digest is `canonical` below, which tests/classes/priest/prepare.rs implements again in Rust:
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
OUT = ROOT / "tests" / "classes" / "priest" / "prepare"

SHADOW = "025300011303--500222501201302251"
SMITE = "505030031300001-305051130020003-50002"
MAX = "525333231335121531-33555133232121531-555322521221312251"
ONE = "111111111111111111-11111111111111111-111111111111111111"

MIND_BLAST = 10947
SMITE_RANK_8 = 10934

# name, fixture whose request is the base, talents string, race, class options, cast spell
CASES = [
    ("shadow-undead-shadowfiend", "shadow-priest-shadowfiend", SHADOW, "RaceUndead",
     {"useShadowfiend": True}, MIND_BLAST),
    ("shadow-undead-no-shadowfiend", "shadow-priest-shadowfiend", SHADOW, "RaceUndead", {}, MIND_BLAST),
    ("shadow-night-elf-preshadowform", "shadow-priest-shadowfiend", SHADOW, "RaceNightElf",
     {"useShadowfiend": True, "preShadowform": True}, MIND_BLAST),
    ("shadow-human-inner-fire", "shadow-priest-shadowfiend", SHADOW, "RaceHuman",
     {"armor": "InnerFire"}, MIND_BLAST),
    ("shadow-three-targets", "shadow-priest-3-targets-multidot-reads", SHADOW, "RaceUndead",
     {"useShadowfiend": True}, MIND_BLAST),
    ("smite-holy-nova", "smite-priest-holy-nova", SMITE, "RaceHuman", {}, SMITE_RANK_8),
    ("smite-power-infusion", "smite-priest-power-infusion", "515030031305001031-00505023002-003",
     "RaceUndead", {}, SMITE_RANK_8),
    ("smite-no-inner-focus", "smite-priest-no-inner-focus", "505030030300001-305051130020003-50002",
     "RaceDwarf", {}, SMITE_RANK_8),
    ("every-talent-at-max", "shadow-priest-shadowfiend", MAX, "RaceUndead",
     {"useShadowfiend": True, "preShadowform": True, "armor": "InnerFire"}, MIND_BLAST),
    ("every-talent-at-one", "shadow-priest-shadowfiend", ONE, "RaceNightElf",
     {"useShadowfiend": True}, MIND_BLAST),
    ("mixed-build", "shadow-priest-shadowfiend", "203000100003100030-01554030002011101-451022501020001241",
     "RaceHuman", {"useShadowfiend": True}, MIND_BLAST),
    ("mixed-build-two", "shadow-priest-shadowfiend", "214322010024111200-31213100221121430-023002511121201110",
     "RaceUndead", {"preShadowform": True}, MIND_BLAST),
    ("mixed-build-three", "shadow-priest-shadowfiend", "301211131013121131-32531001012001411-201321421020311130",
     "RaceTroll", {"useShadowfiend": True, "armor": "InnerFire"}, MIND_BLAST),
]


def strip(request, talents, race, options, spell):
    out = copy.deepcopy(request)
    raid = out["raid"]
    raid.pop("buffs", None)
    raid.pop("debuffs", None)
    party = raid["parties"][0]
    party.pop("buffs", None)
    player = party["players"][0]
    for key in ("buffs", "consumables", "bonusStats", "profession1", "profession2"):
        player.pop(key, None)
    player["race"] = race
    player["equipment"] = {"items": []}
    player["rotation"] = {"type": "TypeAPL",
                          "priorityList": [{"action": {"castSpell": {"spellId": {"spellId": spell}}}}]}
    player["talentsString"] = talents
    player["dpsPriest"] = {"options": {"classOptions": options}}
    return out


def canonical(value):
    """The text the digests are taken of, the same in tests/classes/priest/prepare.rs."""
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


def label_pet(pet):
    return pet["label"]


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
        ("pets", prepared.get("pets", []), label_pet),
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
    for name, fixture, talents, race, options, spell in CASES:
        base = json.loads((FIXTURES / f"{fixture}.request.json").read_text())
        request = strip(base, talents, race, options, spell)
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
