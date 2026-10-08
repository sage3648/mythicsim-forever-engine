#!/usr/bin/env python3
"""Write the variants of the external cooldown and warrior taunt sweep: Innervate and Mana Tide
Totem that druids and shamans in the raid cast on the player (individualBuffs.innervates and
partyBuffs.manaTideTotems) for every class, with one, two and three sources, and Taunt and
Intimidating Shout cast by Protection, Arms and Fury Warriors against 1, 2, 3 and 5 targets, plus
the accepted fixtures of all four.

    python3 tools/external_cooldown_sweep.py --output <scratch>

writes `<scratch>/NAME-sweep-NN.request.json` for every row below, each the randomized variants
tools/sweep.py draws from an accepted request. The sources of a variant follow its position, so
every count appears in every row; half of the Innervate variants run without mana consumables,
Wisdom and Mana Spring Totem so that the mana falls far enough to be innervated, and all of them
run at least five minutes. A warrior casts the taunt and the shout at the first target or at a
random one. The same options always give the same variants. Compare them with
`tools/prepared_v2.py compare`.

Uses only Python's standard library.
"""

import argparse
import copy
import json
from pathlib import Path
import random

from sweep import generate

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures" / "mage" / "prepared-v2"

INNERVATE = 29166
TAUNT = 355
INTIMIDATING_SHOUT = 5246
DEFENSIVE_STANCE = 71

# The sources of a variant by position.
SOURCES = (1, 2, 3)
# The shortest fight of an Innervate variant, long enough to fall to the threshold and to cast twice.
INNERVATE_DURATION = 300

MAGE_RACES = ("RaceHuman", "RaceGnome", "RaceUndead", "RaceTroll")
WARLOCK_RACES = ("RaceHuman", "RaceGnome", "RaceOrc", "RaceUndead")
PRIEST_RACES = ("RaceHuman", "RaceDwarf", "RaceNightElf", "RaceUndead", "RaceTroll")
PALADIN_RACES = ("RaceHuman", "RaceDwarf")
HUNTER_RACES = ("RaceDwarf", "RaceNightElf", "RaceOrc", "RaceTauren", "RaceTroll")
DRUID_RACES = ("RaceNightElf", "RaceTauren")
SHAMAN_RACES = ("RaceOrc", "RaceTauren", "RaceTroll")
WARRIOR_RACES = ("RaceHuman", "RaceDwarf", "RaceOrc", "RaceUndead", "RaceTroll", "RaceTauren",
                 "RaceNightElf", "RaceGnome")
ROGUE_RACES = ("RaceHuman", "RaceDwarf", "RaceNightElf", "RaceGnome", "RaceOrc", "RaceUndead", "RaceTroll")
# A melee build stays in range.
MELEE_DISTANCE = 5

# label, accepted request, races, max distance. Every class, the mana classes with the casters at
# any range, and a warrior and a rogue, which have no mana at all.
CLASS_BASES = [
    ("balance-druid", "production-balance-druid", DRUID_RACES, None),
    ("feral-druid", "production-feral-druid", DRUID_RACES, MELEE_DISTANCE),
    ("shadow-priest", "production-shadow-priest", PRIEST_RACES, None),
    ("smite-priest", "production-smite-priest", PRIEST_RACES, None),
    ("fire-mage", "production-fire", MAGE_RACES, None),
    ("frost-mage", "production-frost-2-targets", MAGE_RACES, None),
    ("destruction-warlock", "production-destruction-warlock", WARLOCK_RACES, None),
    ("affliction-warlock", "production-affliction-warlock", WARLOCK_RACES, None),
    ("demonology-warlock", "production-demonology-warlock", WARLOCK_RACES, None),
    ("marksmanship-hunter", "production-marksmanship-hunter", HUNTER_RACES, None),
    ("beast-mastery-hunter", "production-beast-mastery-hunter", HUNTER_RACES, None),
    ("survival-hunter", "production-survival-hunter", HUNTER_RACES, MELEE_DISTANCE),
    ("retribution-paladin", "production-retribution-paladin", PALADIN_RACES, MELEE_DISTANCE),
    ("holy-paladin", "production-holy-protection-paladin", PALADIN_RACES, MELEE_DISTANCE),
    ("shockadin-paladin", "production-shockadin-paladin", PALADIN_RACES, MELEE_DISTANCE),
    ("elemental-shaman", "production-elemental-shaman", SHAMAN_RACES, None),
    ("enhancement-shaman", "production-enhancement-shaman", SHAMAN_RACES, MELEE_DISTANCE),
    ("fury-warrior", "production-warrior", WARRIOR_RACES, MELEE_DISTANCE),
    ("combat-rogue", "production-combat-rogue", ROGUE_RACES, MELEE_DISTANCE),
]
CLASS_VARIANTS = 6

