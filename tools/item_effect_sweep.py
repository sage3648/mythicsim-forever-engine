#!/usr/bin/env python3
"""Sweep the item and enchant effects Go registers in code, and list the ones Rust refuses.

    python3 tools/item_effect_sweep.py variants --output <scratch>
    python3 tools/item_effect_sweep.py survey --output validation/RECORD.json

`variants` writes `<scratch>/NAME-sweep-NN.request.json` for every row below, each the randomized
variants tools/sweep.py draws from an accepted production request with the item equipped in the
slot its type names, in place of the entry of that type the request holds: a trinket takes a
ring's place, a weapon the main hand's, since appending an entry equips nothing the list already
fills. The same options always give the same variants. Compare them with
`tools/prepared_v2.py compare`.

`survey` puts every item and enchant of data/go-tables.json (`item_effect_ids` and
`enchant_effect_ids`, the effects Go registers in code) in its real slot of one request for each
of nine classes, prepares each in Rust and runs the coverage gate, and writes a record of the
verdict of every pair and of each refusal's code and reason.

Uses only Python's standard library.
"""

import argparse
from collections import Counter
import copy
from concurrent.futures import ThreadPoolExecutor
from datetime import date
import json
from pathlib import Path
import subprocess
import tempfile

from compare import PIN
from sweep import generate

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures" / "mage" / "prepared-v2"
DATA = ROOT / "data"
NOT_PREPARED = 5

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

# Accepted production requests by role, with the races their class draws from.
TANKS = [("production-protection-warrior-3-targets", WARRIOR_RACES),
         ("production-protection-paladin", PALADIN_RACES),
         ("production-feral-bear-druid", DRUID_RACES)]
MELEE = [("production-warrior", WARRIOR_RACES),
         ("production-combat-rogue-3-targets", ROGUE_RACES),
         ("production-retribution-paladin", PALADIN_RACES),
         ("enhancement-shaman-dual-wield-windfury-flametongue", SHAMAN_RACES),
         ("production-survival-hunter", HUNTER_RACES),
         ("production-feral-druid", DRUID_RACES)]
RANGED = [("production-marksmanship-hunter", HUNTER_RACES), ("production-survival-hunter", HUNTER_RACES)]
CASTERS = [("production-frost-2-targets", MAGE_RACES), ("production-destruction-warlock", WARLOCK_RACES),
           ("production-shadow-priest", PRIEST_RACES), ("production-elemental-shaman", SHAMAN_RACES),
           ("production-balance-druid", DRUID_RACES)]
EVERY_CLASS = TANKS[:1] + MELEE + CASTERS + [("production-protection-paladin", PALADIN_RACES)]

