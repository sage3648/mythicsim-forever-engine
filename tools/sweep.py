#!/usr/bin/env python3
"""Generate randomized variants of one RaidSimRequest for a compatibility sweep.

Each variant changes the fight length and its variation, the target level, the
distance, some talent ranks, empties some gear slots, drops some raid buffs, debuffs
and consumables, and varies reaction time and the random stream mode. With --races it
also draws each variant's race from the given list, from a separate stream, so the
variants are otherwise those of the same seed without races. --max-distance draws the
distance only from those within it, so a melee build stays in range. The same seed and
options always give the same variants. Compare them with `tools/prepared_v2.py compare`.
Uses only Python's standard library.
"""

import argparse
import copy
import json
from pathlib import Path
import random

DURATIONS = (30, 60, 90, 120, 180, 240, 300, 360)
VARIATIONS = (0, 5, 15, 30)
DISTANCES = (0, 5, 10, 20, 30)
REACTIONS = (0, 50, 100, 250)
# Consumables that come as an item and its list; drop them together.
PAIRED = {"potId": "potions", "conjuredId": "conjuredItems"}


def resolve(document, pointer):
    for part in [p for p in pointer.split("/") if p]:
        document = document[part]
    return document


def lower_talents(talents, rng):
    """Reduce some ranks of a Go talents string, tree by tree; never raise one."""
    trees = []
    for tree in talents.split("-"):
        ranks = [int(rank) for rank in tree]
        for index, rank in enumerate(ranks):
            if rank and rng.random() < 0.3:
                ranks[index] = rng.randint(0, rank)
        trees.append("".join(str(rank) for rank in ranks))
    return "-".join(trees)


def drop_flags(flags, rng, chance):
    return {key: value for key, value in flags.items() if not (value is True and rng.random() < chance)}


def variant(base, rng, iterations, distances=DISTANCES):
    request = copy.deepcopy(base)
    encounter = request["encounter"]
    encounter["duration"] = rng.choice(DURATIONS)
    # The contract requires a variation shorter than the fight; redraw only when it is not,
    # so earlier sweeps, which never drew such a pair, keep their variants.
    encounter["durationVariation"] = rng.choice(VARIATIONS)
    while encounter["durationVariation"] >= encounter["duration"]:
        encounter["durationVariation"] = rng.choice(VARIATIONS)
    # A fight against several targets holds identical copies, so they share the level.
    level = rng.randint(60, 63)
    for target in encounter["targets"]:
        target["level"] = level
    raid = request["raid"]
    for key in ("buffs", "debuffs"):
        if key in raid:
            raid[key] = drop_flags(raid[key], rng, 0.25)
    player = raid["parties"][0]["players"][0]
    player["distanceFromTarget"] = rng.choice(distances)
    player["reactionTimeMs"] = rng.choice(REACTIONS)
    player["talentsString"] = lower_talents(player["talentsString"], rng)
    if "buffs" in player:
        player["buffs"] = drop_flags(player["buffs"], rng, 0.25)
    items = player["equipment"]["items"]
    for index in range(len(items)):
        if items[index] and rng.random() < 0.15:
            items[index] = {}
    consumables = player.get("consumables", {})
    for key in sorted(consumables):
        if key in consumables and key not in PAIRED.values() and rng.random() < 0.2:
            del consumables[key]
            consumables.pop(PAIRED.get(key, ""), None)
    options = request["simOptions"]
    options["iterations"] = iterations
    options["randomSeed"] = str(rng.randint(1, 10**9))
    options["debugFirstIteration"] = True
    options["useLabeledRands"] = rng.random() < 0.25
    if not options["useLabeledRands"]:
        del options["useLabeledRands"]
    return request


def generate(base, seed, count, iterations=300, races=(), max_distance=None):
    rng = random.Random(seed)
    race_rng = random.Random(f"races-{seed}")
    distances = tuple(d for d in DISTANCES if max_distance is None or d <= max_distance)
    if not distances:
        raise ValueError(f"no distance within {max_distance}")
    variants = []
    for _ in range(count):
        request = variant(base, rng, iterations, distances)
        if races:
            request["raid"]["parties"][0]["players"][0]["race"] = race_rng.choice(races)
        variants.append(request)
    return variants


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("base", type=Path, help="RaidSimRequest JSON, or a document holding one")
    parser.add_argument("--pointer", default="", help="JSON pointer to the request inside the document")
    parser.add_argument("--seed", type=int, required=True)
    parser.add_argument("--count", type=int, default=24)
    parser.add_argument("--iterations", type=int, default=300)
    parser.add_argument("--races", default="", help="comma-separated Go race names to draw from")
    parser.add_argument("--max-distance", type=float, default=None,
                        help="draw only distances within this many yards, such as 5 for melee")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    base = resolve(json.loads(args.base.read_text()), args.pointer)
    args.output.mkdir(parents=True, exist_ok=False)
    races = tuple(race for race in args.races.split(",") if race)
    variants = generate(base, args.seed, args.count, args.iterations, races, args.max_distance)
    for index, request in enumerate(variants):
        (args.output / f"sweep-{index:02d}.json").write_text(json.dumps(request, indent=2) + "\n")
    print(f"Wrote {args.count} variants to {args.output}")


if __name__ == "__main__":
    main()
