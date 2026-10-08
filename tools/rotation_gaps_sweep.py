#!/usr/bin/env python3
"""Write the variants of the live rotation gaps sweep: the rotation values and actions that
refused real production requests, applied to the accepted production requests of the classes
that use them.

    python3 tools/rotation_gaps_sweep.py --output <scratch>

writes `<scratch>/NAME-sweep-NN.request.json` for every row below, each the randomized variants
tools/sweep.py draws from an accepted request whose rotation one transform changed:

    reaction-time   every aura activity read on the player gains includeReactionTime
    dot-base        every cast of a dot the rotation reads also waits for the fight to outlast the
                    dot's base duration (dotBaseDuration)
    all-targets     every other dot read names AllTargets, and every other aura read AllTargets
                    or AllPlayers: references Go resolves to no unit
    weave           a Hunter that moves to 5 yards for its melee abilities and back to 12 for its
                    shots, as the production weavers do, after a prepull run
    timeline        a Hunter that runs to the ranges of a timeline, among them the dead zone
                    inside 8 yards and out of range, so the ranged auto stops and starts

The same options always give the same variants. Compare them with `tools/prepared_v2.py
compare`. Uses only Python's standard library.
"""

import argparse
import copy
import json
from pathlib import Path

from sweep import generate

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures" / "mage" / "prepared-v2"

SEED = 20261008
ITERATIONS = 200

HUMAN_DWARF = ("RaceHuman", "RaceDwarf")
RACES = {
    "ClassMage": ("RaceHuman", "RaceGnome", "RaceUndead", "RaceTroll"),
    "ClassWarlock": ("RaceHuman", "RaceGnome", "RaceOrc", "RaceUndead"),
    "ClassPriest": ("RaceHuman", "RaceDwarf", "RaceNightElf", "RaceUndead", "RaceTroll"),
    "ClassHunter": ("RaceDwarf", "RaceNightElf", "RaceOrc", "RaceTauren", "RaceTroll", "RaceHuman"),
    "ClassDruid": ("RaceNightElf", "RaceTauren"),
    "ClassShaman": ("RaceOrc", "RaceTauren", "RaceTroll"),
    "ClassWarrior": ("RaceHuman", "RaceDwarf", "RaceOrc", "RaceUndead", "RaceTroll", "RaceTauren"),
    "ClassRogue": ("RaceHuman", "RaceDwarf", "RaceNightElf", "RaceOrc", "RaceUndead", "RaceTroll"),
    "ClassPaladin": HUMAN_DWARF,
}

# Requests whose rotation reads auras of the player, by the class that casts them.
REACTION_REQUESTS = (
    "arcane-mage-aura-is-inactive-reaction-time", "arcane-mage-aura-reaction-time-400ms",
    "fire-mage-aura-inactive", "production-fire", "production-frost-2-targets", "production-frostfire",
    "production-balance-druid", "production-elemental-shaman", "production-enhancement-shaman",
    "production-feral-druid", "production-warrior", "production-arms-warrior", "production-combat-rogue",
    "production-retribution-paladin", "production-protection-paladin", "production-shadow-priest",
    "production-smite-priest", "production-affliction-warlock", "production-destruction-warlock",
    "production-marksmanship-hunter", "production-survival-hunter",
)

# Requests that cast a dot the rotation reads.
DOT_REQUESTS = (
    "production-affliction-warlock", "production-destruction-warlock", "production-demonology-warlock",
    "affliction-warlock-broad", "affliction-warlock-dot-base-duration",
    "affliction-warlock-3-targets-multidot-dots", "production-shadow-priest",
    "production-shadow-priest-2-targets", "production-shadow-priest-5-targets",
    "production-smite-priest", "production-smite-priest-3-targets", "production-marksmanship-hunter",
    "production-beast-mastery-hunter", "production-survival-hunter",
    "marksmanship-hunter-predators-armor-dot-base-duration", "production-balance-druid",
    "production-feral-druid", "production-fire",
)

# Requests that read dots or auras of the target, at one target and at several.
ALL_TARGETS_REQUESTS = (
    "production-affliction-warlock", "affliction-warlock-3-targets-multidot-dots",
    "affliction-warlock-3-targets-aura-source-target", "production-shadow-priest",
    "production-shadow-priest-2-targets", "production-shadow-priest-5-targets",
    "production-smite-priest-3-targets", "shadow-priest-3-targets-multidot-reads",
    "shadow-priest-3-targets-all-targets-dot", "production-marksmanship-hunter",
    "production-balance-druid", "fire-mage-3-targets-dot-target-index", "production-destruction-warlock",
)

