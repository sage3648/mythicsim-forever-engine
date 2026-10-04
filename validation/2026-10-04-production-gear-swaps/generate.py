"""Gear, enchant, consumable and race swap variants of the production requests, by the slot
Go equips each item into (core/database.go EquipItem places items by type, in list order).
Each variant changes one thing; item eligibility follows the UI's canEquipItem
(ui/sim/proto/items.ts) with the class tables of ui/sim/player/classes/capabilities_auto_gen.ts.
The same inputs always give the same variants. Uses only Python's standard library and the
pinned engine's item database.

python3 validation/2026-10-04-production-gear-swaps/generate.py OUT_DIR ITERATIONS
"""
import glob
from collections import Counter
import json
import sys
from pathlib import Path


DB = json.load(open("oracle-cache/source/assets/database/db.json"))
ITEMS = {item["id"]: item for item in DB["items"]}

# ui/sim/player/classes/capabilities_auto_gen.ts, by proto enum value.
ARMOR = {"ClassWarrior": 4, "ClassPaladin": 4, "ClassHunter": 3, "ClassRogue": 2, "ClassPriest": 1,
         "ClassShaman": 3, "ClassMage": 1, "ClassWarlock": 1, "ClassDruid": 2}
CLASS_ID = {"ClassWarrior": 1, "ClassPaladin": 2, "ClassHunter": 3, "ClassRogue": 4, "ClassPriest": 5,
            "ClassShaman": 7, "ClassMage": 8, "ClassWarlock": 9, "ClassDruid": 11}
# weapon type: can use two hand
WEAPONS = {
    "ClassWarrior": {1: True, 2: False, 3: False, 4: True, 5: False, 6: True, 7: False, 8: True, 9: True},
    "ClassPaladin": {1: True, 4: True, 6: True, 7: False, 9: True},
    "ClassHunter": {1: True, 2: False, 3: False, 6: True, 8: True, 9: True},
    "ClassRogue": {1: False, 2: False, 3: False, 4: False, 5: False, 9: False},
    "ClassPriest": {2: False, 4: False, 5: False, 8: True},
    "ClassShaman": {1: True, 2: False, 3: False, 4: True, 5: False, 7: False, 8: True},
    "ClassMage": {2: False, 5: False, 8: True, 9: False},
    "ClassWarlock": {2: False, 5: False, 8: True, 9: False},
    "ClassDruid": {2: False, 3: False, 4: True, 5: False, 6: True, 8: True},
}
RANGED = {"ClassWarrior": {1, 2, 3, 4}, "ClassPaladin": {7}, "ClassHunter": {1, 2, 3}, "ClassRogue": {1, 2, 3, 4},
          "ClassPriest": {5}, "ClassShaman": {8}, "ClassMage": {5}, "ClassWarlock": {5}, "ClassDruid": {6}}
RACES = {
    "ClassWarrior": ["RaceHuman", "RaceDwarf", "RaceNightElf", "RaceGnome", "RaceHighOrderSkyborne", "RaceOrc", "RaceUndead",
                     "RaceTauren", "RaceTroll", "RaceWindshaperSkyborne"],
    "ClassPaladin": ["RaceHuman", "RaceDwarf", "RaceUndead"],
    "ClassHunter": ["RaceHuman", "RaceDwarf", "RaceNightElf", "RaceHighOrderSkyborne", "RaceOrc", "RaceTauren", "RaceTroll",
                    "RaceWindshaperSkyborne"],
    "ClassRogue": ["RaceHuman", "RaceDwarf", "RaceNightElf", "RaceGnome", "RaceHighOrderSkyborne", "RaceOrc", "RaceUndead",
                   "RaceTroll", "RaceWindshaperSkyborne"],
    "ClassPriest": ["RaceHuman", "RaceDwarf", "RaceNightElf", "RaceGnome", "RaceUndead", "RaceTroll"],
    "ClassShaman": ["RaceDwarf", "RaceOrc", "RaceTauren", "RaceTroll", "RaceWindshaperSkyborne"],
    "ClassMage": ["RaceHuman", "RaceGnome", "RaceHighOrderSkyborne", "RaceOrc", "RaceUndead", "RaceTroll"],
    "ClassWarlock": ["RaceHuman", "RaceGnome", "RaceOrc", "RaceUndead", "RaceTroll"],
    "ClassDruid": ["RaceNightElf", "RaceHighOrderSkyborne", "RaceTauren", "RaceWindshaperSkyborne"],
}
DUAL_WIELD_SPECS = {"hunter", "rogue", "enhancementShaman", "dpsWarrior", "protectionWarrior"}
MAIN_HAND, OFF_HAND, RANGED_SLOT = 14, 15, 16
FINGERS, TRINKETS = (10, 11), (12, 13)
# Slot families that share items.
FAMILY = {s: s for s in range(17)}
FAMILY.update({11: 10, 13: 12})
IMBUES = {"mhImbueId", "ohImbueId"}


