#!/usr/bin/env python3
"""Write the variants of the Holy Nova and Demoralizing Shout sweep: every rank of Holy Nova cast by
a Shadow Priest without Shadowform and by a Smite Priest, and Demoralizing Shout cast by Protection,
Fury and Arms Warriors, each against 1, 3 and 5 targets, plus the accepted fixtures of both spells.

    python3 tools/live_spell_sweep.py --output <scratch>

writes `<scratch>/NAME-sweep-NN.request.json` for every row below, each the randomized variants
tools/sweep.py draws from an accepted request, with the Holy Nova rank the row names in the rotation
or the shout cast by the rotation. The same options always give the same variants. Compare them with
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

# Holy Nova's damage spell by rank (sim/priest/spell_data_auto_gen.go HolyNova).
HOLY_NOVA_RANKS = (15237, 15430, 15431, 27799, 27800, 27801)
# The rotation item of the top rank in each Priest base.
HOLY_NOVA_TOP = 27801

PRIEST_RACES = ("RaceHuman", "RaceDwarf", "RaceNightElf", "RaceUndead", "RaceTroll")
WARRIOR_RACES = ("RaceHuman", "RaceDwarf", "RaceOrc", "RaceUndead", "RaceTroll", "RaceTauren",
                 "RaceNightElf", "RaceGnome")

# name, accepted request, variants of each rank or request
NOVA_BASES = [
    ("shadow-priest-holy-nova", "shadow-priest-holy-nova"),
    ("shadow-priest-holy-nova-3-targets", "shadow-priest-holy-nova-3-targets"),
    ("shadow-priest-holy-nova-5-targets", "shadow-priest-holy-nova-5-targets"),
    ("smite-priest-holy-nova", "smite-priest-holy-nova"),
    ("smite-priest-holy-nova-3-targets", "smite-priest-holy-nova-3-targets"),
    ("smite-priest-holy-nova-5-targets", "smite-priest-holy-nova-5-targets"),
]
NOVA_VARIANTS = 4

# name, accepted request, variants
WARRIOR_BASES = [
    ("protection-warrior-demoralizing-shout", "protection-warrior-demoralizing-shout", 4),
    ("protection-warrior-demoralizing-shout-3-targets", "protection-warrior-demoralizing-shout-3-targets", 5),
    ("protection-warrior-demoralizing-shout-5-targets", "protection-warrior-demoralizing-shout-5-targets", 4),
    ("protection-warrior-demoralizing-shout-over-roar-debuff",
     "protection-warrior-demoralizing-shout-over-roar-debuff", 4),
    ("protection-warrior-demoralizing-shout-over-shout-debuff",
     "protection-warrior-demoralizing-shout-over-shout-debuff", 4),
    ("fury-warrior-demoralizing-shout", "fury-warrior-demoralizing-shout", 4),
    ("fury-warrior-demoralizing-shout-3-targets", "fury-warrior-demoralizing-shout-3-targets", 5),
    ("fury-warrior-demoralizing-shout-5-targets", "fury-warrior-demoralizing-shout-5-targets", 4),
]

# The accepted fixtures of both spells, as they are.
FIXTURE_ROWS = [
    "smite-priest-holy-nova", "smite-priest-holy-nova-3-targets", "smite-priest-holy-nova-5-targets",
    "shadow-priest-holy-nova", "shadow-priest-holy-nova-3-targets", "shadow-priest-holy-nova-5-targets",
    "shadow-priest-shadowform-holy-nova-power-infusion", "smite-priest-holy-nova-ephemeral-power",
    "protection-warrior-demoralizing-shout", "protection-warrior-demoralizing-shout-3-targets",
    "protection-warrior-demoralizing-shout-5-targets", "fury-warrior-demoralizing-shout",
    "fury-warrior-demoralizing-shout-3-targets", "fury-warrior-demoralizing-shout-5-targets",
    "protection-warrior-demoralizing-shout-over-roar-debuff",
    "protection-warrior-demoralizing-shout-over-shout-debuff",
]
FIXTURE_COUNT = 5

SEED = 20261009
ITERATIONS = 200
# A melee build stays in range; a priest draws any distance.
MELEE_DISTANCE = 5


def request_of(name):
    return json.loads((FIXTURES / f"{name}.request.json").read_text())


def with_rank(base, rank_spell):
    """A copy of the request whose rotation casts Holy Nova of the given rank in place of the top rank."""
    request = copy.deepcopy(base)
    rotation = request["raid"]["parties"][0]["players"][0]["rotation"]
    for item in rotation["priorityList"]:
        cast = item["action"].get("castSpell")
        if cast and cast["spellId"].get("spellId") == HOLY_NOVA_TOP:
            cast["spellId"]["spellId"] = rank_spell
            if "rank" in cast["spellId"]:
                cast["spellId"]["rank"] = HOLY_NOVA_RANKS.index(rank_spell) + 1
    return request


def keep_holy_nova(request):
    """The request with the Holy Nova talent (Holy tree, sixth node) learned again if the draw
    dropped it, since the rotation casts the spell and Go refuses a spell the priest lacks."""
    player = request["raid"]["parties"][0]["players"][0]
    trees = player["talentsString"].split("-")
    holy = list(trees[1])
    holy[5] = "1"
    trees[1] = "".join(holy)
    player["talentsString"] = "-".join(trees)
    return request


def rows():
    """Every row: (name, base request, count, races, max distance, fix), in a fixed order."""
    out = []
    for label, base_name in NOVA_BASES:
        base = request_of(base_name)
        for rank in HOLY_NOVA_RANKS:
            out.append((f"{label}-rank-{HOLY_NOVA_RANKS.index(rank) + 1}", with_rank(base, rank), NOVA_VARIANTS,
                        PRIEST_RACES, None, keep_holy_nova))
    for label, base_name, count in WARRIOR_BASES:
        out.append((label, request_of(base_name), count, WARRIOR_RACES, MELEE_DISTANCE, None))
    for name in FIXTURE_ROWS:
        base = request_of(name)
        priest = base["raid"]["parties"][0]["players"][0]["class"] == "ClassPriest"
        out.append((f"fixture-{name}", base, FIXTURE_COUNT, PRIEST_RACES if priest else WARRIOR_RACES,
                    None if priest else MELEE_DISTANCE, keep_holy_nova if priest else None))
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
