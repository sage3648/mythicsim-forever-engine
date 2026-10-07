#!/usr/bin/env python3
"""Write the Druid preparation goldens: stripped requests and the pinned Go exporter's output.

Each case is a Druid fixture request without buffs, debuffs, consumables, professions and tanks,
a druid with one spell cast as its rotation and the gear the case names (none for most
cases), so what it prepares is the Druid's own: spells, forms, talent auras, class effects,
energy and stats. The Go exporter prepares it and the goldens keep a digest of each part of the
answer, never the answer itself, so a failing test names the spell, aura or effect that changed
and the files stay small.

    python3 tools/druid_prepare_goldens.py --oracle /path/to/forever-go-oracle-v2

The digest is `canonical` below, which tests/classes/druid/prepare.rs implements again in Rust:
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
OUT = ROOT / "tests" / "classes" / "druid" / "prepare"

# The most points each talent takes, by tree (proto.DruidTalents field order).
MOST = (
    [5, 5, 3, 2, 2, 2, 3, 1, 1, 5, 5, 2, 1, 3, 5, 1],
    [5, 5, 2, 3, 2, 3, 3, 2, 1, 2, 1, 1, 3, 2, 2, 1, 2, 5, 5, 1],
    [5, 5, 5, 3, 3, 3, 5, 1, 5, 3, 1, 1, 3, 2, 5, 1],
)


def build(per_talent):
    """A talents string: `per_talent` maps (tree, index) to the points, the rest taking none."""
    trees = []
    for tree, most in enumerate(MOST):
        trees.append("".join(str(min(per_talent(tree, i, m), m)) for i, m in enumerate(most)))
    return "-".join(trees)


def at_most(tree, i, m):
    return m


def at_one(tree, i, m):
    return 1


def mixed(seed):
    state = [seed]

    def points(tree, i, m):
        state[0] = (state[0] * 1103515245 + 12345) & 0x7FFFFFFF
        return (state[0] >> 8) % (m + 1)

    return points


# Spells the rotation casts: Wrath 9912, Shred 9830 and Lacerate 1235827.
WRATH, SHRED, LACERATE = 9912, 9830, 1235827

# name, fixture whose request is the base, talents string (None keeps the fixture's), spell the
# rotation casts, the gear's item IDs
CASES = [
    ("balance", "production-balance-druid", None, WRATH, []),
    ("balance-hybrid", "balance-druid-berserk-natural-reaction", None, WRATH, []),
    ("balance-every-talent-at-max", "production-balance-druid", build(at_most), WRATH, []),
    ("balance-every-talent-at-one", "production-balance-druid", build(at_one), WRATH, []),
    ("balance-mixed", "production-balance-druid", build(mixed(7)), WRATH, []),
    ("balance-idol-of-the-moon-and-cenarion", "production-balance-druid", None, WRATH,
     [16828, 16829, 16830, 16831, 16833, 23197]),
    ("balance-stormrage", "production-balance-druid", None, WRATH, [16900, 16901]),
    ("cat", "production-feral-druid", None, SHRED, []),
    ("cat-wildheart-build", "feral-druid-wildheart-raiment", None, SHRED, []),
    ("cat-every-talent-at-max", "production-feral-druid", build(at_most), SHRED, []),
    ("cat-every-talent-at-one", "production-feral-druid", build(at_one), SHRED, []),
    ("cat-mixed", "production-feral-druid", build(mixed(11)), SHRED, []),
    ("cat-mixed-two", "production-feral-druid", build(mixed(23)), SHRED, []),
    ("cat-feralheart-six-pieces", "production-feral-druid", None, SHRED,
     [22106, 22107, 22108, 22109, 22110, 22111]),
    ("cat-wolfshead-and-ferocity", "production-feral-druid", None, SHRED, [8345, 22397]),
    ("cat-symbols-of-unending-life", "production-feral-druid", None, SHRED, [21407, 21408, 21409]),
    ("cat-champions-refuge", "production-feral-druid", None, SHRED,
     [227202, 227203, 227204, 227205, 227206, 227207]),
    ("cat-three-targets", "feral-druid-3-targets", None, SHRED, []),
    ("bear", "production-feral-bear-druid", None, LACERATE, []),
    ("bear-every-talent-at-max", "production-feral-bear-druid", build(at_most), LACERATE, []),
    ("bear-every-talent-at-one", "production-feral-bear-druid", build(at_one), LACERATE, []),
    ("bear-mixed", "production-feral-bear-druid", build(mixed(31)), LACERATE, []),
    ("bear-idol-of-brutality", "production-feral-bear-druid", None, LACERATE, [23198, 21407]),
    ("bear-warlords-sanctuary", "production-feral-bear-druid", None, LACERATE,
     [231683, 231684, 231685, 231686, 231687, 231688]),
]


def strip(request, talents, spell, items):
    out = copy.deepcopy(request)
    raid = out["raid"]
    for key in ("buffs", "debuffs", "tanks"):
        raid.pop(key, None)
    for target in out["encounter"].get("targets", []):
        target.pop("tankIndex", None)
        target.pop("secondTankIndex", None)
    party = raid["parties"][0]
    party.pop("buffs", None)
    player = party["players"][0]
    for key in ("buffs", "consumables", "bonusStats", "profession1", "profession2"):
        player.pop(key, None)
    if spell != LACERATE:
        # A bear keeps its fixture race: with no gear a Night Elf has too little health to shift.
        player["race"] = "RaceNightElf"
    player["equipment"] = {"items": [{"id": item} for item in items]}
    player["rotation"] = {"type": "TypeAPL",
                          "priorityList": [{"action": {"castSpell": {"spellId": {"spellId": spell}}}}]}
    if talents:
        player["talentsString"] = talents
    return out


def canonical(value):
    """The text the digests are taken of, the same in tests/classes/druid/prepare.rs."""
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
        ("player.energy", player.get("energy")),
        ("player.talents", player["talents"]),
        ("player.health_at_reset", player.get("health_at_reset")),
        ("melee", prepared["melee"]),
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
    for name, fixture, talents, spell, items in CASES:
        base = json.loads((FIXTURES / f"{fixture}.request.json").read_text())
        request = strip(base, talents, spell, items)
        request_path = OUT / f"{name}.request.json"
        request_path.write_text(json.dumps(request, indent=1, sort_keys=True) + "\n")
        with tempfile.TemporaryDirectory() as scratch:
            result = Path(scratch) / "go.json"
            run = subprocess.run([args.oracle, "prepare", "--infile", str(request_path), "--scenario", name,
                                  "--outfile", str(result)], capture_output=True, text=True)
            if run.returncode != 0:
                sys.exit(f"{name}: the Go exporter failed: {run.stderr.strip()[:400]}")
            prepared = json.loads(result.read_text())
        golden = {"case": name, "request_sha256": prepared["request_sha256"], "sections": sections(prepared)}
        (OUT / f"{name}.golden.json").write_text(json.dumps(golden, indent=1, sort_keys=True) + "\n")
        print(f"wrote {name}")


if __name__ == "__main__":
    main()
