#!/usr/bin/env python3
"""Write the Warlock preparation goldens: stripped requests and the pinned Go exporter's output.

Each case is a Warlock fixture request without buffs, debuffs, consumables, professions and
gear, a Human warlock with one Shadow Bolt rotation action, so what it prepares is the
Warlock's own: spells, talent auras, demons, class effects and stats. The Go exporter prepares
it and the goldens keep a digest of each part of the answer, never the answer itself, so a
failing test names the spell, aura or effect that changed and the files stay small.

    python3 tools/warlock_prepare_goldens.py --oracle /path/to/forever-go-oracle-v2

The digest is `canonical` below, which tests/classes/warlock/prepare.rs implements again in
Rust: objects by sorted key, numbers as the IEEE 754 bits of their float64 value, so Go's `1`
and Rust's `1.0` agree.
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
OUT = ROOT / "tests" / "classes" / "warlock" / "prepare"

AFFLICTION = "0555002013520005--0550005003"
DEMONOLOGY = "02-0025003231020301351-0550005003"
DESTRUCTION = "2522-0050203001-155030510310005"
# Every talent at its last rank, and every talent at one point.
MAX_TALENTS = "25552323135211351-2355233231221331351-2555355133121351"
ONE_TALENT = "11111111111111111-1111111111111111111-1111111111111111"

# name, fixture whose request is the base, talents string, options of the class (None keeps the
# fixture's), and the number of targets (None keeps the fixture's), and whether the race stays
CASES = [
    ("affliction-succubus", "production-affliction-warlock", AFFLICTION, {}, None, False),
    ("affliction-imp", "production-affliction-warlock", AFFLICTION, {"summon": "Imp"}, None, False),
    ("affliction-voidwalker", "production-affliction-warlock", AFFLICTION, {"summon": "Voidwalker"}, None, False),
    ("affliction-felhunter", "production-affliction-warlock", AFFLICTION, {"summon": "Felhunter"}, None, False),
    ("affliction-no-summon", "production-affliction-warlock", AFFLICTION, {"summon": "NoSummon"}, None, False),
    ("affliction-gnome", "production-affliction-warlock", AFFLICTION, {}, None, True),
    ("demonology-gnome", "production-demonology-warlock", DEMONOLOGY, {}, None, True),
    ("affliction-curse-of-elements", "production-affliction-warlock", AFFLICTION,
     {"curseOptions": "Elements"}, None, False),
    ("affliction-curse-of-recklessness", "production-affliction-warlock", AFFLICTION,
     {"curseOptions": "Recklessness"}, None, False),
    ("affliction-curse-of-doom", "production-affliction-warlock", AFFLICTION,
     {"curseOptions": "Doom"}, None, False),
    ("affliction-no-armor", "production-affliction-warlock", AFFLICTION, {"armor": "NoArmor"}, None, False),
    ("affliction-three-targets", "affliction-warlock-3-targets-multidot-dots", AFFLICTION, {}, None, False),
    ("demonology-succubus", "production-demonology-warlock", DEMONOLOGY, {}, None, False),
    ("demonology-imp", "production-demonology-warlock", DEMONOLOGY, {"summon": "Imp"}, None, False),
    ("demonology-voidwalker", "production-demonology-warlock", DEMONOLOGY, {"summon": "Voidwalker"}, None, False),
    ("demonology-felhunter", "production-demonology-warlock", DEMONOLOGY, {"summon": "Felhunter"}, None, False),
    ("demonology-pact-voidwalker", "production-demonology-warlock", DEMONOLOGY,
     {"pactSacrifice": "Voidwalker"}, None, False),
    ("demonology-pact-succubus", "production-demonology-warlock", DEMONOLOGY,
     {"summon": "Imp", "pactSacrifice": "Succubus"}, None, False),
    ("demonology-sacrificed-imp", "production-demonology-warlock", DEMONOLOGY,
     {"summon": "Imp", "sacrificeSummon": True}, None, False),
    ("demonology-sacrificed-voidwalker", "production-demonology-warlock", DEMONOLOGY,
     {"summon": "Voidwalker", "sacrificeSummon": True}, None, False),
    ("demonology-sacrificed-succubus", "production-demonology-warlock", DEMONOLOGY,
     {"summon": "Succubus", "sacrificeSummon": True}, None, False),
    ("demonology-two-targets", "demonology-warlock-2-targets-hellfire", DEMONOLOGY, {}, None, False),
    ("destruction-imp", "destruction-warlock-broad", DESTRUCTION, {"sacrificeSummon": False}, None, False),
    ("destruction-sacrificed-imp", "production-destruction-warlock", DESTRUCTION, {}, None, False),
    ("destruction-succubus", "destruction-warlock-broad", DESTRUCTION,
     {"summon": "Succubus", "sacrificeSummon": False}, None, False),
    ("destruction-bane-of-havoc", "destruction-warlock-3-targets-bane-of-havoc", "2522-0050203001-155030510310105",
     {}, None, False),
    ("destruction-five-targets", "destruction-warlock-5-targets-rain-of-fire", DESTRUCTION, {}, None, False),
    ("every-talent-at-max", "production-affliction-warlock", MAX_TALENTS, {}, None, False),
    ("every-talent-at-max-imp", "production-affliction-warlock", MAX_TALENTS, {"summon": "Imp"}, None, False),
    ("every-talent-at-one", "production-affliction-warlock", ONE_TALENT, {}, None, False),
    ("every-talent-at-one-sacrificed", "production-affliction-warlock", ONE_TALENT,
     {"summon": "Imp", "sacrificeSummon": True}, None, False),
]


def strip(request, talents, options, keep_race):
    out = copy.deepcopy(request)
    raid = out["raid"]
    raid.pop("buffs", None)
    raid.pop("debuffs", None)
    party = raid["parties"][0]
    party.pop("buffs", None)
    player = party["players"][0]
    for key in ("buffs", "consumables", "bonusStats", "profession1", "profession2"):
        player.pop(key, None)
    if not keep_race:
        player["race"] = "RaceHuman"
    player["equipment"] = {"items": []}
    player["rotation"] = {"type": "TypeAPL",
                          "priorityList": [{"action": {"castSpell": {"spellId": {"spellId": 25307}}}}]}
    player["talentsString"] = talents
    class_options = player["warlock"]["options"]["classOptions"]
    class_options.update(options)
    return out


def canonical(value):
    """The text the digests are taken of, the same in tests/classes/warlock/prepare.rs."""
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
    for name, fixture, talents, options, targets, keep_race in CASES:
        base = json.loads((FIXTURES / f"{fixture}.request.json").read_text())
        request = strip(base, talents, options, keep_race)
        if targets is not None:
            request["encounter"]["targets"] = request["encounter"]["targets"][:1] * targets
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
