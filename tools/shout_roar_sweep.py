#!/usr/bin/env python3
"""Write the variants of the Challenging Shout and Demoralizing Roar sweep: Challenging Shout cast
by Protection, Arms and Fury Warriors against 1, 3 and 5 targets, and Demoralizing Roar in the
rotations of bear and cat Druids against 1, 3 and 5 targets with no raid debuff, the raid's
permanent Demoralizing Shout debuff and its permanent Demoralizing Roar debuff, plus a Protection
Warrior's Demoralizing Shout under the same debuffs when its aura asks to be refreshed, and the
accepted fixtures of both spells.

    python3 tools/shout_roar_sweep.py --output <scratch>

writes `<scratch>/NAME-sweep-NN.request.json` for every row below, each the randomized variants
tools/sweep.py draws from an accepted request, with the shout or the roar added to the rotation.
A row that names a raid debuff keeps it in every variant, since the draw drops raid debuffs
at random. The same options always give the same variants. Compare them with
`tools/prepared_v2.py compare`.

Uses only Python's standard library.
"""

import argparse
import copy
import json
from pathlib import Path

from sweep import generate

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures" / "mage" / "prepared-v2"

CHALLENGING_SHOUT = 1161
DEMORALIZING_ROAR = 9898
DEMORALIZING_SHOUT = 11556

WARRIOR_RACES = ("RaceHuman", "RaceDwarf", "RaceOrc", "RaceUndead", "RaceTroll", "RaceTauren",
                 "RaceNightElf", "RaceGnome")
DRUID_RACES = ("RaceNightElf", "RaceTauren")
# A melee build stays in range.
MELEE_DISTANCE = 5

# The raid debuffs that share the Demoralizing category with the roar and the warrior's shout.
DEBUFFS = (("", None), ("-over-shout-debuff", "demoralizingShout"), ("-over-roar-debuff", "demoralizingRoar"))

# label, accepted request, targets, variants. The target count copies the request's first target.
CHALLENGING_BASES = [
    ("protection-warrior-challenging-shout", "production-protection-warrior", 1, 4),
    ("protection-warrior-challenging-shout-3-targets", "production-protection-warrior-3-targets", 3, 4),
    ("protection-warrior-challenging-shout-5-targets", "production-protection-warrior-5-targets", 5, 4),
    ("arms-warrior-challenging-shout", "production-arms-warrior", 1, 4),
    ("arms-warrior-challenging-shout-3-targets", "production-arms-warrior-3-targets", 3, 4),
    ("arms-warrior-challenging-shout-5-targets", "production-arms-warrior-3-targets", 5, 4),
    ("fury-warrior-challenging-shout", "production-warrior", 1, 4),
    ("fury-warrior-challenging-shout-2-targets", "production-warrior-2-targets", 2, 3),
    ("fury-warrior-challenging-shout-5-targets", "production-warrior-5-targets", 5, 4),
]

# The accepted fixtures of the shout, as they are.
SHOUT_FIXTURES = [
    "protection-warrior-challenging-shout", "protection-warrior-challenging-shout-3-targets",
    "arms-warrior-challenging-shout", "arms-warrior-challenging-shout-3-targets",
]
SHOUT_FIXTURE_COUNT = 5

# label, the bear's accepted request by target count
BEAR_BASES = {1: "production-feral-bear-druid", 3: "production-feral-bear-druid-3-targets",
              5: "production-feral-bear-druid-5-targets"}
CAT_BASE = "production-feral-druid"
BEAR_VARIANTS = 4
BEAR_UNTANKED_VARIANTS = 3
CAT_VARIANTS = 3
SHOUT_REFRESH_VARIANTS = 3

# The accepted fixtures of the roar, as they are.
ROAR_FIXTURES = [
    f"feral-{prefix}druid-demoralizing-roar{suffix}{tag}"
    for kind, prefix in (("bear", "bear-"), ("cat", ""))
    for suffix in ("", "-3-targets", "-5-targets")
    for tag, debuff in DEBUFFS
    if not (kind == "bear" and debuff is None)
]
ROAR_FIXTURE_COUNT = 3

SEED = 20261010
ITERATIONS = 200


def request_of(name):
    return json.loads((FIXTURES / f"{name}.request.json").read_text())


def with_targets(request, count):
    """A copy of the request with the given number of copies of its first target."""
    request = copy.deepcopy(request)
    targets = request["encounter"]["targets"]
    request["encounter"]["targets"] = [copy.deepcopy(targets[0]) for _ in range(count)]
    return request


def rotation_of(request):
    return request["raid"]["parties"][0]["players"][0]["rotation"]["priorityList"]


