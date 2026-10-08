#!/usr/bin/env python3
"""Write the variants of the remaining rotation inputs sweep: the rotation constructs that Rust
refused after the live rotation gaps, applied to the accepted production requests of every class.

    python3 tools/rotation_remaining_sweep.py --output <scratch>

writes `<scratch>/NAME-sweep-NN.request.json` for every row below, each the randomized variants
tools/sweep.py draws from an accepted request whose rotation one transform changed:

    moves        a timeline of move and moveDuration actions in the priority list, among them the
                 ranges inside a ranged weapon's dead zone and out of melee range, after a prepull
                 move; the player's race and distance vary
    speed        the same timeline for a player whose movement speed an aura changes: Cat Form,
                 Pursuit of Justice, Prowl, and a movement speed enchant on the boots that shares
                 their category
    units        casts, friendly casts, channels, sequence steps and prepull casts whose target
                 names no unit (AllTargets, AllPlayers, a player past the first) or a reference
                 Go reads by its type alone, which Go builds no action for or builds as written
    reaction     includeReactionTime on the activity and the stacks of the auras of the player
                 and of the target
    refresh      auraShouldRefresh on the auras of the player and of the target that belong to an
                 exclusive category, stacking ones included
    armor        auraShouldRefresh and the reaction time of the stacks of the armor debuffs of a
                 Warrior and a Rogue, which they describe unless the raid's Expose Armor holds the
                 category
    friendly     castFriendlySpell of a Paladin written as castSpell at the player
    groups       the priority list cut into groups that references run, referenced once or more,
                 nested, unreferenced or not found, with placeholders the reference fills, value
                 variables and a group's variables that override them

The same options always give the same variants. Compare them with `tools/prepared_v2.py compare`.
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

SEED = 20261009
ITERATIONS = 200

HUMAN_DWARF = ("RaceHuman", "RaceDwarf")
RACES = {
    "ClassMage": ("RaceHuman", "RaceGnome", "RaceUndead", "RaceTroll", "RaceNightElf"),
    "ClassWarlock": ("RaceHuman", "RaceGnome", "RaceOrc", "RaceUndead", "RaceNightElf"),
    "ClassPriest": ("RaceHuman", "RaceDwarf", "RaceNightElf", "RaceUndead", "RaceTroll"),
    "ClassHunter": ("RaceDwarf", "RaceNightElf", "RaceOrc", "RaceTauren", "RaceTroll", "RaceHuman"),
    "ClassDruid": ("RaceNightElf", "RaceTauren"),
    "ClassShaman": ("RaceOrc", "RaceTauren", "RaceTroll", "RaceDwarf"),
    "ClassWarrior": ("RaceHuman", "RaceDwarf", "RaceOrc", "RaceUndead", "RaceTroll", "RaceTauren", "RaceNightElf"),
    "ClassRogue": ("RaceHuman", "RaceDwarf", "RaceNightElf", "RaceOrc", "RaceUndead", "RaceTroll"),
    "ClassPaladin": HUMAN_DWARF,
}

# One accepted request of every spec and role, at one target.
CLASS_REQUESTS = (
    "production-fire", "production-frostfire", "production-affliction-warlock",
    "production-destruction-warlock", "production-demonology-warlock", "production-shadow-priest",
    "production-smite-priest", "production-balance-druid", "production-feral-druid",
    "production-feral-bear-druid", "production-elemental-shaman", "production-enhancement-shaman",
    "production-beast-mastery-hunter", "production-marksmanship-hunter", "production-survival-hunter",
    "production-warrior", "production-arms-warrior", "production-protection-warrior",
    "production-combat-rogue", "production-assassination-rogue", "production-subtlety-rogue",
    "production-retribution-paladin", "production-protection-paladin", "production-holy-protection-paladin",
)

# The same requests at several targets.
SEVERAL_TARGETS = (
    "production-shadow-priest-5-targets", "affliction-warlock-3-targets-multidot-dots",
    "production-arms-warrior-3-targets", "production-protection-warrior-3-targets",
    "production-feral-bear-druid-3-targets", "production-protection-paladin-3-targets",
    "production-survival-hunter-2-targets", "production-assassination-rogue-3-targets",
    "production-elemental-shaman-5-targets", "production-frost-2-targets",
)

# Requests whose player has an aura that changes its movement speed.
SPEED_REQUESTS = (
    "production-feral-druid", "production-feral-bear-druid", "ret-pursuit-of-justice",
    "production-retribution-paladin", "production-protection-paladin",
)

# Requests of a Warrior and a Rogue, whose classes describe the armor category.
ARMOR_REQUESTS = (
    "production-arms-warrior", "production-protection-warrior", "production-warrior",
    "production-combat-rogue", "production-assassination-rogue", "production-subtlety-rogue",
)

# Requests of a Paladin whose rotation casts a friendly spell at the player.
FRIENDLY_REQUESTS = (
    "protection-flash-of-light-self", "shockadin-flash-of-light-blessed", "shockadin-holy-light-self",
    "shockadin-holy-shock-heal",
)

# The accepted fixtures of these gaps, as they are.
FIXTURE_ROWS = (
    "arcane-mage-aura-reaction-time-400ms",
)
FIXTURE_COUNT = 3

# Movement speed enchants for the boots (slot 8): Cat's Swiftness and Boar's Speed.
SPEED_ENCHANTS = (2939, 2940)
BOOTS = 8

MOVE_COUNT = 4
SPEED_COUNT = 3
UNITS_COUNT = 3
REACTION_COUNT = 3
REFRESH_COUNT = 4
GROUPS_COUNT = 3
ARMOR_COUNT = 4
FRIENDLY_COUNT = 3


def request_of(name):
    return json.loads((FIXTURES / f"{name}.request.json").read_text())


def prepared_of(name):
    path = FIXTURES / f"{name}.prepared.json"
    return json.loads(path.read_text()) if path.exists() else None


def player_of(request):
    return request["raid"]["parties"][0]["players"][0]


def const(value):
    return {"const": {"val": value}}


def compare(lhs, op, rhs):
    return {"cmp": {"lhs": lhs, "op": op, "rhs": rhs}}


def time_window(start, end):
    return {"and": {"vals": [
        compare({"currentTime": {}}, "OpGe", const(f"{start}s")),
        compare({"currentTime": {}}, "OpLt", const(f"{end}s"))]}}


def first_spell(request):
    """The action ID of the first cast of the priority list that names a spell, with its tag."""
    for item in player_of(request)["rotation"].get("priorityList", []):
        cast = item.get("action", {}).get("castSpell")
        if cast and cast.get("spellId", {}).get("spellId"):
            return dict(cast["spellId"])
    return None


def cast(spell, target=None, kind="castSpell", condition=None):
    if not isinstance(spell, dict):
        spell = {"spellId": spell}
    body = {"spellId": spell}
    if target is not None:
        body["target"] = target
    action = {kind: body}
    if condition:
        action["condition"] = condition
    return {"action": action}


def prepend(request, items, prepull=()):
    rotation = player_of(request)["rotation"]
    rotation["priorityList"] = list(items) + rotation.get("priorityList", [])
    if prepull:
        rotation["prepullActions"] = list(prepull) + rotation.get("prepullActions", [])


def move_timeline(request, rng):
    """Moves along the fight: a prepull run, then legs that move to a range or for a time, each
    in a window of seconds, some of them back to back so that a move starts as another ends."""
    request = copy.deepcopy(request)
    ranges = (0, 3, 5, 7, 8, 12, 20, 25, 30, 35, 40)
    start = rng.choice((10, 15, 20))
    items = []
    for _ in range(rng.randint(3, 6)):
        end = start + rng.randint(2, 12)
        if rng.random() < 0.3:
            seconds = rng.choice(("1s", "1500ms", "2s", "3s", "5s"))
            action = {"moveDuration": {"duration": const(seconds)}}
        else:
            action = {"move": {"rangeFromTarget": const(str(rng.choice(ranges)))}}
        action["condition"] = time_window(start, end)
        items.append({"action": action})
        start = end + rng.randint(0, 10)
    prepull = []
    if rng.random() < 0.7:
        prepull.append({"action": {"move": {"rangeFromTarget": const(str(rng.choice(ranges)))}},
                        "doAtValue": const(f"-{rng.randint(2, 6)}s")})
    if rng.random() < 0.3:
        prepull.append({"action": {"moveDuration": {"duration": const(rng.choice(("1s", "2s")))}},
                        "doAtValue": const(f"-{rng.randint(1, 3)}s")})
    prepend(request, items, prepull)
    return request, len(items)


def speed_enchant(request, rng):
    """A movement speed enchant on the boots, which shares the passive speed category of the
    speed auras of the class."""
    request = copy.deepcopy(request)
    boots = player_of(request)["equipment"]["items"]
    if len(boots) > BOOTS and boots[BOOTS]:
        boots[BOOTS] = dict(boots[BOOTS], enchant=rng.choice(SPEED_ENCHANTS))
    return request


UNIT_SHAPES = (
    {"type": "AllTargets"},
    {"type": "AllPlayers"},
    {"type": "Player", "index": 1},
    {"type": "Player", "index": 7},
    {"type": "AllTargets", "index": 2, "owner": {"type": "Self"}},
    {"type": "CurrentTarget", "index": 3},
    {"type": "Target", "index": 0, "owner": {"type": "Self"}},
    {"type": "Target", "index": 9},
    {"type": "NextTarget", "index": 1},
    {"type": "PreviousTarget", "owner": {"type": "Self"}},
    {"type": "Self", "index": 4},
)
# The shapes that name a unit a cast of the rotation can reach.
UNIT_SHAPES_AT_THE_TARGET = (
    {"type": "CurrentTarget", "index": 3},
    {"type": "Target", "index": 0, "owner": {"type": "Self"}},
)


def units(request, rng):
    """Casts of the rotation's first spell at units that Go finds none of, in a priority list
    item, a friendly cast, a channel, a step of a sequence and a prepull action, and one at a
    unit Go reads by its type alone. Returns the request and the number of actions added."""
    spell = first_spell(request)
    if spell is None:
        return request, 0
    request = copy.deepcopy(request)
    nobody = [shape for shape in UNIT_SHAPES if shape not in UNIT_SHAPES_AT_THE_TARGET
              and shape["type"] not in ("Self", "NextTarget", "PreviousTarget")]
    pick = lambda: rng.choice(nobody)
    items = [
        cast(spell, pick()),
        cast(spell, pick(), kind="castFriendlySpell"),
        {"action": {"channelSpell": {"spellId": spell, "target": pick()}}},
        {"action": {"sequence": {"actions": [cast(spell, pick())["action"], cast(spell)["action"]]}}},
        {"action": {"strictSequence": {"actions": [cast(spell, pick())["action"], cast(spell)["action"]]}}},
        cast(spell, rng.choice(UNIT_SHAPES_AT_THE_TARGET), condition=compare(
            {"currentTime": {}}, "OpLt", const("1s"))),
    ]
    rng.shuffle(items)
    prepull = [{"action": cast(spell, pick())["action"], "doAtValue": const("-1s")}]
    prepend(request, items, prepull)
    return request, len(items)


def walk(node, visit):
    visit(node)
    if isinstance(node, dict):
        for child in node.values():
            walk(child, visit)
    elif isinstance(node, list):
        for child in node:
            walk(child, visit)


# Spelldata's armor debuffs bid by their stacks. auraShouldRefresh reads them only where a class
# describes their category, as a Warrior and a Rogue do unless the raid's Expose Armor holds it.
BIDS_BY_STACKS = ("Sunder Armor", "Expose Armor")


def described(prepared, unit, aura):
    """Whether an exclusive_category effect of the unit describes a category the aura is in."""
    categories = {membership["category"] for membership in aura["exclusive_memberships"]}
    return any(effect["kind"] == "exclusive_category" and effect["unit"] == unit
               and effect["category"] in categories for effect in prepared.get("effects", []))


def aura_ids(prepared, unit, stacking=False, armor=False):
    """(label, protojson action ID, max stacks) of the unit's auras that belong to an exclusive
    category, as the accepted fixture exported them. The armor debuffs the raid holds are left
    out unless the caller removes the raid's Expose Armor."""
    found = []
    for aura in prepared[unit]["auras"]:
        if (aura["label"].startswith(BIDS_BY_STACKS) and unit == "target" and not armor
                and not described(prepared, unit, aura)):
            continue
        if aura.get("exclusive_memberships") and aura.get("action_id") and (aura["max_stacks"] > 0 or not stacking):
            ids = {{"spell_id": "spellId", "item_id": "itemId", "other_id": "otherId", "tag": "tag"}[key]: value
                   for key, value in aura["action_id"].items()}
            found.append((aura["label"], ids, aura["max_stacks"]))
    return found