# name, the items of the group by name, the requests they are worn on, variants of each, whether the
# build stands in melee range
ITEM_ROWS = [
    ("stacking-armor-penetration", {"bonereavers-edge": 17076}, MELEE + TANKS[1:2] + RANGED[:1], 2, True),
    ("thunderfury", {"thunderfury": 19019}, MELEE + TANKS, 2, True),
    ("thunderfury-caster", {"thunderfury": 19019}, CASTERS[:2], 2, False),
    ("struck-damage", {"totem-of-infliction": 1131, "skullflame-shield": 1168, "vile-protector": 7747,
                       "thermaplugg": 9458, "girdle-of-reprisal": 11861, "grand-marshals-aegis": 18825,
                       "high-warlords-shield-wall": 18826, "premier-grand-marshals-aegis": 272838,
                       "premier-high-warlords-shield-wall": 272591}, TANKS + MELEE[:1], 2, True),
    ("melee-damage-procs", {"red-whelp-gloves": 7284, "fiery-plate-gauntlets": 12631, "blazefury": 17111,
                            "force-imbued-gauntlets": 18383, "thorncursed-grips": 274159,
                            "coldflame-saber": 276631}, MELEE, 2, True),
    ("ranged-damage-procs", {"hurricane": 2824, "copper-bombs": 285275, "bronze-bombs": 285276,
                             "iron-bombs": 285277, "dark-iron-bombs": 285278}, RANGED, 3, False),
    ("spell-damage-procs", {"swine-fists": 10760, "cursed-murloc-eye": 273840, "searing-dagger": 273003},
     CASTERS + MELEE[:1], 2, False),
    ("struck-stat-and-heal-procs", {"the-green-tower": 1204, "wall-of-the-dead": 1979, "painwalker-buckler": 274290,
                                    "truesilver-breastplate": 7939}, TANKS + MELEE[:1], 2, True),
    ("cast-stat-procs", {"darkmoon-blue-dragon": 19288, "wrath-of-cenarius": 21190}, CASTERS, 2, False),
    ("damage-shields", {"naglering": 11669, "drillborer-disk": 17066, "razor-gauntlets": 18326,
                        "essence-of-the-pure-flame": 18815, "freezing-band": 942,
                        "force-reactive-disk": 18168}, TANKS, 2, True),
    ("darkmoon-cards", {"darkmoon-heroism": 19287, "darkmoon-maelstrom": 19289}, MELEE + TANKS, 2, True),
    ("on-use-stat-items", {"earthstrike": 21180, "slayers-crest": 23041, "eye-of-moam": 21473,
                           "charm-of-valor": 19952}, MELEE[:3] + TANKS[:2] + CASTERS[:2], 2, True),
    ("jom-gabbar", {"jom-gabbar": 23570}, MELEE[:3] + MELEE[4:5] + TANKS[1:2] + CASTERS[:2], 2, True),
    ("other-class-idols-librams-totems", {"idol-of-ferocity": 22397, "idol-of-brutality": 23198,
                                          "idol-of-the-moon": 23197, "libram-of-hope": 22401,
                                          "libram-of-invocation": 249442, "sentinels-libram": 272434,
                                          "libram-of-law": 272435, "libram-of-economy": 272436,
                                          "libram-of-infusion": 279248, "totem-of-the-storm": 23199,
                                          "totem-of-thunder": 228176}, EVERY_CLASS, 1, False),
]

# Items that hit several targets, against several: the on-use damage items and the area enchant.
MANY = [("production-frost-2-targets", MAGE_RACES), ("production-warrior-2-targets", WARRIOR_RACES),
        ("production-combat-rogue-3-targets", ROGUE_RACES), ("production-survival-hunter-5-targets", HUNTER_RACES)]
SEVERAL_TARGET_ROWS = [
    ("damage-on-use-several-targets", {"smokeys-lighter": 13171, "shard-of-the-fallen-star": 21891,
                                       "everlook-pathcarver": 274759, "helm-of-fire": 8348}, MANY, 2, False),
    # The wearer throws a Goblin Sapper Charge, whose hit on the thrower the proc hears.
    ("painwalker-with-a-sapper", {"painwalker-buckler": 274290}, MELEE[:2] + MELEE[5:], 3, True),
]

# Enchants Go registers in code, put on the first item of the type they name: name, enchant effect,
# requests, variants of each.
ENCHANT_ROWS = [
    ("fiery-blaze", 36, MANY[:3], 3),
]

# The accepted fixtures of these items, as they are.
FIXTURE_ROWS = [
    "warrior-bonereavers-edge", "protection-paladin-bonereavers-edge", "survival-hunter-bonereavers-edge",
    "combat-rogue-3-targets-bonereavers-edge", "warrior-thunderfury", "protection-warrior-3-targets-thunderfury",
    "protection-paladin-thunderfury", "protection-paladin-thunderfury-thunder-clap-debuff",
    "warrior-plaguefang-stinging-viper", "warrior-stinging-viper-plaguefang",
    "warrior-2-targets-plaguefang-stinging-viper", "combat-rogue-3-targets-plaguefang-stinging-viper",
    "protection-warrior-3-targets-plaguefang-stinging-viper",
]
FIXTURE_COUNT = 8
FIXTURE_RACES = {"ClassWarrior": WARRIOR_RACES, "ClassRogue": ROGUE_RACES, "ClassPaladin": PALADIN_RACES,
                 "ClassHunter": HUNTER_RACES}

SEED = 20261009
ITERATIONS = 200
MELEE_DISTANCE = 5

CLASS_BASES = {
    "warrior": "production-warrior", "mage": "production-frost-2-targets", "rogue": "production-combat-rogue",
    "hunter": "production-marksmanship-hunter", "priest": "production-shadow-priest",
    "paladin": "production-protection-paladin", "druid": "production-feral-druid",
    "shaman": "enhancement-shaman-dual-wield-windfury-flametongue", "warlock": "production-destruction-warlock",
}