def with_challenging_shout(request):
    """The request whose rotation casts Challenging Shout first."""
    request = copy.deepcopy(request)
    rotation_of(request).insert(0, {"action": {"castSpell": {"spellId": {"spellId": CHALLENGING_SHOUT}}}})
    return request


def with_roar(request):
    """The request whose rotation casts Demoralizing Roar first, when the target's debuff is due."""
    request = copy.deepcopy(request)
    rotation_of(request).insert(0, {"action": {
        "castSpell": {"spellId": {"spellId": DEMORALIZING_ROAR}},
        "condition": {"auraShouldRefresh": {
            "auraId": {"spellId": DEMORALIZING_ROAR},
            "maxOverlap": {"const": {"val": "2s"}},
            "sourceUnit": {"type": "CurrentTarget"},
        }},
    }})
    return request


def with_debuff(request, debuff):
    request = copy.deepcopy(request)
    if debuff:
        request["raid"]["debuffs"][debuff] = True
    return request


def with_shout_refresh(request):
    """The request whose Demoralizing Shout waits for its aura to want a refresh, in place of
    the 1.5 sec left the accepted requests wait for."""
    request = copy.deepcopy(request)
    for item in rotation_of(request):
        cast = item["action"].get("castSpell")
        if cast and cast["spellId"].get("spellId") == DEMORALIZING_SHOUT:
            item["action"]["condition"] = {"auraShouldRefresh": {
                "auraId": {"spellId": DEMORALIZING_SHOUT},
                "maxOverlap": {"const": {"val": "2s"}},
                "sourceUnit": {"type": "CurrentTarget"},
            }}
    return request


def untanked(request):
    """The request with nothing tanking the target, so no copy of it swings."""
    request = copy.deepcopy(request)
    request["raid"].pop("tanks", None)
    return request


def keep_debuff(debuff):
    """A fix that puts the raid debuff back where the draw dropped it."""
    def fix(request):
        request["raid"]["debuffs"][debuff] = True
        return request
    return fix


def rows():
    """Every row: (name, base request, count, races, max distance, fix), in a fixed order."""
    out = []
    for label, base_name, targets, count in CHALLENGING_BASES:
        base = with_challenging_shout(with_targets(request_of(base_name), targets))
        out.append((label, base, count, WARRIOR_RACES, MELEE_DISTANCE, None))
    for count, base_name in BEAR_BASES.items():
        suffix = "" if count == 1 else f"-{count}-targets"
        for tag, debuff in DEBUFFS:
            base = with_debuff(request_of(base_name), debuff)
            fix = keep_debuff(debuff) if debuff else None
            out.append((f"bear-tank-roar{suffix}{tag}", base, BEAR_VARIANTS, DRUID_RACES, MELEE_DISTANCE, fix))
            out.append((f"bear-no-tank-roar{suffix}{tag}", untanked(base), BEAR_UNTANKED_VARIANTS, DRUID_RACES,
                        MELEE_DISTANCE, fix))
            cat = with_roar(with_debuff(with_targets(request_of(CAT_BASE), count), debuff))
            out.append((f"cat-roar{suffix}{tag}", cat, CAT_VARIANTS, DRUID_RACES, MELEE_DISTANCE, fix))
    # The warrior's own shout by auraShouldRefresh, which reads the same category as the roar.
    for count, base_name in ((1, "protection-warrior-demoralizing-shout"),
                             (3, "protection-warrior-demoralizing-shout-3-targets")):
        suffix = "" if count == 1 else f"-{count}-targets"
        for tag, debuff in DEBUFFS:
            base = with_shout_refresh(with_debuff(request_of(base_name), debuff))
            out.append((f"protection-warrior-shout-refresh{suffix}{tag}", base, SHOUT_REFRESH_VARIANTS,
                        WARRIOR_RACES, MELEE_DISTANCE, keep_debuff(debuff) if debuff else None))
    for name in SHOUT_FIXTURES:
        out.append((f"fixture-{name}", request_of(name), SHOUT_FIXTURE_COUNT, WARRIOR_RACES, MELEE_DISTANCE, None))
    for name in ROAR_FIXTURES:
        debuff = ("demoralizingShout" if "over-shout" in name else
                  "demoralizingRoar" if "over-roar" in name else None)
        out.append((f"fixture-{name}", request_of(name), ROAR_FIXTURE_COUNT, DRUID_RACES, MELEE_DISTANCE,
                    keep_debuff(debuff) if debuff else None))
    return out


def variants():
    """(file stem, request) for every variant of every row."""
    out = []
    for index, (name, base, count, races, distance, fix) in enumerate(rows()):
        drawn = generate(base, SEED + index, count, ITERATIONS, races, distance)
        for number, request in enumerate(drawn):
            out.append((f"{name}-sweep-{number:02d}", fix(request) if fix else request))
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