def reaction_time(request, prepared, rng):
    """includeReactionTime on the activity and the stacks of auras of the player and of the
    target: the first spell is cast on the strength of each read. Returns the request and the
    number of reads added."""
    spell = first_spell(request)
    if spell is None or prepared is None:
        return request, 0
    request = copy.deepcopy(request)
    items = []
    for unit, source in (("player", {"type": "Self"}), ("target", {"type": "CurrentTarget"}),
                         ("target", {"type": "Target", "index": 0})):
        auras = aura_ids(prepared, unit)
        rng.shuffle(auras)
        for label, ids, stacks in auras[:3]:
            read = {"auraId": ids, "sourceUnit": source, "includeReactionTime": True}
            kind = rng.choice(("auraIsActive", "auraIsInactive") + (("auraNumStacks",) if stacks else ()))
            condition = {kind: read}
            if kind == "auraNumStacks":
                condition = compare(condition, rng.choice(("OpGe", "OpLt", "OpEq")), const(str(rng.randint(0, stacks))))
            items.append(cast(spell, condition=condition))
    prepend(request, items)
    return request, len(items)


def aura_should_refresh(request, prepared, rng):
    """The first spell is cast when an aura of the player or of the target that belongs to an
    exclusive category asks to be refreshed, within an overlap. Returns the request and the number
    of reads added."""
    spell = first_spell(request)
    if spell is None or prepared is None:
        return request, 0
    request = copy.deepcopy(request)
    items = []
    for unit, sources in (("player", ({"type": "Self"}, {"type": "Player"})),
                          ("target", (None, {"type": "CurrentTarget"}, {"type": "Target", "index": 0}))):
        auras = aura_ids(prepared, unit)
        rng.shuffle(auras)
        for label, ids, stacks in auras[:4]:
            read = {"auraId": ids}
            source = rng.choice(sources)
            if source:
                read["sourceUnit"] = source
            overlap = rng.choice((None, "2s", "10s", "30s"))
            if overlap:
                read["maxOverlap"] = const(overlap)
            items.append(cast(spell, condition={"auraShouldRefresh": read}))
    prepend(request, items)
    return request, len(items)


