"""Trinket and enchant variants of the production requests, from the app's Forever catalogs.

python3 THIS OUT_DIR ITERATIONS PARTNER_TRINKET

Trinkets: each trinket of the app's forever-trinkets.json the class may wear goes alone into
the first trinket slot, and behind a fixed stat trinket into the second. Enchants: each
enchant of the app's Forever gear index (forever-gear-index.json `enchants`) that the pinned
engine knows goes on every worn item it applies to, by the UI's enchantAppliesToItem and
canEquipEnchant.
"""
import glob
import importlib.util
import json
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).parent
spec_ = importlib.util.spec_from_file_location("tables", HERE / "tables.py")
tables = importlib.util.module_from_spec(spec_)
spec_.loader.exec_module(tables)

OUT = Path(sys.argv[1])
ITERATIONS = int(sys.argv[2])
OUT.mkdir(parents=True, exist_ok=True)
DB = json.load(open("oracle-cache/source/assets/database/db.json"))
ITEMS = {item["id"]: item for item in DB["items"]}
ENCHANTS = {e["effectId"]: e for e in DB["enchants"]}
TRINKETS = json.load(open(HERE / "catalogs" / "forever-trinkets.json"))["trinkets"]
BUILDER_ENCHANTS = json.load(open(HERE / "catalogs" / "forever-gear-index.json"))["enchants"]
CLASS_NAMES = {"Warrior": "ClassWarrior", "Paladin": "ClassPaladin", "Hunter": "ClassHunter", "Rogue": "ClassRogue",
               "Priest": "ClassPriest", "Shaman": "ClassShaman", "Mage": "ClassMage", "Warlock": "ClassWarlock",
               "Druid": "ClassDruid"}
# A stat trinket with no effect, worn in the first slot so the trinket under test lands in the second.
PARTNER = int(sys.argv[3]) if len(sys.argv) > 3 else None
MAIN_HAND, OFF_HAND = 14, 15


def player(request):
    return request["raid"]["parties"][0]["players"][0]


def item_slots(item):
    kind = item.get("type", 0)
    if kind == 11:
        return {10, 11}
    if kind == 12:
        return {12, 13}
    if kind == 14:
        return {16}
    if kind == 13:
        hand = item.get("handType", 0)
        return {MAIN_HAND} if hand == 1 else {OFF_HAND} if hand == 3 else {MAIN_HAND, OFF_HAND}
    return {kind - 1}


def enchant_slots(enchant):
    slots = set()
    for kind in [enchant.get("type", 0)] + list(enchant.get("extraTypes") or []):
        if kind == 13:
            slots |= {MAIN_HAND, OFF_HAND}
        elif kind:
            slots |= item_slots({"type": kind})
    return slots


def enchant_applies(enchant, item):
    """ui/sim/proto/items.ts enchantAppliesToItem."""
    if not enchant_slots(enchant) & item_slots(item):
        return False
    kind, hand, weapon, ranged = (enchant.get("enchantType", 0), item.get("handType", 0), item.get("weaponType", 0),
                                  item.get("rangedWeaponType", 0))
    if kind == 1 and hand != 4:
        return False
    if kind == 4 and weapon != 8:
        return False
    if kind == 2 and weapon != 7:
        return False
    if (kind == 5) != (weapon == 5):
        return False
    if weapon == 7 and kind != 2:
        return False
    if enchant.get("type") == 14 and ranged not in (1, 2, 3):
        return False
    if ranged != 5 and ranged > 0 and enchant.get("type") != 14:
        return False
    return True


def can_enchant(enchant, p):
    """canEquipEnchant: the class allowlist and the Enchanting profession."""
    allow = enchant.get("classAllowlist") or []
    if allow and tables.CLASS_ID[p["class"]] not in allow:
        return False
    if enchant.get("requiredProfession") and "ProfessionEnchanting" not in (p.get("profession1"), p.get("profession2")):
        return False
    return True


requests = {Path(f).name.replace(".request.json", ""): json.load(open(f))
            for f in sorted(glob.glob(str(HERE.parent / "2026-10-04-production-gear-swaps" / "requests" / "*.json")))}
index = []


def write(name, request, base, change, description):
    variant = json.loads(json.dumps(request))
    change(variant)
    variant["simOptions"]["iterations"] = ITERATIONS
    json.dump(variant, open(OUT / f"{name}.request.json", "w"))
    index.append({"scenario": name, "base": base, "change": description})


for base, request in requests.items():
    p = player(request)
    cls = p["class"]
    for trinket in TRINKETS:
        if trinket["classes"] and cls not in {CLASS_NAMES.get(c) for c in trinket["classes"]}:
            continue
        item = ITEMS.get(trinket["id"])
        if not item or item.get("classAllowlist") and tables.CLASS_ID[cls] not in item["classAllowlist"]:
            continue

        def alone(variant, tid=trinket["id"]):
            player(variant)["equipment"]["items"].append({"id": tid})
        write(f"{base}--trinket1-{trinket['id']}", request, base, alone,
              {"kind": "trinket", "slot": 12, "item": trinket["id"], "name": trinket["name"]})
        if PARTNER and trinket["id"] != PARTNER:
            def second(variant, tid=trinket["id"]):
                player(variant)["equipment"]["items"].extend([{"id": PARTNER}, {"id": tid}])
            write(f"{base}--trinket2-{trinket['id']}", request, base, second,
                  {"kind": "trinket", "slot": 13, "item": trinket["id"], "name": trinket["name"], "partner": PARTNER})
    entries = p["equipment"]["items"]
    for position, entry in enumerate(entries):
        item = ITEMS.get(entry.get("id", 0))
        if not item:
            continue
        for effect in BUILDER_ENCHANTS:
            enchant = ENCHANTS.get(effect)
            if not enchant or effect == entry.get("enchant") or not enchant_applies(enchant, item) or not can_enchant(enchant, p):
                continue

            def change(variant, position=position, effect=effect):
                player(variant)["equipment"]["items"][position]["enchant"] = effect
            write(f"{base}--enchant-{entry['id']}-{effect}", request, base, change,
                  {"kind": "enchant", "item": entry["id"], "enchant": effect, "name": enchant["name"]})
    write(f"{base}--base", request, base, lambda variant: None, {"kind": "base"})

json.dump(index, open(OUT / "index.json", "w"), indent=1)
print(len(index), Counter(e["change"]["kind"] for e in index))