def read_lines(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def items_database():
    return {record["id"]: record for record in read_lines(DATA / "items.jsonl")}


def enchants_database():
    found = {}
    for record in read_lines(DATA / "enchants.jsonl"):
        found.setdefault(record["effect_id"], record)
    return found


def request_of(name):
    return json.loads((FIXTURES / f"{name}.request.json").read_text())


def entries(request):
    return request["raid"]["parties"][0]["players"][0]["equipment"]["items"]


def slot_of(request, item, database):
    """The index of the entry the item replaces, or None: an entry of the same type, a trinket
    taking a ring's place, a weapon the first weapon's (an off hand item the second's)."""
    record = database[item]
    kind = record.get("type")
    kind = "ItemTypeFinger" if kind == "ItemTypeTrinket" else kind
    held = entries(request)
    if kind == "ItemTypeWeapon":
        weapons = [i for i, entry in enumerate(held) if database.get(entry.get("id"), {}).get("type") == kind]
        if record.get("hand_type") == "HandTypeOffHand" and len(weapons) > 1:
            return weapons[1]
        return weapons[0] if weapons else None
    same = [i for i, entry in enumerate(held) if database.get(entry.get("id"), {}).get("type") == kind]
    return same[0] if same else None


def equipped(base, item, database):
    """A copy of the request with the item in the place its type names: it replaces the entry of that
    type the request holds, since an appended entry would sit beside it and equip nothing there."""
    request = copy.deepcopy(base)
    held = entries(request)
    index = slot_of(request, item, database)
    if index is None:
        # The request leaves the slot empty (a ranged slot, say), so an entry fills it.
        held.append({"id": item})
    else:
        held[index] = {"id": item}
    return request


def enchanted(base, effect, database, enchants):
    """A copy of the request with the enchant on the first item of the type it names, or None."""
    request = copy.deepcopy(base)
    kind = enchants[effect]["type"]
    for entry in entries(request):
        if database.get(entry.get("id"), {}).get("type") == kind:
            entry["enchant"] = effect
            return request
    return None


def rows():
    """Every row: (name, base request, count, races, max distance), in a fixed order."""
    database = items_database()
    out = []
    for group, items, bases, count, melee in ITEM_ROWS:
        for base_name, races in bases:
            base = request_of(base_name)
            for name, item in items.items():
                out.append((f"{group}-{base_name}-{name}", equipped(base, item, database), count, races,
                            MELEE_DISTANCE if melee else None))
    for group, items, bases, count, melee in SEVERAL_TARGET_ROWS:
        for base_name, races in bases:
            base = request_of(base_name)
            for name, item in items.items():
                out.append((f"{group}-{base_name}-{name}", equipped(base, item, database), count, races,
                            MELEE_DISTANCE if melee else None))
    enchants = enchants_database()
    for name, effect, bases, count in ENCHANT_ROWS:
        for base_name, races in bases:
            out.append((f"enchant-{name}-{base_name}", enchanted(request_of(base_name), effect, database, enchants),
                        count, races, MELEE_DISTANCE))
    for name in FIXTURE_ROWS:
        base = request_of(name)
        player_class = base["raid"]["parties"][0]["players"][0]["class"]
        out.append((f"fixture-{name}", base, FIXTURE_COUNT, FIXTURE_RACES[player_class], MELEE_DISTANCE))
    return out


def variants():
    """(file stem, request) for every variant of every row."""
    out = []
    for index, (name, base, count, races, distance) in enumerate(rows()):
        drawn = generate(base, SEED + index, count, ITERATIONS, races, distance)
        for number, request in enumerate(drawn):
            out.append((f"{name}-sweep-{number:02d}", request))
    return out


def verdict(prepared, checked):
    """The outcome of one pair from the engine's answers: `prepared` is the exit status and
    standard output of `prepare`, `checked` those of `check`, or None when preparation refused.
    ("ran", "") when the gate admits it, else (code, reason) of the first refusal."""
    status, output = prepared
    if status == NOT_PREPARED:
        refusal = json.loads(output)["refusal"]
        return refusal["code"], refusal["reason"]
    if status != 0:
        return "error", output.strip()[:200]
    gate = json.loads(checked[1])
    if gate.get("supported"):
        return "ran", ""
    refusals = gate.get("refusals") or [{"code": "unsupported", "reason": ""}]
    return refusals[0]["code"], "; ".join(refusal["reason"] for refusal in refusals)[:300]


def survey_pair(engine, label, request, scratch):
    path = Path(scratch) / f"{label}.request.json"
    path.write_text(json.dumps(request))
    prepared_path = Path(scratch) / f"{label}.prepared.json"
    done = subprocess.run([str(engine), "prepare", "--request", str(path), "--outfile", str(prepared_path)],
                          capture_output=True, text=True)
    checked = None
    if done.returncode == 0:
        gate = subprocess.run([str(engine), "check", "--infile", str(prepared_path)], capture_output=True, text=True)
        checked = (gate.returncode, gate.stdout)
        prepared_path.unlink()
    path.unlink()
    return verdict((done.returncode, done.stdout or done.stderr), checked)


def survey(engine, jobs):
    """One row for every item and enchant on each class: (class, kind, id, name, code, reason)."""
    tables = json.loads((DATA / "go-tables.json").read_text())
    database, enchants = items_database(), enchants_database()
    pairs = []
    for player_class, base_name in CLASS_BASES.items():
        base = request_of(base_name)
        base["simOptions"]["iterations"] = 20
        base["simOptions"]["debugFirstIteration"] = False
        for item in tables["item_effect_ids"]:
            pairs.append((player_class, "item", item, database[item]["name"], equipped(base, item, database)))
        for effect in tables["enchant_effect_ids"]:
            request = enchanted(base, effect, database, enchants)
            if request:
                pairs.append((player_class, "enchant", effect, enchants[effect]["name"], request))
    with tempfile.TemporaryDirectory() as scratch:
        def one(pair):
            player_class, kind, ident, name, request = pair
            code, reason = survey_pair(engine, f"{player_class}-{kind}-{ident}", request, scratch)
            return player_class, kind, ident, name, code, reason
        with ThreadPoolExecutor(jobs) as pool:
            return list(pool.map(one, pairs))


def survey_record(results, revision):
    """The record of a survey: the counts, and each item or enchant with the codes and reasons it is
    refused on, by class."""
    outcomes = Counter("ran" if code == "ran" else "refused" for *_, code, _ in results)
    refused = {}
    for player_class, kind, ident, name, code, reason in results:
        if code == "ran":
            continue
        entry = refused.setdefault((kind, ident), {"kind": kind, "id": ident, "name": name, "refusals": {}})
        entry["refusals"].setdefault(f"{code}: {reason}", []).append(player_class)
    return {
        "kind": "item_refusal_survey",
        "date": date.today().isoformat(),
        "scope": "Every item and enchant effect Go registers in code (item_effect_ids and enchant_effect_ids of "
                 "data/go-tables.json), each worn in its real slot of one production request for each of nine "
                 "classes, prepared in Rust and run through the coverage gate: which pairs the gate admits and "
                 "which it refuses, with the code and reason.",
        "generator": "python3 tools/item_effect_sweep.py survey --output validation/RECORD.json",
        "source_revision": revision,
        "outcomes": dict(sorted(outcomes.items())),
        "pairs": len(results),
        "refused": [refused[key] for key in sorted(refused)],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["variants", "survey"])
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--engine", type=Path, default=ROOT / "target" / "release" / "forever-engine")
    parser.add_argument("--jobs", type=int, default=3)
    args = parser.parse_args()
    if args.command == "variants":
        args.output.mkdir(parents=True, exist_ok=False)
        written = variants()
        for stem, request in written:
            (args.output / f"{stem}.request.json").write_text(json.dumps(request, indent=2) + "\n")
        print(f"Wrote {len(written)} variants of {len(rows())} requests to {args.output}")
        return
    results = survey(args.engine, args.jobs)
    record = survey_record(results, PIN)
    args.output.write_text(json.dumps(record, indent=2) + "\n")
    print(f"{record['outcomes']} of {record['pairs']} pairs; {len(record['refused'])} items and enchants refused")


if __name__ == "__main__":
    main()