def armor_debuffs(request, prepared, rng):
    """The armor debuffs of the target are read without the raid's Expose Armor, which would hold
    their category: auraShouldRefresh with an overlap, and the stacks with the reaction time."""
    spell = first_spell(request)
    if spell is None or prepared is None:
        return request, 0
    request = copy.deepcopy(request)
    request["raid"].get("debuffs", {}).pop("exposeArmor", None)
    items = []
    for label, ids, stacks in aura_ids(prepared, "target", stacking=True, armor=True):
        if not label.startswith(BIDS_BY_STACKS[0]):
            continue
        for overlap in (None, "3s", "12s"):
            read = {"auraId": ids}
            if overlap:
                read["maxOverlap"] = const(overlap)
            items.append(cast(spell, condition={"auraShouldRefresh": read}))
        items.append(cast(spell, condition=compare(
            {"auraNumStacks": {"auraId": ids, "sourceUnit": {"type": "CurrentTarget"},
                               "includeReactionTime": True}},
            rng.choice(("OpLt", "OpGe")), const(str(rng.randint(1, stacks))))))
    rng.shuffle(items)
    prepend(request, items)
    return request, len(items)


def friendly_as_cast(request):
    """Every castFriendlySpell is a castSpell at the player."""
    request = copy.deepcopy(request)
    changed = 0
    for item in player_of(request)["rotation"].get("priorityList", []):
        body = item["action"].pop("castFriendlySpell", None)
        if body is not None:
            body.setdefault("target", {"type": "Self"})
            item["action"]["castSpell"] = body
            changed += 1
    return request, changed


