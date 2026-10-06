#!/usr/bin/env python3
"""Generate requests whose rotations name a unit, to compare with the pinned Go engine.

Each request is a production reference request, or a Mage census base, with items put into
its rotation and, for several targets, the one boss target repeated. The same bases and
options always give the same requests. Compare them with `tools/prepared_v2.py compare`.

forms   dot values with a target unit, a dot value without a spell, casts at a target, aura
        values with a source unit, `auraIsInactive`, ordered comparisons of booleans, and
        dot and aura reads on other targets after a multidot, at 1 and 3 targets, and at 2
        and 5 for some. Every item is written here, not by the application's builder.
casts   every `castSpell` of each production rotation aimed at another target, at 3 targets.

Uses only Python's standard library.
"""

import argparse
import copy
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "fixtures" / "mage" / "prepared-v2"
CENSUS = ROOT / "validation" / "2026-10-04-mage-census" / "bases"


def spell(spell_id, **extra):
    return {"spellId": spell_id, **extra}


def const(val):
    return {"const": {"val": val}}


def cmp(op, lhs, rhs):
    return {"cmp": {"op": op, "lhs": lhs, "rhs": rhs}}


def all_of(*vals):
    return {"and": {"vals": list(vals)}}


def any_of(*vals):
    return {"or": {"vals": list(vals)}}


def negate(val):
    return {"not": {"val": val}}


def unit(kind, index=None):
    return {"type": kind} if index is None else {"type": kind, "index": index}


CURRENT = unit("CurrentTarget")
TARGET = [unit("Target", 0), unit("Target", 1), unit("Target", 2)]
NEXT = unit("NextTarget")
PREVIOUS = unit("PreviousTarget")
TIME = {"currentTime": {}}


def dot_value(kind, spell_id, target=None):
    config = {"spellId": spell_id}
    if target is not None:
        config["targetUnit"] = target
    return {kind: config}


def dot_active(spell_id, target=None):
    return dot_value("dotIsActive", spell_id, target)


def dot_remaining(spell_id, target=None):
    return dot_value("dotRemainingTime", spell_id, target)


def dot_tick(spell_id, target=None):
    return dot_value("dotTimeToNextTick", spell_id, target)


def aura(kind, aura_id, source=None):
    config = {"auraId": aura_id}
    if source is not None:
        config["sourceUnit"] = source
    return {kind: config}


def cast(spell_id, cond=None, target=None):
    config = {"spellId": spell_id}
    if target is not None:
        config["target"] = target
    action = {"castSpell": config}
    if cond is not None:
        action["condition"] = cond
    return {"action": action}


def window(low, high):
    return all_of(cmp("OpGe", TIME, const(low)), cmp("OpLt", TIME, const(high)))


# Per build: its base request, three castable spells with a dot on the target, a filler, a
# spell that puts an aura on the target and that aura, and an aura of the player's own.
PROFILES = {
    "affliction-warlock": dict(
        base=FIXTURES / "production-affliction-warlock.request.json",
        dots=[spell(25311), spell(11713), spell(25309)], filler=spell(25307),
        debuff=spell(1311680), debuff_aura=spell(1311680), own=spell(17941)),
    "destruction-warlock": dict(
        base=FIXTURES / "production-destruction-warlock.request.json",
        dots=[spell(25311), spell(11713), spell(25309)], filler=spell(25307),
        debuff=spell(1311680), debuff_aura=spell(1311680), own=spell(17941)),
    "fire-mage": dict(
        base=FIXTURES / "production-fire.request.json",
        dots=[spell(25306), spell(18809), spell(10216)], filler=spell(25306),
        debuff=spell(25306), debuff_aura=spell(25306), own=spell(12536)),
    "frost-mage": dict(
        base=CENSUS / "frost-mage.request.json",
        dots=[spell(25306), spell(10216), spell(401502)], filler=spell(25304),
        debuff=spell(25306), debuff_aura=spell(25306), own=spell(12536)),
    "arcane-mage": dict(
        base=CENSUS / "arcane-mage.request.json",
        dots=[spell(25306), spell(10216), spell(25345)], filler=spell(25306),
        debuff=spell(25306), debuff_aura=spell(25306), own=spell(12536)),
    "warrior": dict(
        base=FIXTURES / "production-warrior.request.json",
        dots=[spell(11574)] * 3, filler=spell(11574),
        debuff=spell(11597), debuff_aura=spell(11597), own=spell(2458)),
    "shadow-priest": dict(
        base=FIXTURES / "production-shadow-priest.request.json",
        dots=[spell(10894, rank=8), spell(19280), spell(15407)], filler=spell(10947),
        debuff=spell(10894), debuff_aura=spell(10894), own=spell(15473)),
    "balance-druid": dict(
        base=FIXTURES / "production-balance-druid.request.json",
        dots=[spell(9835), spell(24977), spell(9835)], filler=spell(25298),
        debuff=spell(9835), debuff_aura=spell(9835), own=spell(24858)),
    "survival-hunter": dict(
        base=FIXTURES / "production-survival-hunter.request.json",
        dots=[spell(25295), spell(14305), spell(25295)], filler=spell(14268),
        debuff=spell(25295), debuff_aura=spell(25295), own=spell(10610)),
    "elemental-shaman": dict(
        base=FIXTURES / "production-elemental-shaman.request.json",
        dots=[spell(29228), spell(10438), spell(29228)], filler=spell(915),
        debuff=spell(29228), debuff_aura=spell(29228), own=spell(10432)),
}