def player(request):
    return request["raid"]["parties"][0]["players"][0]


def spec(p):
    known = {"buffs", "channelClipDelayMs", "class", "consumables", "distanceFromTarget", "equipment", "name",
             "profession1", "profession2", "race", "reactionTimeMs", "rotation", "talentsString", "cooldowns",
             "healingModel", "inFrontOfTarget", "bonusStats", "enableItemSwap", "itemSwap"}
    return next(k for k in p if k not in known)


def can_equip(cls, spec_name, item, slot):
    """ui/sim/proto/items.ts canEquipItem."""
    if item.get("classAllowlist") and CLASS_ID[cls] not in item["classAllowlist"]:
        return False
    kind = item.get("type", 0)
    if kind in (11, 12):
        return True
    if kind == 13:
        weapon = item.get("weaponType", 0)
        if weapon not in WEAPONS[cls]:
            return False
        hand = item.get("handType", 0)
        if (hand == 3 or (hand == 2 and slot == OFF_HAND)) and weapon not in (5, 7) and spec_name not in DUAL_WIELD_SPECS:
            return False
        if hand == 4 and (not WEAPONS[cls][weapon] or slot == OFF_HAND):
            return False
        return True
    if kind == 14:
        return item.get("rangedWeaponType", 0) in RANGED[cls]
    return ARMOR[cls] >= item.get("armorType", 0)

OUT = Path(sys.argv[1])
ITERATIONS = int(sys.argv[2])
OUT.mkdir(parents=True, exist_ok=True)
ITEMS = ITEMS
MAIN_HAND, OFF_HAND, RANGED_SLOT = 14, 15, 16


def player(request):
    return request["raid"]["parties"][0]["players"][0]


def equip(entries):
    """Go EquipItem over the list: the slot of each entry, by list position."""
    slots = {}
    taken = {}
    for position, entry in enumerate(entries):
        item = ITEMS.get(entry.get("id", 0))
        if not item:
            continue
        kind, hand, weapon = item.get("type", 0), item.get("handType", 0), item.get("weaponType", 0)
        if kind == 11:
            slot = 10 if 10 not in taken else 11
        elif kind == 12:
            slot = 12 if 12 not in taken else 13
        elif kind == 13:
            main = ITEMS.get(taken.get(MAIN_HAND, {}).get("id", 0), {})
            if weapon == 7 and main.get("handType") != 4:
                slot = OFF_HAND
            elif hand in (1, 0):
                slot = MAIN_HAND
            elif hand == 3:
                slot = OFF_HAND
            elif MAIN_HAND not in taken:
                slot = MAIN_HAND
            elif OFF_HAND not in taken:
                slot = OFF_HAND
            else:
                continue
        elif kind == 14:
            slot = RANGED_SLOT
        else:
            slot = kind - 1
        taken[slot] = entry
        slots[position] = slot
    return slots


def by_slot(entries):
    return {slot: position for position, slot in equip(entries).items()}


requests = {Path(f).name.replace(".request.json", ""): json.load(open(f)) for f in sorted(glob.glob(str(Path(__file__).parent / "requests" / "*.json")))}
FAMILY = {s: s for s in range(17)}
FAMILY.update({11: 10, 13: 12})

