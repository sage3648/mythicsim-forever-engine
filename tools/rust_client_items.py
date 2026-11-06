#!/usr/bin/env python3
"""Write data/forever-client-items.jsonl: the equippable items of the live client that the pinned
reference database (data/items.jsonl) lacks, such as the grey cloaks and shields a leveling
character wears.

Under Forever's rules preparation reads these rows for an item id data/items.jsonl does not hold
(src/prepare/items.rs). Rows use the items.jsonl row schema.

Sources, at the data's client build (data/manifest.json):

    https://wago.tools/db2/ItemSparse/csv?product=wowf   names, quality, slot, item delay
    https://wago.tools/db2/Item/csv?product=wowf         class and subclass
    https://nether.wowhead.com/forever/tooltip/item/<id>?x=<random>
                                                       armor, damage, speed, block, stats

An item is taken when it is a weapon or armor row, not in items.jsonl, with an equipment slot,
a real name (no deprecated, test, old or placeholder rows) and a tooltip with no use, equip or
proc effect. An item whose tooltip is missing or has a line this tool does not read is left out
and listed.

    python3 tools/rust_client_items.py fetch  --tables DIR --cache DIR
    python3 tools/rust_client_items.py build  --tables DIR --cache DIR [--report FILE]
    python3 tools/rust_client_items.py check

fetch asks for each missing tooltip about twice a second, backs off on 403 and 429 and keeps every
response in the cache, so a rerun, and build, need no network. DIR for --tables holds
ItemSparse.csv and Item.csv. check is an offline audit of the committed file.
"""

import argparse
import csv
import html
import json
from pathlib import Path
import random
import re
import sys
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
ITEMS = DATA / "items.jsonl"
OUT = DATA / "forever-client-items.jsonl"

TOOLTIP_URL = "https://nether.wowhead.com/forever/tooltip/item/{id}?x={cache_buster}"
USER_AGENT = "Mozilla/5.0 (compatible; item-data-generator)"
REQUEST_INTERVAL = 0.5

# InventoryType values that are not worn: none, shirt, bag, tabard, ammunition.
NOT_WORN = {0, 4, 18, 19, 24}
WEAPON_CLASS, ARMOR_CLASS = 2, 4

# A name that marks a row as development data rather than an item a player holds.
JUNK_NAME = re.compile(
    r"deprecated|\btest|qatest|^old|\(old\)|\[ph\]|placeholder|monster|unused|^\d+ (green|blue|epic|rare)"
    r"|ggggfg|delete me|\(dnt\)|\bph\b|\btwain\b|trash weapon|warlock orb|aura proc",
    re.IGNORECASE,
)

ITEM_TYPE = {
    1: "ItemTypeHead",
    2: "ItemTypeNeck",
    3: "ItemTypeShoulder",
    5: "ItemTypeChest",
    6: "ItemTypeWaist",
    7: "ItemTypeLegs",
    8: "ItemTypeFeet",
    9: "ItemTypeWrist",
    10: "ItemTypeHands",
    11: "ItemTypeFinger",
    12: "ItemTypeTrinket",
    16: "ItemTypeBack",
    20: "ItemTypeChest",
}
ARMOR_TYPE = {
    1: "ArmorTypeCloth",
    2: "ArmorTypeLeather",
    3: "ArmorTypeMail",
    4: "ArmorTypePlate",
}
# Weapon subclass to the weapon type of a melee weapon.
WEAPON_TYPE = {
    0: "WeaponTypeAxe",
    1: "WeaponTypeAxe",
    4: "WeaponTypeMace",
    5: "WeaponTypeMace",
    6: "WeaponTypePolearm",
    7: "WeaponTypeSword",
    8: "WeaponTypeSword",
    10: "WeaponTypeStaff",
    13: "WeaponTypeFist",
    15: "WeaponTypeDagger",
}
# Weapon subclass to the ranged weapon type of an item in the ranged slot.
RANGED_TYPE = {
    2: "RangedWeaponTypeBow",
    3: "RangedWeaponTypeGun",
    16: "RangedWeaponTypeThrown",
    18: "RangedWeaponTypeCrossbow",
    19: "RangedWeaponTypeWand",
}
HAND_TYPE = {
    13: "HandTypeOneHand",
    17: "HandTypeTwoHand",
    21: "HandTypeMainHand",
    22: "HandTypeOffHand",
}

