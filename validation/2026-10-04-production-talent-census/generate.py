"""Talent census variants of the production requests: each variant changes one talent. A taken
talent is removed; any other talent the request could reach is raised to its maximum rank. The
talent builder's rules decide what is legal (ui/features/talents/model/can_set_points.ts at the
pinned revision): 51 points at level 60, five points in a tree for each row above, and a maximum
rank prerequisite. A removal also clears the talents that need the removed one, and when it
would strand deeper points it spends the freed points on the shallowest talents of the tree the
builder lets take them; the variant records both. A raise past the 51 point cap frees the points it needs one at a
time from other talents the builder lets go, other trees first, then the deepest rows; the
variant records them. A raise whose row or prerequisite the request has not reached is skipped.
The same inputs always give the same variants. Uses only Python's standard library and the
pinned engine's talent trees (ui/sim/talents/trees).

python3 validation/2026-10-04-production-talent-census/generate.py OUT_DIR ITERATIONS
"""
from collections import Counter
import glob
import json
import sys
from pathlib import Path

HERE = Path(__file__).parent
REQUESTS = HERE.parent / "2026-10-04-production-gear-swaps" / "requests"
TREES = Path("oracle-cache") / ("sour" + "ce") / "ui" / "sim" / "talents" / "trees"
MAX_POINTS = 51
POINTS_PER_ROW = 5


def player(request):
    return request["raid"]["parties"][0]["players"][0]


def parse(config, text):
    """talents_string.ts parseTalentsString."""
    trees = text.split("-")
    return [[min(int(trees[t][i]) if t < len(trees) and i < len(trees[t]) else 0, talent["maxPoints"])
             for i, talent in enumerate(tree["talents"])] for t, tree in enumerate(config)]


def serialize(points):
    """talents_string.ts serializeTalentsString."""
    return "-".join("".join(map(str, tree)).rstrip("0") for tree in points).rstrip("-")


def graph(tree):
    """tree_graph.ts buildTreeGraph: each talent's prerequisite and children."""
    where = {(t["location"]["rowIdx"], t["location"]["colIdx"]): i for i, t in enumerate(tree["talents"])}
    prereq = []
    for talent in tree["talents"]:
        location = talent.get("prereqLocation")
        prereq.append(where.get((location["rowIdx"], location["colIdx"]), -1) if location else -1)
    children = [[c for c, p in enumerate(prereq) if p == i] for i in range(len(prereq))]
    return prereq, children


def stranded(tree, points, rows):
    totals = [0] * rows
    for talent, value in zip(tree["talents"], points):
        totals[talent["location"]["rowIdx"]] += value
    cumulative, running = [], 0
    for total in totals:
        running += total
        cumulative.append(running)
    return sum(1 for talent, value in zip(tree["talents"], points)
               if value > 0 and talent["location"]["rowIdx"] > 0
               and cumulative[talent["location"]["rowIdx"] - 1] < talent["location"]["rowIdx"] * POINTS_PER_ROW)


def can_set(config, points, t, i, new):
    """can_set_points.ts canSetPoints."""
    tree = config[t]
    talent = tree["talents"][i]
    prereq, children = graph(tree)
    old = points[t][i]
    rows = max(x["location"]["rowIdx"] for tr in config for x in tr["talents"]) + 1
    if new > old:
        if sum(map(sum, points)) + new - old > MAX_POINTS:
            return False
        if sum(points[t]) < talent["location"]["rowIdx"] * POINTS_PER_ROW:
            return False
        p = prereq[i]
        return not (p >= 0 and points[t][p] < tree["talents"][p]["maxPoints"])
    after = [new if k == i else v for k, v in enumerate(points[t])]
    if stranded(tree, after, rows) > stranded(tree, points[t], rows):
        return False
    return not any(points[t][c] > 0 for c in children[i])