def form_dot_unit(p):
    """`targetUnit` naming the current target, spelled three ways."""
    a, b, c = p["dots"]
    return [
        cast(a, negate(dot_active(a, CURRENT))),
        cast(b, cmp("OpLe", dot_remaining(b, CURRENT), const("3s"))),
        cast(c, all_of(negate(dot_active(c, unit("Target"))),
                       cmp("OpGe", dot_tick(c, TARGET[0]), const("0s")))),
    ], []


def form_dot_targets(p):
    """Dots read and cast on each target by index, the next and the previous, and a target
    the fight may lack."""
    a, b, c = p["dots"]
    front = [cast(a, all_of(negate(dot_active(a, TARGET[i])),
                            cmp("OpLe", dot_remaining(a, TARGET[i]), const("3s"))), target=TARGET[i])
             for i in range(3)]
    front.append(cast(b, negate(dot_active(b, NEXT)), target=NEXT))
    front.append(cast(c, cmp("OpLe", dot_remaining(c, PREVIOUS), const("2s")), target=PREVIOUS))
    front.append(cast(b, cmp("OpGt", dot_tick(b, TARGET[1]), const("1s")), target=unit("Target", 7)))
    return front, []


def form_cast_target(p):
    """The filler cast at each kind of target in turn."""
    f = p["filler"]
    return [
        cast(f, window("5s", "25s"), target=TARGET[1]),
        cast(f, window("25s", "45s"), target=TARGET[2]),
        cast(f, window("45s", "65s"), target=NEXT),
        cast(f, window("65s", "85s"), target=PREVIOUS),
        cast(f, window("85s", "105s"), target=unit("Target")),
        cast(f, window("105s", "125s"), target=CURRENT),
    ], []


def form_aura_target(p):
    """Auras read on other targets, with the debuff cast on the one it is read on."""
    d, au = p["debuff"], p["debuff_aura"]
    front = [cast(d, all_of(aura("auraIsKnown", au, TARGET[i]), aura("auraIsInactive", au, TARGET[i])),
                  target=TARGET[i]) for i in range(3)]
    front.append(cast(d, cmp("OpLe", aura("auraRemainingTime", au, NEXT), const("2s")), target=NEXT))
    front.append(cast(d, cmp("OpLt", aura("auraNumStacks", au, PREVIOUS), const("1")), target=PREVIOUS))
    front.append(cast(p["filler"], negate(aura("auraIsActive", au, TARGET[2]))))
    return front, []


def form_dot_no_spell(p):
    """Dot values without a spell: the term drops out, alone the condition is always true."""
    a, b, c = p["dots"]
    return [
        cast(a, all_of({"dotIsActive": {}}, negate(dot_active(a)))),
        cast(b, any_of({"dotIsActive": {"targetUnit": CURRENT}}, negate(dot_active(b)))),
        cast(c, all_of(negate({"dotRemainingTime": {}}), negate(dot_active(c, CURRENT)))),
    ], [
        cast(p["filler"], cmp("OpLe", {"dotRemainingTime": {}}, const("3s"))),
        cast(p["filler"], negate({"dotIsActive": {}})),
    ]


def form_ordered_boolean(p):
    """Ordered comparisons of booleans, which Go drops: in and, or, not, between two
    constants, and alone at the end of the list, where it is always true."""
    a, b, c = p["dots"]
    au, own = p["debuff_aura"], p["own"]
    return [
        cast(a, all_of(cmp("OpGt", dot_active(a), aura("auraIsActive", own)), negate(dot_active(a)))),
        cast(b, any_of(cmp("OpLt", dot_active(b), const("true")), negate(dot_active(b)))),
        cast(c, all_of(negate(cmp("OpGe", aura("auraIsActive", au, CURRENT), aura("auraIsActive", own))),
                       negate(dot_active(c)))),
        cast(a, all_of(cmp("OpLe", const("true"), const("false")), negate(dot_active(a, CURRENT)))),
    ], [cast(p["filler"], cmp("OpGt", dot_active(a), aura("auraIsActive", own)))]