STAT_INDEX = {
    "Strength": 0,
    "Agility": 1,
    "Stamina": 2,
    "Intellect": 3,
    "Spirit": 16,
    "Arcane Resistance": 35,
    "Fire Resistance": 36,
    "Frost Resistance": 37,
    "Nature Resistance": 38,
    "Shadow Resistance": 39,
}
STAT_ARMOR, STAT_BLOCK_VALUE = 30, 27


def read_csv(path):
    with open(path, newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def known_ids():
    with open(ITEMS) as rows:
        return {json.loads(line)["id"] for line in rows}


def candidates(tables):
    """The weapon and armor rows with a slot that items.jsonl lacks, as (id, sparse, item), and
    the rows left out for their name, by id."""
    sparse = {int(row["ID"]): row for row in read_csv(Path(tables) / "ItemSparse.csv")}
    classes = {int(row["ID"]): row for row in read_csv(Path(tables) / "Item.csv")}
    known = known_ids()
    taken, junk = [], {}
    for item_id in sorted(sparse):
        row = sparse[item_id]
        item = classes.get(item_id)
        if item_id in known or item is None:
            continue
        if int(row["InventoryType"]) in NOT_WORN:
            continue
        if int(item["ClassID"]) not in (WEAPON_CLASS, ARMOR_CLASS):
            continue
        name = row["Display_lang"]
        if JUNK_NAME.search(name):
            junk[item_id] = name
            continue
        taken.append((item_id, row, item))
    return taken, junk


def cache_path(cache, item_id):
    return Path(cache) / f"{item_id}.json"


def fetch_tooltip(item_id):
    """One tooltip response as text, with a cache buster in the query."""
    url = TOOLTIP_URL.format(id=item_id, cache_buster=random.randrange(10**9))
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read().decode("utf-8")


def fetch(tables, cache):
    """Fetch every candidate's tooltip that the cache lacks."""
    Path(cache).mkdir(parents=True, exist_ok=True)
    taken, _ = candidates(tables)
    wanted = [item_id for item_id, _, _ in taken if not cache_path(cache, item_id).exists()]
    print(f"{len(taken)} candidates, {len(wanted)} tooltips to fetch", file=sys.stderr)
    delay = REQUEST_INTERVAL
    index = 0
    while index < len(wanted):
        item_id = wanted[index]
        try:
            text = fetch_tooltip(item_id)
            json.loads(text)
        except urllib.error.HTTPError as err:
            if err.code in (403, 429):
                delay = min(delay * 2, 60)
                print(f"HTTP {err.code} at {item_id}, waiting {delay:.0f}s", file=sys.stderr)
                time.sleep(delay * 4)
                continue
            if err.code == 404:
                cache_path(cache, item_id).write_text(json.dumps({"missing": True}))
                index += 1
                continue
            raise
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as err:
            print(f"{item_id}: {err}, retrying", file=sys.stderr)
            time.sleep(5)
            continue
        cache_path(cache, item_id).write_text(text)
        index += 1
        delay = max(REQUEST_INTERVAL, delay * 0.9)
        time.sleep(delay)
        if index % 50 == 0:
            print(f"{index}/{len(wanted)}", file=sys.stderr)


class Unreadable(Exception):
    """A tooltip this tool will not turn into a row; the message is the reason listed."""


SLOT_WORDS = {
    "Head", "Neck", "Shoulder", "Back", "Chest", "Waist", "Legs", "Feet", "Wrist", "Hands",
    "Finger", "Trinket", "One-Hand", "Two-Hand", "Main Hand", "Off Hand", "Held In Off-hand",
    "Ranged", "Thrown",
}  # fmt: skip
TYPE_WORDS = {
    "Cloth", "Leather", "Mail", "Plate", "Shield", "Axe", "Mace", "Sword", "Dagger", "Staff",
    "Polearm", "Fist Weapon", "Bow", "Crossbow", "Gun", "Wand", "Thrown", "Miscellaneous",
}  # fmt: skip
EFFECT_LINE = re.compile(r"^(Equip|Use|Chance on hit):", re.IGNORECASE)
STAT_LINE = re.compile(r"^\+(\d+) ([A-Za-z ]+)$")


def tooltip_lines(tooltip):
    """The visible text of a tooltip, one entry per line, with the data comments kept as text
    for the numbers this tool reads (<!--ilvl-->15 reads as the number alone)."""
    text = re.sub(r"<!--(?!itemEffects)[^>]*?-->", "", tooltip)
    effects = re.search(r"<!--itemEffects:(\d+)-->", tooltip)
    if effects and int(effects.group(1)) > 0:
        raise Unreadable("has item effects")
    text = re.sub(r'<div class="whtt-sellprice">.*?</div>', "", text, flags=re.DOTALL)
    text = re.sub(r"<br\s*/?>|</td>|</tr>|</th>|</span>|</table>|<div[^>]*>|</div>", "\n", text)
    text = re.sub(r"Binds when (?:picked up|equipped)|<(?:table|tr|td|th)[^>]*>", "\n", text)
    text = html.unescape(re.sub(r"<[^>]+>", "", text))
    return [line.strip() for line in text.split("\n") if line.strip()]


def parse_tooltip(response):
    """What a tooltip response says about an item, or Unreadable."""
    if response.get("missing") or "tooltip" not in response:
        raise Unreadable("no tooltip")
    name = response["name"]
    out = {"name": name, "quality": response.get("quality"), "stats": {}}
    for line in tooltip_lines(response["tooltip"]):
        if line == name or line in SLOT_WORDS or line in TYPE_WORDS:
            continue
        if match := re.fullmatch(r"Item Level (\d+)", line):
            out["ilvl"] = int(match.group(1))
        elif match := re.fullmatch(r"(\d+) Armor", line):
            out["armor"] = int(match.group(1))
        elif match := re.fullmatch(r"(\d+) Block", line):
            out["block"] = int(match.group(1))
        elif match := re.fullmatch(r"(\d+)(?: - (\d+))? (?:[A-Z][a-z]+ )?Damage", line):
            out["damage"] = (int(match.group(1)), int(match.group(2) or match.group(1)))
        elif match := re.fullmatch(r"Speed (\d+\.\d+)", line):
            out["speed"] = float(match.group(1))
        elif re.fullmatch(r"\(\d+(\.\d+)? damage per second\)", line):
            pass
        elif re.fullmatch(r'"[^"]*"', line):
            pass
        elif line == "Unique":
            out["unique"] = True
        elif match := re.fullmatch(r"Classes: (.+)", line):
            out["classes"] = ["Class" + name.strip().replace(" ", "") for name in match.group(1).split(",")]
        elif re.fullmatch(r"Durability \d+ / \d+|Requires Level \d+", line):
            pass
        elif match := STAT_LINE.match(line):
            stat = STAT_INDEX.get(match.group(2))
            if stat is None:
                raise Unreadable(f"unread line {line!r}")
            out["stats"][stat] = out["stats"].get(stat, 0) + int(match.group(1))
        elif EFFECT_LINE.match(line):
            raise Unreadable("has item effects")
        else:
            raise Unreadable(f"unread line {line!r}")
    if "ilvl" not in out:
        raise Unreadable("no item level")
    return out


def row_for(item_id, sparse, item, response):
    """The items.jsonl row for a candidate, or Unreadable."""
    parsed = parse_tooltip(response)
    inventory = int(sparse["InventoryType"])
    subclass = int(item["SubclassID"])
    if parsed["ilvl"] != int(sparse["ItemLevel"]):
        raise Unreadable(f"item level {parsed['ilvl']} differs from ItemLevel {sparse['ItemLevel']}")
    row = {"id": item_id, "name": sparse["Display_lang"]}
    damage_expected = False
    if int(item["ClassID"]) == ARMOR_CLASS and subclass == 6:
        if inventory != 14:
            raise Unreadable(f"shield in slot {inventory}")
        row.update(type="ItemTypeWeapon", weapon_type="WeaponTypeShield", hand_type="HandTypeOffHand")
    elif int(item["ClassID"]) == ARMOR_CLASS and inventory == 23:
        row.update(type="ItemTypeWeapon", weapon_type="WeaponTypeOffHand", hand_type="HandTypeOffHand")
    elif int(item["ClassID"]) == ARMOR_CLASS:
        if inventory not in ITEM_TYPE:
            raise Unreadable(f"armor in slot {inventory}")
        row["type"] = ITEM_TYPE[inventory]
        if subclass in ARMOR_TYPE:
            row["armor_type"] = ARMOR_TYPE[subclass]
        elif subclass != 0:
            raise Unreadable(f"armor subclass {subclass}")
    elif inventory in (15, 25, 26):
        if subclass not in RANGED_TYPE:
            raise Unreadable(f"ranged subclass {subclass}")
        row.update(type="ItemTypeRanged", ranged_weapon_type=RANGED_TYPE[subclass])
        damage_expected = True
    else:
        if subclass not in WEAPON_TYPE or inventory not in HAND_TYPE:
            raise Unreadable(f"weapon subclass {subclass} in slot {inventory}")
        row.update(
            type="ItemTypeWeapon",
            weapon_type=WEAPON_TYPE[subclass],
            hand_type=HAND_TYPE[inventory],
        )
        damage_expected = True
    option = {}
    if damage_expected:
        if "damage" not in parsed or "speed" not in parsed:
            raise Unreadable("weapon without damage or speed")
        delay = int(sparse["ItemDelay"]) / 1000
        if abs(delay - parsed["speed"]) > 0.006:
            raise Unreadable(f"speed {parsed['speed']} differs from ItemDelay {delay}")
        row["weapon_speed"] = parsed["speed"]
        option["weapon_damage_min"], option["weapon_damage_max"] = parsed["damage"]
    elif "damage" in parsed or "speed" in parsed:
        raise Unreadable("damage on an item that is not a weapon")
    stats = dict(parsed["stats"])
    if parsed.get("armor"):
        stats[STAT_ARMOR] = parsed["armor"]
    if parsed.get("block"):
        stats[STAT_BLOCK_VALUE] = parsed["block"]
    if stats:
        option["stats"] = {str(key): stats[key] for key in sorted(stats)}
    option["ilvl"] = parsed["ilvl"]
    row["scaling_options"] = {"0": option}
    if parsed.get("unique"):
        row["unique"] = True
    if parsed.get("classes"):
        row["class_allowlist"] = parsed["classes"]
    return row


def plain_numbers(value):
    """A value with whole floats as integers, as items.jsonl writes them."""
    if isinstance(value, float) and value == int(value):
        return int(value)
    if isinstance(value, dict):
        return {key: plain_numbers(item) for key, item in value.items()}
    if isinstance(value, list):
        return [plain_numbers(item) for item in value]
    return value


def dump_row(row):
    return json.dumps(plain_numbers(row), separators=(",", ":"), ensure_ascii=False)


def build(tables, cache, report=None):
    """Write OUT from the cache; returns (rows, left_out) with left_out as {id: (name, reason)}."""
    taken, _ = candidates(tables)
    rows, left_out = [], {}
    for item_id, sparse, item in taken:
        path = cache_path(cache, item_id)
        if not path.exists():
            left_out[item_id] = (sparse["Display_lang"], "tooltip not fetched")
            continue
        try:
            rows.append(row_for(item_id, sparse, item, json.loads(path.read_text())))
        except Unreadable as err:
            left_out[item_id] = (sparse["Display_lang"], str(err))
    OUT.write_text("".join(dump_row(row) + "\n" for row in rows))
    if report:
        Path(report).write_text(
            "".join(f"{i}\t{n}\t{r}\n" for i, (n, r) in sorted(left_out.items()))
        )
    return rows, left_out


ROW_KEYS = {
    "id", "name", "type", "armor_type", "weapon_type", "hand_type", "ranged_weapon_type",
    "weapon_speed", "scaling_options", "unique", "class_allowlist",
}  # fmt: skip


def check():
    """Offline audit of OUT: sorted unique ids, none that items.jsonl holds, only the keys the
    generator writes, and every line exactly as dump_row writes it."""
    known = known_ids()
    previous = 0
    count = 0
    for number, line in enumerate(OUT.read_text().splitlines(), 1):
        row = json.loads(line)
        problems = []
        if not line.startswith('{"id":'):
            problems.append("does not start with its id")
        if row["id"] <= previous:
            problems.append("is not in increasing id order")
        if row["id"] in known:
            problems.append("is an id items.jsonl holds")
        if set(row) - ROW_KEYS:
            problems.append(f"has keys {sorted(set(row) - ROW_KEYS)}")
        if dump_row(row) != line:
            problems.append("is not written as dump_row writes it")
        if problems:
            raise ValueError(f"{OUT.name} line {number} (item {row['id']}): " + ", ".join(problems))
        previous = row["id"]
        count += 1
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("command", choices=["fetch", "build", "check"])
    parser.add_argument("--tables")
    parser.add_argument("--cache")
    parser.add_argument("--report")
    args = parser.parse_args()
    if args.command == "fetch":
        fetch(args.tables, args.cache)
    elif args.command == "build":
        rows, left_out = build(args.tables, args.cache, args.report)
        print(f"{len(rows)} rows, {len(left_out)} left out")
    else:
        print(f"{check()} rows ok")


if __name__ == "__main__":
    main()