# Hunters, at one target and at several.
HUNTER_REQUESTS = (
    "production-beast-mastery-hunter", "production-beast-mastery-hunter-3-targets",
    "production-marksmanship-hunter", "production-marksmanship-hunter-3-targets",
    "production-survival-hunter", "production-survival-hunter-2-targets", "production-survival-hunter-5-targets",
    "survival-melee-hunter-cat-heartbeat",
)

# The accepted fixtures of the gaps, as they are.
FIXTURE_ROWS = (
    "arcane-mage-aura-is-inactive-reaction-time", "arcane-mage-aura-reaction-time-400ms",
    "affliction-warlock-dot-base-duration", "marksmanship-hunter-predators-armor-dot-base-duration",
    "shadow-priest-3-targets-all-targets-dot", "survival-hunter-weaving-moves", "marksmanship-hunter-moves",
)
FIXTURE_COUNT = 8

REACTION_COUNT = 3
DOT_COUNT = 5
ALL_TARGETS_COUNT = 5
HUNTER_COUNT = 6


def request_of(name):
    return json.loads((FIXTURES / f"{name}.request.json").read_text())


def player_of(request):
    return request["raid"]["parties"][0]["players"][0]


def walk(node, visit):
    """Call `visit` on every dict and list of a JSON document, parents first."""
    visit(node)
    if isinstance(node, dict):
        for child in node.values():
            walk(child, visit)
    elif isinstance(node, list):
        for child in node:
            walk(child, visit)


def const(value):
    return {"const": {"val": value}}


def compare(lhs, op, rhs):
    return {"cmp": {"lhs": lhs, "op": op, "rhs": rhs}}


def conjoin(condition, extra):
    """The condition and the extra term, flattening a condition that is already a conjunction."""
    if not condition:
        return extra
    if "and" in condition:
        return {"and": {"vals": condition["and"]["vals"] + [extra]}}
    return {"and": {"vals": [condition, extra]}}


def reaction_time(request):
    """Every aura activity read on the player includes the reaction time. The number of reads
    changed, so a request without one can be left out."""
    request = copy.deepcopy(request)
    changed = []

    def visit(node):
        if not isinstance(node, dict):
            return
        for kind in ("auraIsActive", "auraIsInactive"):
            config = node.get(kind)
            if isinstance(config, dict) and config.get("sourceUnit", {"type": "Self"}).get("type") in ("Self",):
                config["includeReactionTime"] = True
                changed.append(kind)

    walk(player_of(request)["rotation"], visit)
    return request, len(changed)


def dot_spells(rotation):
    """The spell IDs of the dots the rotation reads."""
    spells = []

    def visit(node):
        if isinstance(node, dict):
            for kind in ("dotIsActive", "dotRemainingTime", "dotTimeToNextTick", "dotBaseDuration"):
                config = node.get(kind)
                if isinstance(config, dict) and config.get("spellId", {}).get("spellId"):
                    spells.append(config["spellId"]["spellId"])

    walk(rotation, visit)
    return sorted(set(spells))


def dot_base_duration(request):
    """Every cast of a dot the rotation reads also waits for the fight to outlast its base
    duration. Returns the request and the number of casts changed."""
    request = copy.deepcopy(request)
    rotation = player_of(request)["rotation"]
    spells = dot_spells(rotation)
    changed = 0
    for item in rotation.get("priorityList", []):
        action = item.get("action", {})
        cast = action.get("castSpell")
        if not cast or cast.get("spellId", {}).get("spellId") not in spells:
            continue
        spell = cast["spellId"]
        extra = compare({"remainingTime": {}}, "OpGt",
                        {"dotBaseDuration": {"spellId": {"spellId": spell["spellId"]}}})
        action["condition"] = conjoin(action.get("condition"), extra)
        changed += 1
    return request, changed


def all_targets(request):
    """Every other dot read names AllTargets and every other aura read AllTargets or
    AllPlayers, in the order the rotation holds them. Returns the request and the reads changed."""
    request = copy.deepcopy(request)
    count = [0, 0]

    def visit(node):
        if not isinstance(node, dict):
            return
        for kind in ("dotIsActive", "dotRemainingTime", "dotTimeToNextTick"):
            config = node.get(kind)
            if isinstance(config, dict):
                count[0] += 1
                if count[0] % 2 == 1:
                    config["targetUnit"] = {"type": "AllTargets"}
        for kind in ("auraIsActive", "auraIsInactive", "auraIsKnown", "auraNumStacks", "auraRemainingTime"):
            config = node.get(kind)
            if isinstance(config, dict) and config.get("sourceUnit", {}).get("type") != "Pet":
                count[1] += 1
                if count[1] % 2 == 1:
                    config["sourceUnit"] = {"type": "AllTargets" if count[1] % 4 == 1 else "AllPlayers"}

    walk(player_of(request)["rotation"], visit)
    return request, count[0] // 2 + count[0] % 2 + count[1] // 2 + count[1] % 2