def group(name, actions, variables=None):
    body = {"name": name, "actions": actions}
    if variables:
        body["variables"] = [{"name": key, "value": value} for key, value in variables.items()]
    return body


def reference(name, variables=None, condition=None):
    body = {"groupName": name}
    if variables:
        body["variables"] = [{"name": key, "value": value} for key, value in variables.items()]
    action = {"groupReference": body}
    if condition:
        action["condition"] = condition
    return {"action": action}


GROUP_SHAPES = ("once", "condition", "twice", "placeholder", "nested", "override", "extras", "variable")
# A draw of the sweep takes one of the shapes above or a random cut of the whole list.
GROUP_DRAWS = GROUP_SHAPES + ("random",) * 4


def gated(old, extra):
    """A condition that also needs the extra term."""
    return {"and": {"vals": [extra] + ([old] if old else [])}}


def random_groups(request, rng):
    """The priority list cut at random into two to four groups, each with plain conditions, a
    placeholder its reference fills, a value variable, a variable it overrides or a constant
    condition Go keeps; referenced in a shuffled order, some twice, some with a condition,
    some not at all, one nested in another, and one reference to a group that is not there."""
    rotation = player_of(request)["rotation"]
    items = rotation.get("priorityList", [])
    cuts = sorted(rng.sample(range(1, len(items)), min(len(items) - 1, rng.randint(1, 3))))
    pieces, last = [], 0
    for cut in cuts + [len(items)]:
        pieces.append(copy.deepcopy(items[last:cut]))
        last = cut
    names = [f"g{index}" for index in range(len(pieces))]
    cut_groups, variables, fills = [], [], {}
    for index, piece in enumerate(pieces):
        own = {}
        mode = rng.choice(("plain", "plain", "placeholder", "variable", "override", "constant"))
        for item in piece:
            old = item["action"].get("condition")
            if mode == "placeholder":
                item["action"]["condition"] = gated(old, {"variablePlaceholder": {"name": f"p{index}"}})
            elif mode == "variable":
                item["action"]["condition"] = gated(old, {"variableRef": {"name": f"v{index}"}})
            elif mode == "override":
                item["action"]["condition"] = gated(old, {"variableRef": {"name": f"o{index}"}})
            elif mode == "constant":
                item["action"]["condition"] = gated(old, const(rng.choice(("true", "true", "false"))))
        if mode == "placeholder":
            fills[index] = {f"p{index}": compare({"currentTime": {}}, rng.choice(("OpGe", "OpLt")),
                                                 const(f"{rng.randint(0, 40)}s"))}
        elif mode == "variable":
            variables.append({"name": f"v{index}", "value": compare(
                {"currentTime": {}}, "OpGe", const(f"{rng.randint(0, 10)}s"))})
        elif mode == "override":
            variables.append({"name": f"o{index}", "value": const(rng.choice(("true", "false")))})
            own[f"o{index}"] = compare({"currentTime": {}}, "OpGe", const(f"{rng.randint(0, 30)}s"))
        cut_groups.append(group(names[index], piece, own or None))
    nested = None
    if len(pieces) > 1 and rng.random() < 0.4:
        outer, inner = rng.sample(range(len(pieces)), 2)
        cut_groups[outer]["actions"].append(reference(names[inner], fills.get(inner)))
        nested = inner
    top = []
    order = list(range(len(pieces)))
    rng.shuffle(order)
    for index in order:
        if rng.random() < 0.15 or (index == nested and rng.random() < 0.5):
            continue
        when = compare({"currentTime": {}}, "OpGe", const(f"{rng.randint(0, 5)}s")) if rng.random() < 0.3 else None
        top.append(reference(names[index], fills.get(index), when))
        if rng.random() < 0.2:
            top.append(reference(names[index], fills.get(index)))
    if rng.random() < 0.15:
        top.append(reference("nowhere"))
    rotation["priorityList"] = top
    rotation["groups"] = cut_groups
    if variables:
        rotation["valueVariables"] = variables


