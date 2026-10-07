#!/usr/bin/env python3
"""Write the Shaman preparation goldens: stripped requests and the pinned Go exporter's output.

Each case is a Shaman fixture request without buffs, debuffs, consumables, professions and gear
but its weapons (bare, with no enchant), with one Lightning Bolt rotation action, so what it
prepares is the Shaman's own: spells, talent auras, imbues, totems, class effects and stats. The
Go exporter prepares it and the goldens keep a digest of each part of the answer, never the
answer itself, so a failing test names the spell, aura or effect that changed and the files stay
small.

    python3 tools/shaman_prepare_goldens.py --oracle /path/to/forever-go-oracle-v2

The digest is `canonical` below, which tests/classes/shaman/prepare.rs implements again in Rust:
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
OUT = ROOT / "tests" / "classes" / "shaman" / "prepare"

ELEMENTAL = "5502301320123051-05-05305"
ENHANCEMENT = "450033102-055030031005102251"
# Every talent at its last rank (a bool talent at 1), and every talent at one point.
MAX_ELEMENTAL = "5535331323123151--"
MAX_ENHANCEMENT = "-255232331355112251-"
MAX_RESTORATION = "--5533523315513151"
MAX_TALENTS = "5535331323123151-255232331355112251-5533523315513151"
ONE_TALENT = "1111111111111111-111111111111111111-1111111111111111"
ONE_ELEMENTAL = "1111111111111111--"
ONE_ENHANCEMENT = "-111111111111111111-"
ONE_RESTORATION = "--1111111111111111"

# The weapons a case keeps: a dual wield pair, a two hander and a caster's dagger and held item.
DUAL_WIELD = [{"id": 21242}, {"id": 18828}]
TWO_HANDER = [{"id": 12784}]
CASTER = [{"id": 13964}, {"id": 22329}]

ELEMENTAL_FIXTURE = "production-elemental-shaman"
ENHANCEMENT_FIXTURE = "enhancement-shaman-dual-wield-windfury-flametongue"
TWO_HAND_FIXTURE = "production-enhancement-shaman"


def enhancement(imbue_mh, imbue_oh, sync="Auto", **extra):
    options = {"classOptions": {"imbueMh": imbue_mh}, "imbueOh": imbue_oh, "syncType": sync}
    options.update(extra)
    return options


# name, fixture whose request is the base, talents string, the spec's options (None keeps the
# fixture's), the weapons, the number of targets (None keeps the fixture's), whether the spec is
# switched to Restoration. The exporter cannot export a Restoration shaman (it has no auto attacks
# for Windfury Totem's extra attack), so no case does.
CASES = [
    ("elemental", ELEMENTAL_FIXTURE, ELEMENTAL, None, CASTER, None, False),
    ("elemental-three-targets", "elemental-shaman-multidot-flame-shock-3-targets", ELEMENTAL, None, CASTER, None,
     False),
    ("elemental-windfury-imbue", ELEMENTAL_FIXTURE, ELEMENTAL, {"classOptions": {"imbueMh": "WindfuryWeapon"}},
     CASTER, None, False),
    ("elemental-max-talents", ELEMENTAL_FIXTURE, MAX_ELEMENTAL, None, CASTER, None, False),
    ("elemental-one-talent", ELEMENTAL_FIXTURE, ONE_ELEMENTAL, None, CASTER, None, False),
    ("elemental-no-talents", ELEMENTAL_FIXTURE, "", None, CASTER, None, False),
    ("elemental-no-weapon", ELEMENTAL_FIXTURE, ELEMENTAL, None, [], None, False),
    ("enhancement", ENHANCEMENT_FIXTURE, ENHANCEMENT, None, DUAL_WIELD, None, False),
    ("enhancement-two-hander", TWO_HAND_FIXTURE, ENHANCEMENT, None, TWO_HANDER, None, False),
    ("enhancement-no-weapon", TWO_HAND_FIXTURE, ENHANCEMENT, None, [], None, False),
    ("enhancement-windfury-flametongue", ENHANCEMENT_FIXTURE, ENHANCEMENT,
     enhancement("WindfuryWeapon", "FlametongueWeapon"), DUAL_WIELD, None, False),
    ("enhancement-frostbrand-rockbiter", ENHANCEMENT_FIXTURE, ENHANCEMENT,
     enhancement("FrostbrandWeapon", "RockbiterWeapon"), DUAL_WIELD, None, False),
    ("enhancement-flametongue-windfury", ENHANCEMENT_FIXTURE, ENHANCEMENT,
     enhancement("FlametongueWeapon", "WindfuryWeapon"), DUAL_WIELD, None, False),
    ("enhancement-flametongue-flametongue", ENHANCEMENT_FIXTURE, ENHANCEMENT,
     enhancement("FlametongueWeapon", "FlametongueWeapon"), DUAL_WIELD, None, False),
    ("enhancement-windfury-windfury", ENHANCEMENT_FIXTURE, ENHANCEMENT,
     enhancement("WindfuryWeapon", "WindfuryWeapon"), DUAL_WIELD, None, False),
    ("enhancement-frostbrand-windfury", ENHANCEMENT_FIXTURE, ENHANCEMENT,
     enhancement("FrostbrandWeapon", "WindfuryWeapon"), DUAL_WIELD, None, False),
    ("enhancement-rockbiter-rockbiter", ENHANCEMENT_FIXTURE, ENHANCEMENT,
     enhancement("RockbiterWeapon", "RockbiterWeapon"), DUAL_WIELD, None, False),
    ("enhancement-no-imbue", ENHANCEMENT_FIXTURE, ENHANCEMENT, enhancement("NoImbue", "NoImbue"), DUAL_WIELD, None,
     False),
    ("enhancement-imbue-swap", ENHANCEMENT_FIXTURE, ENHANCEMENT,
     {"classOptions": {"imbueMh": "RockbiterWeapon", "imbueMhSwap": "WindfuryWeapon"}, "imbueOh": "NoImbue",
      "imbueOhSwap": "FlametongueWeapon", "syncType": "Auto"}, DUAL_WIELD, None, False),
    ("enhancement-rockbiter-two-hander", TWO_HAND_FIXTURE, ENHANCEMENT, enhancement("RockbiterWeapon", "NoImbue"),
     TWO_HANDER, None, False),
    ("enhancement-windfury-two-hander", TWO_HAND_FIXTURE, ENHANCEMENT, enhancement("WindfuryWeapon", "NoImbue"),
     TWO_HANDER, None, False),
    ("enhancement-imbue-without-oh", TWO_HAND_FIXTURE, ENHANCEMENT, enhancement("FlametongueWeapon", "WindfuryWeapon"),
     TWO_HANDER, None, False),
    ("enhancement-sync", ENHANCEMENT_FIXTURE, ENHANCEMENT, enhancement("WindfuryWeapon", "FlametongueWeapon", "SyncMainhandOffhandSwings"),
     DUAL_WIELD, None, False),
    ("enhancement-delay", ENHANCEMENT_FIXTURE, ENHANCEMENT, enhancement("WindfuryWeapon", "FlametongueWeapon", "DelayOffhandSwings"),
     DUAL_WIELD, None, False),
    ("enhancement-no-sync", ENHANCEMENT_FIXTURE, ENHANCEMENT, enhancement("WindfuryWeapon", "FlametongueWeapon", "NoSync"),
     DUAL_WIELD, None, False),
    ("enhancement-three-targets", "production-enhancement-shaman-3-targets", ENHANCEMENT,
     enhancement("WindfuryWeapon", "FlametongueWeapon"), DUAL_WIELD, None, False),
    ("enhancement-max-talents", ENHANCEMENT_FIXTURE, MAX_ENHANCEMENT, enhancement("WindfuryWeapon", "FlametongueWeapon"),
     DUAL_WIELD, None, False),
    ("enhancement-max-talents-rockbiter", ENHANCEMENT_FIXTURE, MAX_ENHANCEMENT,
     enhancement("RockbiterWeapon", "FrostbrandWeapon"), DUAL_WIELD, None, False),
    ("enhancement-one-talent", ENHANCEMENT_FIXTURE, ONE_ENHANCEMENT, enhancement("WindfuryWeapon", "FlametongueWeapon"),
     DUAL_WIELD, None, False),
    ("enhancement-no-talents", ENHANCEMENT_FIXTURE, "", enhancement("WindfuryWeapon", "FlametongueWeapon"),
     DUAL_WIELD, None, False),
    ("every-talent-at-max", ENHANCEMENT_FIXTURE, MAX_TALENTS, enhancement("WindfuryWeapon", "FlametongueWeapon"),
     DUAL_WIELD, None, False),
    ("every-talent-at-one", ENHANCEMENT_FIXTURE, ONE_TALENT, enhancement("WindfuryWeapon", "FlametongueWeapon"),
     DUAL_WIELD, None, False),
]


def merge(into, update):
    for key, value in update.items():
        if isinstance(value, dict):
            merge(into.setdefault(key, {}), value)
        else:
            into[key] = value


def strip(request, talents, options, weapons, restoration):
    out = copy.deepcopy(request)
    raid = out["raid"]
    raid.pop("buffs", None)
    raid.pop("debuffs", None)
    party = raid["parties"][0]
    party.pop("buffs", None)
    player = party["players"][0]
    for key in ("buffs", "consumables", "bonusStats", "profession1", "profession2"):
        player.pop(key, None)
    player["equipment"] = {"items": copy.deepcopy(weapons)}
    player["rotation"] = {"type": "TypeAPL",
                          "priorityList": [{"action": {"castSpell": {"spellId": {"spellId": 15208}}}}]}
    player["talentsString"] = talents
    if restoration:
        player.pop("elementalShaman", None)
        player["restorationShaman"] = {"options": {"classOptions": {}}}
    spec = next(key for key in ("elementalShaman", "enhancementShaman", "restorationShaman") if key in player)
    if options is not None:
        merge(player[spec]["options"], options)
    return out


def canonical(value):
    """The text the digests are taken of, the same in tests/classes/shaman/prepare.rs."""
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
        ("melee", prepared["melee"]),
        ("encounter", prepared["encounter"]),
        ("unrepresented", prepared["unrepresented"]),
    ):
        parts[name] = digest(value)
    return parts


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--oracle", required=True, help="the pinned Go exporter, forever-go-oracle-v2")
    parser.add_argument("--only", help="write only the case with this name")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    for name, fixture, talents, options, weapons, targets, restoration in CASES:
        if args.only and args.only != name:
            continue
        base = json.loads((FIXTURES / f"{fixture}.request.json").read_text())
        request = strip(base, talents, options, weapons, restoration)
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