def move(range_yards, condition=None):
    action = {"move": {"rangeFromTarget": const(str(range_yards))}}
    if condition:
        action["condition"] = condition
    return {"action": action}


def prepull_move(range_yards, at):
    return {"action": {"move": {"rangeFromTarget": const(str(range_yards))}}, "doAtValue": const(at)}


def time_window(start, end):
    return {"and": {"vals": [
        compare({"currentTime": {}}, "OpGe", const(f"{start}s")),
        compare({"currentTime": {}}, "OpLt", const(f"{end}s"))]}}


def weave(request):
    """The weaving Hunter's moves: in to 5 yards when its melee abilities are about to be ready
    and no shot is, back out to 12 otherwise, after a prepull run out to 20."""
    request = copy.deepcopy(request)
    rotation = player_of(request)["rotation"]
    ready = lambda spell: compare({"spellTimeToReady": {"spellId": {"spellId": spell}}}, "OpLe", const("1s"))
    moves = [
        move(5, {"and": {"vals": [
            ready(1317257), ready(14266),
            compare({"autoTimeToNext": {"autoType": "RangedAuto"}}, "OpGt", const("1.3s"))]}}),
        move(12, {"and": {"vals": [
            {"not": {"val": {"auraIsActive": {"auraId": {"spellId": 14266, "tag": 3}}}}},
            compare({"spellTimeToReady": {"spellId": {"spellId": 1317257}}}, "OpGt", const("0s"))]}}),
    ]
    rotation["priorityList"] = rotation.get("priorityList", [])[:1] + moves + rotation.get("priorityList", [])[1:]
    rotation["prepullActions"] = rotation.get("prepullActions", []) + [prepull_move(20, "-4s")]
    return request, len(moves)


def timeline(request):
    """The ranged Hunter's moves along a timeline: out to 30 yards, in to 10 and to 6, which is
    inside the dead zone of its bow, out to 20, and in to melee range."""
    request = copy.deepcopy(request)
    rotation = player_of(request)["rotation"]
    legs = [(10, 30, 18), (18, 40, 10), (40, 55, 6), (55, 70, 20), (70, 85, 3), (85, 100, 25)]
    moves = [move(distance, time_window(start, end)) for start, end, distance in legs]
    items = rotation.get("priorityList", [])
    rotation["priorityList"] = items[:1] + moves + items[1:]
    rotation["prepullActions"] = rotation.get("prepullActions", []) + [prepull_move(25, "-6s")]
    return request, len(moves)


def rows():
    """Every row: (name, base request, count, races, kind), in a fixed order."""
    out = []
    for name in REACTION_REQUESTS:
        base, changed = reaction_time(request_of(name))
        if changed:
            out.append((f"reaction-time-{name}", base, REACTION_COUNT, RACES[player_of(base)["class"]]))
    for name in DOT_REQUESTS:
        base, changed = dot_base_duration(request_of(name))
        if changed:
            out.append((f"dot-base-{name}", base, DOT_COUNT, RACES[player_of(base)["class"]]))
    for name in ALL_TARGETS_REQUESTS:
        base, changed = all_targets(request_of(name))
        if changed:
            out.append((f"all-targets-{name}", base, ALL_TARGETS_COUNT, RACES[player_of(base)["class"]]))
    for name in HUNTER_REQUESTS:
        for label, transform in (("weave", weave), ("timeline", timeline)):
            base, _ = transform(request_of(name))
            out.append((f"{label}-{name}", base, HUNTER_COUNT, RACES["ClassHunter"]))
    for name in FIXTURE_ROWS:
        base = request_of(name)
        out.append((f"fixture-{name}", base, FIXTURE_COUNT, RACES[player_of(base)["class"]]))
    return out


# The weapon slots: main hand, off hand and ranged. A Hunter without its bow has a ranged weapon
# of zero swing speed, whose shots Go computes as NaN, so the variants keep the base's weapons.
WEAPON_SLOTS = (12, 13, 14)


def keep_weapons(base, request):
    items = player_of(request)["equipment"]["items"]
    kept = player_of(base)["equipment"]["items"]
    for slot in WEAPON_SLOTS:
        if slot < len(items) and slot < len(kept):
            items[slot] = copy.deepcopy(kept[slot])
    return request


def variants():
    """(file stem, request) for every variant of every row."""
    out = []
    for index, (name, base, count, races) in enumerate(rows()):
        for number, request in enumerate(generate(base, SEED + index, count, ITERATIONS, races)):
            out.append((f"{name}-sweep-{number:02d}", keep_weapons(base, request)))
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