pools = {}
enchants = {}
for request in requests.values():
    entries = player(request)["equipment"]["items"]
    for position, slot in equip(entries).items():
        entry = entries[position]
        pool = pools.setdefault(FAMILY[slot], [])
        if json.dumps(entry, sort_keys=True) not in [json.dumps(e, sort_keys=True) for e in pool]:
            pool.append(entry)
        if entry.get("enchant"):
            enchants.setdefault(slot, set()).add(entry["enchant"])
consumables = {}
for request in requests.values():
    for key, value in player(request)["consumables"].items():
        consumables.setdefault(key, [])
        if (value, player(request)["class"]) not in consumables[key]:
            consumables[key].append((value, player(request)["class"]))

index = []


def write(name, request, base, change, description, intended=None):
    variant = json.loads(json.dumps(request))
    change(variant)
    entries = player(variant)["equipment"]["items"]
    if intended is not None:
        slot, item_id = intended
        placed = by_slot(entries)
        if slot not in placed or entries[placed[slot]].get("id") != item_id:
            return
    variant["simOptions"]["iterations"] = ITERATIONS
    json.dump(variant, open(OUT / f"{name}.request.json", "w"))
    index.append({"scenario": name, "base": base, "change": description})


for base, request in requests.items():
    p = player(request)
    cls, spec_name = p["class"], spec(p)
    entries = p["equipment"]["items"]
    placed = by_slot(entries)
    for slot in range(17):
        own = entries[placed[slot]] if slot in placed else {}
        for entry in pools.get(FAMILY[slot], []):
            item = ITEMS.get(entry["id"])
            if item is None or entry["id"] == own.get("id"):
                continue
            if not can_equip(cls, spec_name, item, slot):
                continue
            partner = {10: 11, 11: 10, 12: 13, 13: 12}.get(slot)
            if partner in placed and entries[placed[partner]].get("id") == entry["id"]:
                continue
            if slot == OFF_HAND and MAIN_HAND in placed and ITEMS[entries[placed[MAIN_HAND]]["id"]].get("handType") == 4:
                continue

            def change(variant, slot=slot, entry=entry, item=item):
                items = player(variant)["equipment"]["items"]
                where = by_slot(items)
                if slot in where:
                    items[where[slot]] = entry
                else:
                    items.append(entry)
                if slot == MAIN_HAND and item.get("handType") == 4 and OFF_HAND in where:
                    items[where[OFF_HAND]] = {}
            write(f"{base}--slot{slot}-{entry['id']}-{entry.get('enchant', 0)}", request, base, change,
                  {"kind": "item", "slot": slot, "item": entry}, intended=(slot, entry["id"]))
        if own:
            for enchant in sorted(enchants.get(slot, set()) | enchants.get(FAMILY[slot], set())):
                if enchant == own.get("enchant"):
                    continue

                def change(variant, position=placed[slot], enchant=enchant):
                    player(variant)["equipment"]["items"][position]["enchant"] = enchant
                write(f"{base}--enchant{slot}-{enchant}", request, base, change,
                      {"kind": "enchant", "slot": slot, "enchant": enchant})
    for key, values in consumables.items():
        for value, owner in values:
            if p["consumables"].get(key) == value or (key in IMBUES and owner != cls):
                continue

            def change(variant, key=key, value=value):
                player(variant)["consumables"][key] = value
            label = value if not isinstance(value, list) else "-".join(map(str, value))
            write(f"{base}--consumable-{key}-{label}", request, base, change,
                  {"kind": "consumable", "field": key, "value": value})
    for race in RACES[cls]:
        if race == p["race"]:
            continue

        def change(variant, race=race):
            player(variant)["race"] = race
        write(f"{base}--race-{race}", request, base, change, {"kind": "race", "race": race})
    # The base itself, to tell rejections the swap causes from the base's own.
    write(f"{base}--base", request, base, lambda variant: None, {"kind": "base"})

json.dump(index, open(OUT / "index.json", "w"), indent=1)
print(len(index), Counter(entry["change"]["kind"] for entry in index))