def raise_talent(config, points, t, i):
    """The talent at its maximum rank, freeing points past the cap from other talents. Returns
    the new points and the donors, or the reason the builder cannot do it."""
    tree = config[t]
    talent = tree["talents"][i]
    prereq, _ = graph(tree)
    if sum(points[t]) < talent["location"]["rowIdx"] * POINTS_PER_ROW:
        return None, "row not reached"
    if prereq[i] >= 0 and points[t][prereq[i]] < tree["talents"][prereq[i]]["maxPoints"]:
        return None, "prerequisite not at maximum rank"
    work = [list(tree_points) for tree_points in points]
    need = sum(map(sum, work)) + talent["maxPoints"] - work[t][i] - MAX_POINTS
    chain = set()
    p = prereq[i]
    while p >= 0:
        chain.add(p)
        p = prereq[p]
    order = sorted(((dt, di) for dt, dtree in enumerate(config) for di in range(len(dtree["talents"]))
                    if not (dt == t and (di == i or di in chain))),
                   key=lambda k: (k[0] == t, -config[k[0]]["talents"][k[1]]["location"]["rowIdx"], -k[0], -k[1]))
    donors = Counter()
    while need > 0:
        for dt, di in order:
            if work[dt][di] > 0 and can_set(config, work, dt, di, work[dt][di] - 1):
                work[dt][di] -= 1
                donors[(dt, di)] += 1
                need -= 1
                break
        else:
            return None, "no points the builder can free"
    if not can_set(config, work, t, i, talent["maxPoints"]):
        return None, "freeing points loses the row"
    work[t][i] = talent["maxPoints"]
    assert sum(map(sum, work)) <= MAX_POINTS
    return (work, [{"talent": config[dt]["talents"][di]["fancyName"], "points": n}
                   for (dt, di), n in sorted(donors.items())]), None


def remove_talent(config, points, t, i):
    """The talent and the talents that need it as a prerequisite at zero, then, while that
    strands deeper points, the freed points spent one at a time on the shallowest talents of the
    tree the builder lets take one. Returns the new points, the dependents and the refill, or
    the reason the builder cannot do it."""
    tree = config[t]
    prereq, children = graph(tree)
    rows = max(x["location"]["rowIdx"] for tr in config for x in tr["talents"]) + 1
    work = [list(tree_points) for tree_points in points]
    removed, stack, dependents = set(), [i], []
    while stack:
        k = stack.pop()
        if k != i and work[t][k] > 0:
            dependents.append({"talent": tree["talents"][k]["fancyName"], "points": work[t][k]})
        work[t][k] = 0
        removed.add(k)
        stack.extend(children[k])
    order = sorted((k for k in range(len(tree["talents"])) if k not in removed),
                   key=lambda k: (tree["talents"][k]["location"]["rowIdx"], k))
    refill = Counter()
    while stranded(tree, work[t], rows) > stranded(tree, points[t], rows):
        for k in order:
            if work[t][k] < tree["talents"][k]["maxPoints"] and can_set(config, work, t, k, work[t][k] + 1):
                work[t][k] += 1
                refill[k] += 1
                break
        else:
            return None, "no refill keeps the deeper rows"
    return (work, dependents, [{"talent": tree["talents"][k]["fancyName"], "points": n}
                               for k, n in sorted(refill.items())]), None


OUT = Path(sys.argv[1])
ITERATIONS = int(sys.argv[2])
OUT.mkdir(parents=True, exist_ok=True)
index = []
skipped = Counter()


def write(name, request, base, talents, change):
    variant = json.loads(json.dumps(request))
    player(variant)["talentsString"] = talents
    variant["simOptions"]["iterations"] = ITERATIONS
    json.dump(variant, open(OUT / f"{name}.request.json", "w"))
    index.append({"scenario": name, "base": base, "talents": talents, "change": change})


for path in sorted(glob.glob(str(REQUESTS / "*.request.json"))):
    base = Path(path).name.replace(".request.json", "")
    request = json.load(open(path))
    cls = player(request)["class"].replace("Class", "").lower()
    config = json.load(open(TREES / f"{cls}.json"))
    points = parse(config, player(request).get("talentsString", ""))
    # A production tree may already strand a point, as the Destruction Warlock's does; the
    # builder's rules only refuse changes that strand more.
    for t, tree in enumerate(config):
        for i, talent in enumerate(tree["talents"]):
            field = talent["fieldName"]
            if points[t][i] > 0:
                result, reason = remove_talent(config, points, t, i)
                if result is None:
                    skipped[reason] += 1
                else:
                    work, dependents, refill = result
                    write(f"{base}--remove-{field}", request, base, serialize(work),
                          {"kind": "remove", "talent": talent["fancyName"], "points": points[t][i],
                           "dependents": dependents, "refill": refill})
            if points[t][i] < talent["maxPoints"]:
                result, reason = raise_talent(config, points, t, i)
                if result is None:
                    skipped[reason] += 1
                else:
                    work, donors = result
                    write(f"{base}--raise-{field}", request, base, serialize(work),
                          {"kind": "raise", "talent": talent["fancyName"], "from": points[t][i],
                           "to": talent["maxPoints"], "donors": donors})
    # The base itself, to tell rejections a talent causes from the base's own.
    write(f"{base}--base", request, base, player(request).get("talentsString", ""), {"kind": "base"})

json.dump(index, open(OUT / "index.json", "w"), indent=1)
print(len(index), Counter(entry["change"]["kind"] for entry in index), dict(skipped))