def groups(request, rng, shape=None):
    """The priority list cut into groups: one group referenced once, twice, with a condition,
    with a placeholder the reference fills, nested, with a value variable that a group's
    variable overrides, and unreferenced, unfound and used groups. Returns the request and the
    number of references."""
    request = copy.deepcopy(request)
    rotation = player_of(request)["rotation"]
    items = rotation.get("priorityList", [])
    if len(items) < 3:
        return request, 0
    cut = rng.randint(1, len(items) - 2)
    end = rng.randint(cut + 1, len(items))
    middle, tail = items[cut:end], items[end:]
    early = compare({"currentTime": {}}, "OpGe", const(f"{rng.randint(1, 6)}s"))
    shape = shape or rng.choice(GROUP_DRAWS)
    if shape == "random":
        random_groups(request, rng)
        return request, 1
    if shape == "once":
        rotation["groups"] = [group("g", middle)]
        rotation["priorityList"] = items[:cut] + [reference("g")] + tail
    elif shape == "condition":
        rotation["groups"] = [group("g", middle)]
        rotation["priorityList"] = items[:cut] + [reference("g", condition=early)] + tail
    elif shape == "twice":
        rotation["groups"] = [group("g", middle)]
        rotation["priorityList"] = (items[:cut] + [reference("g", condition=compare(
            {"remainingTime": {}}, "OpLt", const("40s")))] + middle + [reference("g")] + tail)
    elif shape == "placeholder":
        gated = copy.deepcopy(middle)
        for item in gated:
            old = item["action"].get("condition")
            terms = [{"variablePlaceholder": {"name": "when"}}] + ([old] if old else [])
            item["action"]["condition"] = {"and": {"vals": terms}}
        rotation["groups"] = [group("g", gated)]
        rotation["priorityList"] = items[:cut] + [reference("g", {"when": early})] + tail
    elif shape == "nested":
        rotation["groups"] = [group("outer", middle + [reference("inner")]), group("inner", tail)]
        rotation["priorityList"] = items[:cut] + [reference("outer")]
    elif shape == "override":
        rotation["valueVariables"] = [{"name": "open", "value": const(rng.choice(("true", "false")))}]
        gated = copy.deepcopy(middle)
        for item in gated:
            old = item["action"].get("condition")
            terms = [{"variableRef": {"name": "open"}}] + ([old] if old else [])
            item["action"]["condition"] = {"and": {"vals": terms}}
        rotation["groups"] = [group("g", gated, {"open": early})]
        rotation["priorityList"] = items[:cut] + [reference("g")] + tail
    elif shape == "extras":
        spell = first_spell(request)
        used = compare({"currentTime": {}}, "OpGe", const("0s"))
        rotation["groups"] = [group("unused", middle), group("g", [
            {"action": dict(items[0]["action"], condition={"and": {"vals": [
                {"actionGroupUsed": {"name": "g"}}, used]}})}])]
        rotation["priorityList"] = items[:cut] + [reference("nowhere"), reference("g")] + items[cut:]
    else:
        # A condition of the rotation replaced by a variable of the same value.
        for position, item in enumerate(items):
            condition = item["action"].get("condition")
            if condition:
                rotation["valueVariables"] = [{"name": "cond", "value": condition}]
                for other in items:
                    if other["action"].get("condition") == condition:
                        other["action"]["condition"] = {"variableRef": {"name": "cond"}}
                break
    return request, 1


