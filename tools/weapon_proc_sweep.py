#!/usr/bin/env python3
"""Write the variants of the weapon proc sweep: every weapon whose proc Go registers through
sim/common/itemhelpers/weaponprocs.go (and Annihilator's debuff proc), worn by the classes that
wield it, casters and druids included, plus the accepted weapon proc fixtures themselves.

    python3 tools/weapon_proc_sweep.py --output <scratch>

writes `<scratch>/NAME-sweep-NN.request.json` for every row below, each the randomized variants
tools/sweep.py draws from an accepted production request with the weapon equipped in place of the
one it carries. The same options always give the same variants. Compare them with
`tools/prepared_v2.py compare`. Go slots a weapon in any hand, so a two hander in the main hand of
a build that dual wields, and the like, are placements the client would not allow; the rows keep
to what the classes can wield.

Uses only Python's standard library.
"""

import argparse
import copy
import json
from pathlib import Path

from sweep import generate

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures" / "mage" / "prepared-v2"

# Weapons by weapon type. A hand's slot is its index in the request's equipment list.
AXES = {"flurry-axe": 871, "annihilator": 12798}
DAGGERS = {"alcors-sunrazor": 14555, "darrowspike": 13984, "coldrage-dagger": 10761,
           "glacial-blade": 19099, "electrified-dagger": 19100, "the-lobotomizer": 19324}
FISTS = {"bloodfist": 11744}
MACES = {"stinging-viper": 6472, "bonechill-hammer": 14487, "masterwork-stormhammer": 12794,
         "the-cruel-hand-of-timmy": 13401}
SWORDS = {"skullforge-reaver": 13361, "plaguefang": 279876, "ebon-hilt-of-marduk": 14576,
          "sword-of-zeal": 6622, "argent-avenger": 13246}
POLEARMS = {"flame-wrath": 11809, "shadowstrike": 17074}
RANGED = {"barbaric-crossbow": 272999, "venomstrike": 6469}

MAIN_HAND, OFF_HAND = 12, 13

# name, accepted request, the weapons it wears, the slot, variants of each, its races
WARRIOR_RACES = ("RaceHuman", "RaceDwarf", "RaceOrc", "RaceUndead", "RaceTroll", "RaceTauren",
                 "RaceNightElf", "RaceGnome")
ROGUE_RACES = ("RaceHuman", "RaceDwarf", "RaceNightElf", "RaceOrc", "RaceUndead", "RaceTroll", "RaceGnome")
PALADIN_RACES = ("RaceHuman", "RaceDwarf")
SHAMAN_RACES = ("RaceOrc", "RaceTauren", "RaceTroll")
HUNTER_RACES = ("RaceDwarf", "RaceNightElf", "RaceOrc", "RaceTauren", "RaceTroll")
DRUID_RACES = ("RaceNightElf", "RaceTauren")
PRIEST_RACES = ("RaceHuman", "RaceDwarf", "RaceNightElf", "RaceUndead", "RaceTroll")
MAGE_RACES = ("RaceHuman", "RaceGnome", "RaceUndead", "RaceTroll")
WARLOCK_RACES = ("RaceHuman", "RaceGnome", "RaceOrc", "RaceUndead")

# (label, base request, weapon table, slot, count, races, ranged hit within melee range only)
WEAPON_ROWS = [
    ("fury-warrior", "production-warrior", {**AXES, **DAGGERS, **FISTS, **MACES, **SWORDS}, MAIN_HAND, 4,
     WARRIOR_RACES),
    ("combat-rogue", "production-combat-rogue", {**DAGGERS, **FISTS, **MACES, **SWORDS}, MAIN_HAND, 4,
     ROGUE_RACES),
    ("combat-rogue-off-hand", "production-combat-rogue",
     {"alcors-sunrazor": 14555, "bloodfist": 11744, "coldrage-dagger": 10761, "stinging-viper": 6472,
      "electrified-dagger": 19100, "plaguefang": 279876}, OFF_HAND, 4, ROGUE_RACES),
    ("protection-paladin", "production-protection-paladin",
     {**AXES, **MACES, **SWORDS}, MAIN_HAND, 4, PALADIN_RACES),
    ("enhancement-shaman", "enhancement-shaman-dual-wield-windfury-flametongue",
     {**AXES, **DAGGERS, **FISTS, **MACES}, MAIN_HAND, 4, SHAMAN_RACES),
    ("survival-hunter", "production-survival-hunter", {**AXES, **DAGGERS, **FISTS, **SWORDS}, MAIN_HAND, 4,
     HUNTER_RACES),
    ("arms-warrior", "production-arms-warrior", POLEARMS, MAIN_HAND, 6, WARRIOR_RACES),
    ("retribution-paladin", "production-retribution-paladin", {**POLEARMS, "wolfsbane": 267369}, MAIN_HAND, 6,
     PALADIN_RACES),
    ("marksmanship-hunter", "production-marksmanship-hunter", RANGED, 13, 6, HUNTER_RACES),
]