# The position of Mana Tide Totem in the Restoration tree of a talents string.
MANA_TIDE_TALENT = 11
# label, accepted request, variants: a shaman whose own talented totem is a party buff too.
TIDE_TALENT_BASES = [
    ("elemental-shaman-tide-talent", "production-elemental-shaman", 6),
]

# label, accepted request, targets, races, variants. The target count copies the first target.
TAUNT_BASES = [
    ("protection-warrior", "production-protection-warrior", 1, 5),
    ("protection-warrior-2-targets", "production-protection-warrior", 2, 4),
    ("protection-warrior-3-targets", "production-protection-warrior-3-targets", 3, 5),
    ("protection-warrior-5-targets", "production-protection-warrior-5-targets", 5, 5),
    ("fury-protection-warrior", "production-fury-protection-warrior", 1, 4),
    ("fury-protection-warrior-3-targets", "production-fury-protection-warrior-3-targets", 3, 4),
    ("arms-warrior", "production-arms-warrior", 1, 5),
    ("arms-warrior-2-targets", "production-arms-warrior-2-targets", 2, 4),
    ("arms-warrior-3-targets", "production-arms-warrior-3-targets", 3, 5),
    ("arms-warrior-5-targets", "production-arms-warrior-3-targets", 5, 4),
    ("fury-warrior", "production-warrior", 1, 4),
    ("fury-warrior-5-targets", "production-warrior-5-targets", 5, 4),
]

# The accepted fixtures with their sources, in the fights the draw gives them.
FIXTURES_OF_THE_SWEEP = [
    "balance-druid-external-innervate", "balance-druid-external-innervate-2-sources",
    "balance-druid-own-and-external-innervate", "fire-mage-evocation-over-external-innervate",
    "enhancement-shaman-external-mana-tide-mp5-drift",
    "shadow-priest-external-innervate", "shadow-priest-external-innervate-2-sources",
    "fire-mage-external-innervate", "fire-mage-external-innervate-2-sources",
    "fire-mage-external-mana-tide", "fire-mage-external-mana-tide-2-sources",
    "destruction-warlock-external-mana-tide", "destruction-warlock-external-mana-tide-2-sources",
    "retribution-paladin-external-mana-tide", "retribution-paladin-external-mana-tide-2-sources",
    "protection-warrior-taunt", "protection-warrior-taunt-3-targets",
    "protection-warrior-intimidating-shout", "protection-warrior-intimidating-shout-3-targets",
    "arms-warrior-taunt", "arms-warrior-taunt-3-targets",
    "arms-warrior-intimidating-shout", "arms-warrior-intimidating-shout-3-targets",
]
FIXTURE_VARIANTS = 3

SEED = 20261012
ITERATIONS = 200


def request_of(name):
    return json.loads((FIXTURES / f"{name}.request.json").read_text())


def player_of(request):
    return request["raid"]["parties"][0]["players"][0]


def rotation_of(request):
    return player_of(request)["rotation"]["priorityList"]


def with_targets(request, count):
    """A copy of the request with the given number of copies of its first target."""
    request = copy.deepcopy(request)
    targets = request["encounter"]["targets"]
    request["encounter"]["targets"] = [copy.deepcopy(targets[0]) for _ in range(count)]
    return request


def cast(spell, target=None):
    """A rotation item that casts the spell, at the given target index when there is one."""
    action = {"castSpell": {"spellId": {"spellId": spell}}}
    if target:
        action["castSpell"]["target"] = {"type": "Target", "index": target}
    return {"action": action}


def with_innervates(request, sources):
    request = copy.deepcopy(request)
    player_of(request).setdefault("buffs", {})["innervates"] = sources
    return request


def with_mana_tides(request, sources):
    request = copy.deepcopy(request)
    request["raid"]["parties"][0].setdefault("buffs", {})["manaTideTotems"] = sources
    return request


def with_mana_tide_talent(request):
    """The request whose talents include Mana Tide Totem, which adds a totem to the party buffs."""
    request = copy.deepcopy(request)
    player = player_of(request)
    trees = player["talentsString"].split("-")
    trees += [""] * (3 - len(trees))
    restoration = list(trees[2].ljust(MANA_TIDE_TALENT + 1, "0"))
    restoration[MANA_TIDE_TALENT] = "1"
    trees[2] = "".join(restoration)
    player["talentsString"] = "-".join(trees)
    return request