def rows():
    """Every row: (name, base request, count, races), in a fixed order."""
    out = []
    for name in CLASS_REQUESTS + SEVERAL_TARGETS:
        base = request_of(name)
        races = RACES[player_of(base)["class"]]
        rng = random.Random(f"moves-{name}")
        moved, _ = move_timeline(base, rng)
        out.append((f"moves-{name}", moved, MOVE_COUNT, races))
    for name in SPEED_REQUESTS:
        base = request_of(name)
        races = RACES[player_of(base)["class"]]
        rng = random.Random(f"speed-{name}")
        moved, _ = move_timeline(base, rng)
        out.append((f"speed-{name}", speed_enchant(moved, rng), SPEED_COUNT, races))
    for name in CLASS_REQUESTS + SEVERAL_TARGETS:
        base = request_of(name)
        races = RACES[player_of(base)["class"]]
        prepared = prepared_of(name)
        for label, transform, count in (
                ("units", lambda r, g: units(r, g), UNITS_COUNT),
                ("reaction", lambda r, g: reaction_time(r, prepared, g), REACTION_COUNT),
                ("refresh", lambda r, g: aura_should_refresh(r, prepared, g), REFRESH_COUNT),
                ("groups", lambda r, g: groups(r, g), GROUPS_COUNT)):
            changed, number = transform(base, random.Random(f"{label}-{name}"))
            if number:
                out.append((f"{label}-{name}", changed, count, races))
    for name in ARMOR_REQUESTS:
        base = request_of(name)
        changed, number = armor_debuffs(base, prepared_of(name), random.Random(f"armor-{name}"))
        if number:
            out.append((f"armor-{name}", changed, ARMOR_COUNT, RACES[player_of(base)["class"]]))
    for name in FRIENDLY_REQUESTS:
        base = request_of(name)
        changed, number = friendly_as_cast(base)
        if number:
            out.append((f"friendly-{name}", changed, FRIENDLY_COUNT, RACES[player_of(base)["class"]]))
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


def keep_boots(base, request):
    """The speed rows keep the enchanted boots of the base, which the draw would empty."""
    items = player_of(request)["equipment"]["items"]
    kept = player_of(base)["equipment"]["items"]
    if len(items) > BOOTS and len(kept) > BOOTS:
        items[BOOTS] = copy.deepcopy(kept[BOOTS])
    return request


def variants():
    """(file stem, request) for every variant of every row."""
    out = []
    for index, (name, base, count, races) in enumerate(rows()):
        for number, request in enumerate(generate(base, SEED + index, count, ITERATIONS, races)):
            request = keep_weapons(base, request)
            if name.startswith("speed-"):
                request = keep_boots(base, request)
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
