"""Gear, enchant, consumable and race swap variants of the production requests.

python3 output/gearswap.py OUT_DIR ITERATIONS

Each variant changes one thing in one production request: an equipment slot takes an item
another production request wears in that slot (if the UI's canEquipItem lets the class wear
it), a slot's enchant takes one another request uses there, a consumable takes another
request's choice, or the race takes another the class can be. Writes the variants and an
index of what each changes.
"""
import glob
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