def starved(request):
    """The request without the mana consumables, Wisdom and Mana Spring Totem."""
    request = copy.deepcopy(request)
    player = player_of(request)
    player["consumables"] = {key: value for key, value in player.get("consumables", {}).items()
                             if key in ("flaskId", "foodId")}
    player.get("buffs", {}).pop("greaterBlessingOfWisdom", None)
    request["raid"]["parties"][0].get("buffs", {}).pop("manaSpringTotem", None)
    return request


def at_least(request, seconds):
    """The request whose fight lasts at least that long; the variation stays shorter."""
    request = copy.deepcopy(request)
    encounter = request["encounter"]
    encounter["duration"] = max(encounter["duration"], seconds)
    return request


def innervate_fix(number):
    """What a variant keeps of the Innervate row: its sources by position, the mana pressure of
    every other one and a long fight."""
    def fix(request):
        request = with_innervates(request, SOURCES[number % len(SOURCES)])
        request = at_least(request, INNERVATE_DURATION)
        return starved(request) if number % 2 == 0 else request
    return fix


def tide_fix(number):
    def fix(request):
        return with_mana_tides(request, SOURCES[number % len(SOURCES)])
    return fix


def taunt_fix(number, targets):
    """A variant casts the taunt and the shout at the first target or at one the number picks."""
    def fix(request):
        request = copy.deepcopy(request)
        target = random.Random(f"taunt-{number}-{targets}").randrange(targets)
        rotation = rotation_of(request)
        items = [cast(TAUNT, target), cast(INTIMIDATING_SHOUT, target)]
        # Taunt needs Defensive Stance, which the rotation changes to first unless it already does.
        stance = [position for position, item in enumerate(rotation)
                  if item["action"].get("castSpell", {}).get("spellId", {}).get("spellId") == DEFENSIVE_STANCE]
        if stance:
            rotation[stance[0] + 1:stance[0] + 1] = items
        else:
            rotation[0:0] = [cast(DEFENSIVE_STANCE)] + items
        return request
    return fix


def rows():
    """Every row: (name, base request, count, races, max distance, fix(number)), in a fixed order."""
    out = []
    for label, base_name, races, distance in CLASS_BASES:
        base = request_of(base_name)
        out.append((f"{label}-innervate", base, CLASS_VARIANTS, races, distance, innervate_fix))
        out.append((f"{label}-mana-tide", base, CLASS_VARIANTS, races, distance, tide_fix))
    for label, base_name, count in TIDE_TALENT_BASES:
        out.append((label, with_mana_tide_talent(request_of(base_name)), count, SHAMAN_RACES, None,
                    tide_fix))
    for label, base_name, targets, count in TAUNT_BASES:
        base = with_targets(request_of(base_name), targets)
        out.append((f"{label}-taunts", base, count, WARRIOR_RACES, MELEE_DISTANCE,
                    lambda number, targets=targets: taunt_fix(number, targets)))
    for name in FIXTURES_OF_THE_SWEEP:
        if "mana-tide" in name:
            sources = int(name.split("-sources")[0].split("-")[-1]) if "-sources" in name else 1
            fix = lambda number, sources=sources: (lambda request: with_mana_tides(request, sources))
        elif "innervate" in name:
            fix = lambda number: (lambda request: at_least(request, INNERVATE_DURATION))
        else:
            fix = None
        base = request_of(name)
        races = (WARRIOR_RACES if "warrior" in name else PALADIN_RACES if "paladin" in name else
                 WARLOCK_RACES if "warlock" in name else DRUID_RACES if "druid" in name else
                 PRIEST_RACES if "priest" in name else SHAMAN_RACES if "shaman" in name else MAGE_RACES)
        melee = MELEE_DISTANCE if any(word in name for word in ("warrior", "paladin", "enhancement")) else None
        out.append((f"fixture-{name}", base, FIXTURE_VARIANTS, races, melee, fix))
    return out


def variants():
    """(file stem, request) for every variant of every row."""
    out = []
    for index, (name, base, count, races, distance, fix) in enumerate(rows()):
        drawn = generate(base, SEED + index, count, ITERATIONS, races, distance)
        for number, request in enumerate(drawn):
            out.append((f"{name}-sweep-{number:02d}", fix(number)(request) if fix else request))
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    written = variants()
    for stem, request in written:
        (args.output / f"{stem}.request.json").write_text(json.dumps(request, indent=2) + "\n")
    print(f"Wrote {len(written)} variants of {len(rows())} requests to {args.output}")


if __name__ == "__main__":
    main()