# Builds that cast or shapeshift, which wield a weapon whose proc hears nothing they do, or hears
# their melee: label, accepted request, weapons, slot, variants of each, the races to draw. They
# stand at any distance.
CASTER_ROWS = [
    ("feral-druid", "production-feral-druid", {**MACES, **FISTS, "glacial-blade": 19099}, MAIN_HAND, 3,
     DRUID_RACES),
    ("feral-bear-druid", "production-feral-bear-druid", {**MACES, **FISTS}, MAIN_HAND, 3, DRUID_RACES),
    ("balance-druid", "production-balance-druid",
     {"masterwork-stormhammer": 12794, "stinging-viper": 6472, "glacial-blade": 19099}, MAIN_HAND, 3,
     DRUID_RACES),
    ("elemental-shaman", "production-elemental-shaman",
     {"masterwork-stormhammer": 12794, "alcors-sunrazor": 14555}, MAIN_HAND, 3, SHAMAN_RACES),
    ("shadow-priest", "production-shadow-priest", {**MACES, "alcors-sunrazor": 14555, "the-lobotomizer": 19324},
     MAIN_HAND, 3, PRIEST_RACES),
    ("frost-mage", "production-frost-2-targets",
     {**DAGGERS, "skullforge-reaver": 13361, "plaguefang": 279876}, MAIN_HAND, 3, MAGE_RACES),
    ("destruction-warlock", "production-destruction-warlock",
     {**DAGGERS, "skullforge-reaver": 13361, "ebon-hilt-of-marduk": 14576}, MAIN_HAND, 3, WARLOCK_RACES),
]

# Weapons whose proc reaches a target past the first or leaves a damage over time, against 2 to 5
# targets: label, request, weapons, slot, variants of each, races
SEVERAL_TARGET_ROWS = [
    ("fury-warrior-2-targets", "production-warrior-2-targets",
     {"masterwork-stormhammer": 12794, "stinging-viper": 6472, "plaguefang": 279876, "annihilator": 12798,
      "skullforge-reaver": 13361, "ebon-hilt-of-marduk": 14576}, MAIN_HAND, 4, WARRIOR_RACES),
    ("fury-warrior-5-targets", "production-warrior-5-targets",
     {"masterwork-stormhammer": 12794, "flurry-axe": 871, "plaguefang": 279876, "annihilator": 12798},
     MAIN_HAND, 4, WARRIOR_RACES),
    ("arms-warrior-3-targets", "production-arms-warrior-3-targets", POLEARMS, MAIN_HAND, 4, WARRIOR_RACES),
    ("protection-warrior-3-targets", "production-protection-warrior-3-targets",
     {"masterwork-stormhammer": 12794, "stinging-viper": 6472, "annihilator": 12798, "bloodfist": 11744},
     MAIN_HAND, 4, WARRIOR_RACES),
    ("combat-rogue-3-targets", "production-combat-rogue-3-targets",
     {"masterwork-stormhammer": 12794, "stinging-viper": 6472, "plaguefang": 279876}, MAIN_HAND, 4,
     ROGUE_RACES),
    ("survival-hunter-5-targets", "production-survival-hunter-5-targets",
     {"plaguefang": 279876, "annihilator": 12798, "skullforge-reaver": 13361}, MAIN_HAND, 4, HUNTER_RACES),
]

# The accepted fixtures of the weapon procs, as they are.
FIXTURE_ROWS = [
    "production-warrior-masterwork-stormhammer", "warrior-3-targets-masterwork-stormhammer",
    "warrior-bloodfist", "warrior-alcors-sunrazor", "warrior-bonechill-hammer", "warrior-plaguefang",
    "warrior-3-targets-stinging-viper", "warrior-flurry-axe", "warrior-3-targets-annihilator",
    "warrior-sword-of-zeal-bloodfist-off-hand", "warrior-lobotomizer", "warrior-ebon-hilt-of-marduk",
    "arms-warrior-3-targets-flame-wrath", "marksmanship-hunter-barbaric-crossbow",
    "marksmanship-hunter-venomstrike", "retribution-paladin-wolfsbane",
    "combat-rogue-glacial-blade-electrified-dagger", "protection-warrior-3-targets-masterwork-stormhammer",
]
FIXTURE_COUNT = 8

SEED = 20261008
ITERATIONS = 200
# A melee build stays in range; a ranged one or a caster draws any distance.
MELEE_DISTANCE = 5


def request_of(name):
    return json.loads((FIXTURES / f"{name}.request.json").read_text())


def equipped(base, slot, item):
    """A copy of the request with the weapon in the slot, bare."""
    request = copy.deepcopy(base)
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"][slot] = {"id": item}
    return request


def rows():
    """Every row: (name, base request, count, races, max distance), in a fixed order."""
    out = []
    for table in (WEAPON_ROWS, CASTER_ROWS, SEVERAL_TARGET_ROWS):
        for label, base_name, weapons, slot, count, races in table:
            base = request_of(base_name)
            ranged = table is CASTER_ROWS or (
                base["raid"]["parties"][0]["players"][0]["class"] == "ClassHunter" and slot == 13)
            for weapon, item in weapons.items():
                out.append((f"{label}-{weapon}", equipped(base, slot, item), count, races,
                            None if ranged else MELEE_DISTANCE))
    for name in FIXTURE_ROWS:
        base = request_of(name)
        player_class = base["raid"]["parties"][0]["players"][0]["class"]
        races = {"ClassWarrior": WARRIOR_RACES, "ClassRogue": ROGUE_RACES, "ClassPaladin": PALADIN_RACES,
                 "ClassHunter": HUNTER_RACES}[player_class]
        ranged = name.startswith("marksmanship-hunter")
        out.append((f"fixture-{name}", base, FIXTURE_COUNT, races, None if ranged else MELEE_DISTANCE))
    return out


def variants():
    """(file stem, request) for every variant of every row."""
    out = []
    for index, (name, base, count, races, distance) in enumerate(rows()):
        drawn = generate(base, SEED + index, count, ITERATIONS, races, distance)
        for number, request in enumerate(drawn):
            out.append((f"{name}-sweep-{number:02d}", request))
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