def form_aura_inactive(p):
    """`auraIsInactive` on the player, the current target, and an aura nobody has."""
    d, au, own = p["debuff"], p["debuff_aura"], p["own"]
    return [
        cast(d, aura("auraIsInactive", au, CURRENT)),
        cast(p["dots"][0], all_of(aura("auraIsInactive", au), negate(dot_active(p["dots"][0])))),
        cast(p["dots"][1], all_of(aura("auraIsInactive", spell(999999999)), negate(dot_active(p["dots"][1])))),
    ], [cast(p["filler"], aura("auraIsInactive", own))]


def form_multidot_reads(p):
    """Dots and auras read on other targets while a multidot puts the dot on them."""
    a, au = p["dots"][0], p["debuff_aura"]
    f = p["filler"]
    return [
        {"action": {"multidot": {"spellId": a, "maxDots": 3, "maxOverlap": const("2s")}}},
        cast(f, all_of(dot_active(a, TARGET[1]), dot_active(a, TARGET[2]),
                       cmp("OpGt", dot_remaining(a, NEXT), dot_remaining(a, PREVIOUS)))),
        cast(f, cmp("OpLt", dot_tick(a, TARGET[2]), dot_tick(a, TARGET[1]))),
        cast(f, all_of(aura("auraIsActive", au, TARGET[1]), negate(aura("auraIsInactive", au, PREVIOUS)))),
        cast(f, cmp("OpGe", aura("auraRemainingTime", au, TARGET[2]), aura("auraRemainingTime", au, TARGET[1]))),
    ], []


FORMS = {
    "dot-target-unit": form_dot_unit,
    "dot-target-index": form_dot_targets,
    "cast-target": form_cast_target,
    "aura-source-target": form_aura_target,
    "dot-without-spell": form_dot_no_spell,
    "ordered-boolean": form_ordered_boolean,
    "aura-inactive": form_aura_inactive,
    "multidot-reads": form_multidot_reads,
}

# Builds that also run against 2 and 5 targets, and the forms that read or cast on them.
MORE_TARGETS = ("affliction-warlock", "fire-mage", "warrior", "shadow-priest")
MORE_FORMS = ("dot-target-index", "cast-target", "aura-source-target")


def with_targets(request, count):
    request = copy.deepcopy(request)
    first = request["encounter"]["targets"][0]
    request["encounter"]["targets"] = [copy.deepcopy(first) for _ in range(count)]
    return request


def priority_list(request):
    return request["raid"]["parties"][0]["players"][0]["rotation"]["priorityList"]


def form_plan():
    """(build, form, targets) for every request of the forms set."""
    plan = []
    for profile in PROFILES:
        for form in FORMS:
            counts = (3,) if form == "multidot-reads" else (1, 3)
            plan.extend((profile, form, count) for count in counts)
    plan.extend((profile, "multidot-reads", 5) for profile in
                ("affliction-warlock", "fire-mage", "warrior", "shadow-priest", "survival-hunter"))
    plan.extend((profile, form, count) for profile in MORE_TARGETS for form in MORE_FORMS
                for count in (2, 5))
    return plan


def forms():
    requests = {}
    for profile, form, count in form_plan():
        p = PROFILES[profile]
        request = with_targets(json.loads(p["base"].read_text()), count)
        front, back = FORMS[form](p)
        items = priority_list(request)
        items[0:0] = front
        items.extend(back)
        requests[f"{profile}-{form}-{count}-target{'s' if count > 1 else ''}"] = request
    return requests


CAST_TARGETS = {
    "second": unit("Target", 1),
    "next": NEXT,
    "previous": PREVIOUS,
}
# The builds that also get the next and previous target.
ALL_KINDS = ("fire", "affliction", "warrior", "shadow-priest", "survival")


def casts():
    requests = {}
    for path in sorted(FIXTURES.glob("production-*.request.json")):
        name = path.name.removesuffix(".request.json")
        if "-targets" in name:
            continue
        base = json.loads(path.read_text())
        for kind, target in CAST_TARGETS.items():
            if kind != "second" and not any(word in name for word in ALL_KINDS):
                continue
            request = with_targets(base, 3)
            for item in priority_list(request):
                if "castSpell" in item["action"]:
                    item["action"]["castSpell"]["target"] = target
            requests[f"{name}-casts-at-{kind}-3-targets"] = request
    return requests


SETS = {"forms": forms, "casts": casts}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("set", choices=sorted(SETS))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    requests = SETS[args.set]()
    for name, request in requests.items():
        (args.output / f"{name}.request.json").write_text(json.dumps(request, indent=2, sort_keys=True) + "\n")
    print(f"Wrote {len(requests)} requests to {args.output}")


if __name__ == "__main__":
    main()
