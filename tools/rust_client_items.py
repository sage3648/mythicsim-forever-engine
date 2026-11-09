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
a real name (no deprecated, test, old or placeholder rows) and a tooltip whose every line this
tool reads. The equip lines that are a plain stat (Increased Defense, Attack Power, spell damage
and healing, hit, crit, mana per 5 sec, dodge, parry, block, expertise, weapon damage;
EQUIP_STATS), a weapon skill (pseudo_stats) or all resistances are read as data/items.jsonl holds
the same line of a Classic item. A line that changes nothing in a fight (mount speed, swimming, a
profession, quest text, a use that only controls an enemy; NO_FIGHT_EFFECT) is left out of the
row. An item whose tooltip is missing, or has any other use, chance on hit, set or equip line, or
any line this tool does not read, is left out and listed.

Most of the items the Forever client adds (ids from about 270000) have an Item row but no
readable ItemSparse row: the client ships them encrypted and wago.tools cannot read them, while
Wowhead's Forever tooltips decode them. Such an item is taken from its tooltip alone: its name,
item level, damage and speed from the tooltip, which must name the Item row's slot, and its
class, subclass and slot from the Item row.

    python3 tools/rust_client_items.py fetch  --tables DIR --cache DIR
    python3 tools/rust_client_items.py build  --tables DIR --cache DIR [--report FILE]
    python3 tools/rust_client_items.py check

An equip line "+N Attack Power against <creature type>" is a creature type bonus the item row
cannot state. The item is taken, the row carries its other stats, and the bonus goes to
data/forever-client-item-bonuses.jsonl ({"id":..,"name":..,"melee_attack_power_versus":
{"MobTypeUndead":33}}), which Forever's preparation reads (src/prepare/forever_items.rs). A line
"Increases damage done to <creature type> by magical spells and effects by up to N" goes there as
spell_damage_versus, as Mark of the Champion's spell damage against Undead and Demons.

    python3 tools/rust_client_items.py add    --tables DIR --cache DIR ID [ID ...]

add fetches the tooltips of the given candidates only, writes their rows into the file in id
order and leaves every other row as it is, for an item that a run of build over the whole
candidate list (hours of tooltips) would add beside the rest.

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
BONUSES = DATA / "forever-client-item-bonuses.jsonl"

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
    "Spell Penetration": 15,
    "Spirit": 16,
    "Block Value": 27,
    # A green +N Armor line is bonus armor, as the reference rows hold it (Ardent Custodian).
    "Armor": 31,
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
    the rows left out for their name, by id. sparse is None for an Item row the ItemSparse table
    has no readable row of (the live client's encrypted rows of the Forever items); its name is
    known only from its tooltip, which build checks the same way."""
    sparse = {int(row["ID"]): row for row in read_csv(Path(tables) / "ItemSparse.csv")}
    classes = {int(row["ID"]): row for row in read_csv(Path(tables) / "Item.csv")}
    known = known_ids()
    taken, junk = [], {}
    for item_id in sorted(classes):
        row = sparse.get(item_id)
        item = classes[item_id]
        if item_id in known:
            continue
        if inventory_type(row, item) in NOT_WORN:
            continue
        if int(item["ClassID"]) not in (WEAPON_CLASS, ARMOR_CLASS):
            continue
        if row is not None and JUNK_NAME.search(row["Display_lang"]):
            junk[item_id] = row["Display_lang"]
            continue
        taken.append((item_id, row, item))
    return taken, junk


def inventory_type(sparse, item):
    """The slot of an item: its ItemSparse row's, or its Item row's when it has no ItemSparse row."""
    return int((sparse or item)["InventoryType"])


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
    # The ItemSparse rows first: they are the rows an earlier run of this tool read.
    taken.sort(key=lambda candidate: (candidate[1] is None, candidate[0]))
    wanted = [item_id for item_id, _, _ in taken if not cache_path(cache, item_id).exists()]
    print(f"{len(taken)} candidates, {len(wanted)} tooltips to fetch", file=sys.stderr)
    delay = REQUEST_INTERVAL
    index = 0
    while index < len(wanted):
        item_id = wanted[index]
        # Requests start at most one per delay, a 404 included.
        started = time.monotonic()
        try:
            text = fetch_tooltip(item_id)
            json.loads(text)
        except urllib.error.HTTPError as err:
            if err.code in (403, 429):
                delay = min(delay * 2, 60)
                print(f"HTTP {err.code} at {item_id}, waiting {delay:.0f}s", file=sys.stderr)
                time.sleep(delay * 4)
                continue
            if err.code != 404:
                raise
            text = json.dumps({"missing": True})
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as err:
            print(f"{item_id}: {err}, retrying", file=sys.stderr)
            time.sleep(5)
            continue
        cache_path(cache, item_id).write_text(text)
        index += 1
        delay = max(REQUEST_INTERVAL, delay * 0.9)
        time.sleep(max(0.0, delay - (time.monotonic() - started)))
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
EFFECT_LINE = re.compile(r"^(Equip|Use|Chance on hit|Set):", re.IGNORECASE)
STAT_LINE = re.compile(r"^([+-])(\d+) ([A-Za-z ]+)$")
# The green regeneration lines of the live client: mana per 5 sec is MP5 (Mindthrust Bracers,
# Restores +3 mana per 5 sec: 3); health per 5 sec has no stat and the reference rows leave it
# out (Death Speaker Scepter, Restores +12 health per 5 sec).
REGEN_LINE = re.compile(r"^Restores \+(\d+) (mana|health) per 5 sec\.?$")
SET_LINE = re.compile(r"^.+ \(\d+/\d+\)$")

# Rating per percent, as src/prepare/character.rs reads the rating stats back.
HIT_RATING, CRIT_RATING, DODGE_RATING, PARRY_RATING, BLOCK_RATING = 10, 14, 12, 15, 5
SCHOOL_DAMAGE = {"Arcane": 6, "Fire": 7, "Frost": 8, "Holy": 9, "Nature": 10, "Shadow": 11}
NUMBER = r"(\d+(?:\.\d+)?)"

# The equip lines that are a plain stat, as data/items.jsonl holds the same line of a Classic
# item: (pattern, [(stat, group, rating per point)]). Increased Defense +6 is 6 Defense rating
# (Ardent Custodian), +20 Attack Power is 20 melee and 20 ranged Attack Power, as the equip
# spells of that line state auras 99 and 124 at the same value (Attack Power 22, 9332; Mark of
# Kern), 1.5% hit is 15 melee hit rating (Staff of Jordan), 1% crit 14 (Stronghold
# Gauntlets), spell damage and healing both spell power stats (Runed Ring), healing and damage
# the two figures (Naga Battle Gloves), 3 mana per 5 sec 3 MP5 (Mindthrust Bracers).
EQUIP_STATS = [
    (rf"Increased Defense \+{NUMBER}", [(25, 1, 1)]),
    (rf"\+{NUMBER} Attack Power", [(17, 1, 1), (18, 1, 1)]),
    (rf"\+{NUMBER} ranged Attack Power", [(18, 1, 1)]),
    (
        rf"\+{NUMBER} Attack Power in Cat, Bear, Dire Bear,? and Moonkin forms only",
        [(19, 1, 1)],
    ),
    (
        rf"Increases damage and healing done by magical spells and effects by up to {NUMBER}",
        [(4, 1, 1), (5, 1, 1)],
    ),
    (rf"Increases healing done by spells and effects by up to {NUMBER}", [(4, 1, 1)]),
    # Damage alone is spell damage (Moss Cinch, 12: 12), healing alone healing power (Death
    # Speaker Scepter, 68: 68).
    (rf"Increases damage done by magical spells and effects by up to {NUMBER}", [(5, 1, 1)]),
    (rf"Increases healing done by magical spells and effects by up to {NUMBER}", [(4, 1, 1)]),
    (
        rf"Increases healing done by up to {NUMBER} and damage done by up to {NUMBER} for all "
        r"magical spells and effects",
        [(4, 1, 1), (5, 2, 1)],
    ),
] + [
    (rf"Increases damage done by {school} spells and effects by up to {NUMBER}", [(stat, 1, 1)])
    for school, stat in SCHOOL_DAMAGE.items()
] + [
    (rf"Improves your chance to hit by {NUMBER}%", [(20, 1, HIT_RATING)]),
    (rf"Improves your chance to get a critical strike by {NUMBER}%", [(21, 1, CRIT_RATING)]),
    (rf"Improves your chance to hit with spells by {NUMBER}%", [(12, 1, HIT_RATING)]),
    (
        rf"Improves your chance to get a critical strike with spells by {NUMBER}%",
        [(13, 1, CRIT_RATING)],
    ),
    (rf"Restores {NUMBER} mana per 5 sec", [(34, 1, 1)]),
    (rf"Increases your chance to Dodge an attack by {NUMBER}%", [(28, 1, DODGE_RATING)]),
    (rf"Increases your chance to Parry an attack by {NUMBER}%", [(29, 1, PARRY_RATING)]),
    (
        rf"Increases your chance to Block attacks with a shield by {NUMBER}%",
        [(26, 1, BLOCK_RATING)],
    ),
    (rf"Increases the block value of your shield by {NUMBER}", [(STAT_BLOCK_VALUE, 1, 1)]),
    # Expertise, 10 rating a percent: Servomechanic Sledgehammer's "Reduces chance to be Dodged or
    # Parried by 1.0%" is 10 ExpertiseRating, Dwarven Tree Chopper's 0.6% is 6.
    (rf"Reduces chance to be Dodged or Parried by {NUMBER}%", [(24, 1, 10)]),
    # Bonus weapon damage: Might of Cenarius's "+4 Weapon Damage" is 4 PhysicalDamage.
    (rf"\+{NUMBER} Weapon Damage", [(40, 1, 1)]),
]

# The weapon skill lines, by their proto.PseudoStat index from PseudoStatAxesSkill (28), as the
# reference rows hold them: Servomechanic Sledgehammer's "Increased Two-Handed Maces +1" is 1 at
# 35, Edgemaster's Handguards' Axes, Swords and Daggers are 28, 29 and 31. Fist Weapons is the
# Unarmed skill (src/prepare/weapon_skill.rs). The proto has 42 pseudo stats.
WEAPON_SKILLS = {
    name: 28 + offset
    for offset, name in enumerate([
        "Axes", "Swords", "Maces", "Daggers", "Fist Weapons", "Two-Handed Axes",
        "Two-Handed Swords", "Two-Handed Maces", "Polearms", "Staves", "Bows", "Guns",
        "Crossbows", "Thrown",
    ])
}  # fmt: skip
PSEUDO_STATS_LEN = 42
SKILL_LINE = re.compile(rf"Equip: Increased ({'|'.join(WEAPON_SKILLS)}) \+(\d+)\.?")
# "+7 to All Resistances" is each of the five resistances, as the reference rows hold a Classic
# item's "+N All Resistances" (Chromatic Sword).
ALL_RESISTANCES_LINE = re.compile(r"\+(\d+) to All Resistances")
RESISTANCES = [35, 36, 37, 38, 39]

# Lines that change nothing a sim of a fight reads, so the item is taken with the line left out:
# travel, mount and swim speed, a zone the fight is never in, falling, stealth detection, a
# gathering or crafting profession, quest text, and uses and procs that only control or dispel
# an enemy, or only raise the damage the wearer takes. Each names the items it was written for.
NO_FIGHT_EFFECT = [
    re.compile(pattern)
    for pattern in [
        # Medal of Valor 270095.
        r"Equip: Reduces the hit chance of Fear effects against you by \d+%\.",
        # The Desert Rose 274648, Xorothian Footwraps 282631, Outrider's Gloves 287010.
        r"Equip: Increases mount speed by \d+% in [A-Za-z ]+ areas\.",
        r"Equip: Increases movement speed while mounted by \d+%\. This effect does not stack "
        r"with similar effects\.",
        # Guidance of Ursoc 279891, Wisdom of Ursol 279892.
        r"Equip: Movement speed increased by \d+% in [A-Za-z ]+ areas\.",
        r"Equip: Restores \d+ Mana per 5 sec in [A-Za-z ]+ areas\.",
        # Gill's Gills 286646, Deep Flora Bindings 286998.
        r"Equip: Underwater Breath lasts \d+% longer than normal\.",
        # Grimm's Packing Gloves 276395, Enchantresses' Spire 287025.
        r"Equip: Increased (?:Herbalism|Mining|Skinning|Fishing) \+\d+\.",
        r"Equip: Reduces the cast time of Disenchant by \d+%\.",
        # Cliff Jumpers 284040, Brewmancer Nightcap 276877.
        r"Equip: Reduces damage from falling\.",
        r"Equip: Increases your effective stealth detection level by \d+\.",
        # Wicked Ritualist's Kris 273832; Razza's Heart, Skarr's Broken Gauntlet, Mushgog's Crown
        # of Thorns 277556 to 277558.
        r"Quest Item",
        r"This Item Begins a Quest",
        # Theka's Last Breath 274641, Emblem of Slamming 275009, Sand-Covered Silk Sash 286612,
        # Scalding Flamehoof 280074, Shen'dralar Relic 277559.
        r"Use: Transform all enemies within \d+ yds into scarabs, .*",
        r"Use: Stuns target and yourself for \d+ sec\. .*",
        r"Use: Removes existing Immobilizing effects .*",
        r"Use: Increases Fire damage taken by \d+% and movement speed by \d+% for \d+ sec\. .*",
        r"Use: Dispel \d+ beneficial magic effect .*",
        # Ankle Slicers 284587, Earthshaker 288900.
        r"Chance on hit: Slice the target's ankle, slowing movement speed by \d+% for \d+ sec\.",
        r"Chance on hit: Knocks down all nearby enemies for \d+ sec\.",
    ]
]
# A clause that doubles an equip line in a city or stronghold, where no fight is (Black Horn
# Necklace 279863): the line reads as its base amount.
CITY_CLAUSE = re.compile(r" This effect is doubled in Strongholds and Cities\.$")


# The creature types an equip line names, by the request's MobType enum (proto.MobType).
CREATURE_TYPES = {
    "Beasts": "MobTypeBeast",
    "Demons": "MobTypeDemon",
    "Dragonkin": "MobTypeDragonkin",
    "Elementals": "MobTypeElemental",
    "Giants": "MobTypeGiant",
    "Humanoids": "MobTypeHumanoid",
    "Mechanicals": "MobTypeMechanical",
    "Undead": "MobTypeUndead",
}
# "Equip: +33 Attack Power against Undead." An attack power bonus against a creature type: melee
# attack power only (A_MOD_MELEE_ATTACK_POWER_VERSUS, as Quel'Serrar's Sanctuary and the Tier 1
# bonuses are), applied while the target is of the type.
VERSUS_LINE = re.compile(
    rf"Equip: \+{NUMBER} Attack Power against ({'|'.join(CREATURE_TYPES)})\.?", re.IGNORECASE
)
# "Equip: Increases damage done to Humanoids by magical spells and effects by up to 10." Spell
# damage against a creature type, as Mark of the Champion's against Undead and Demons, applied
# while the target is of the type.
VERSUS_SPELL_LINE = re.compile(
    rf"Equip: Increases damage done to ({'|'.join(CREATURE_TYPES)}) by magical spells and effects "
    rf"by up to {NUMBER}\.?",
    re.IGNORECASE,
)


def equip_stats(line):
    """The stats of an "Equip:" line that is a plain stat, or None."""
    text = re.sub(r"^Equip:\s*", "", line).rstrip(".").strip()
    for pattern, stats in EQUIP_STATS:
        if match := re.fullmatch(pattern, text, re.IGNORECASE):
            out = {}
            for stat, group, per_point in stats:
                value = float(match.group(group)) * per_point
                if abs(value - round(value)) > 1e-6:
                    raise Unreadable(f"fractional rating in {line!r}")
                out[stat] = out.get(stat, 0) + round(value)
            return out
    return None


def tooltip_lines(tooltip):
    """The visible text of a tooltip, one entry per line, with the data comments kept as text
    for the numbers this tool reads (<!--ilvl-->15 reads as the number alone), and the number
    of item effects the tooltip says the item has."""
    effects = re.search(r"<!--itemEffects:(\d+)-->", tooltip)
    # A rating inside an effect line sits in its own span: <span><!--rtg41-->60</span>.
    text = re.sub(r"<span>(?:<!--[^>]*?-->)?([\d.]+)</span>", r"\1", tooltip)
    text = re.sub(r"<!--[^>]*?-->", "", text)
    text = re.sub(r'<div class="whtt-sellprice">.*?</div>', "", text, flags=re.DOTALL)
    # Wowhead's notes on where the item drops ("Dropped by: Zul'Alai", "Drop Chance: 66.67%",
    # added to the tooltips during 8 to 9 November 2026) say nothing about the item.
    text = re.sub(
        r'<div class="whtt-extra whtt-(?:droppedby|dropchance)">.*?</div>', "", text, flags=re.DOTALL
    )
    text = re.sub(r"<br\s*/?>|</td>|</tr>|</th>|</span>|</table>|<div[^>]*>|</div>", "\n", text)
    text = re.sub(r"Binds when (?:picked up|equipped)|<(?:table|tr|td|th)[^>]*>", "\n", text)
    text = html.unescape(re.sub(r"<[^>]+>", "", text))
    lines = [line.strip() for line in text.split("\n") if line.strip()]
    return lines, int(effects.group(1)) if effects else 0


def parse_tooltip(response):
    """What a tooltip response says about an item, or Unreadable."""
    if response.get("missing") or "tooltip" not in response:
        raise Unreadable("no tooltip")
    name = response["name"]
    out = {"name": name, "quality": response.get("quality"), "stats": {}}
    lines, effects = tooltip_lines(response["tooltip"])
    equip_lines = 0
    for line in lines:
        line = CITY_CLAUSE.sub("", line)
        if line in SLOT_WORDS and "slot" not in out:
            out["slot"] = line
            continue
        if line == name or line in TYPE_WORDS:
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
        elif line in ("Unique", "Unique-Equipped"):
            # Both are the row's unique flag (Emergency Field Cloak is Unique-Equipped).
            out["unique"] = True
        elif match := re.fullmatch(r"Classes: (.+)", line):
            out["classes"] = ["Class" + name.strip().replace(" ", "") for name in match.group(1).split(",")]
        elif re.fullmatch(r"Durability \d+ / \d+|Requires Level \d+", line):
            pass
        elif match := ALL_RESISTANCES_LINE.fullmatch(line):
            for stat in RESISTANCES:
                out["stats"][stat] = out["stats"].get(stat, 0) + int(match.group(1))
        elif match := STAT_LINE.match(line):
            stat = STAT_INDEX.get(match.group(3))
            if stat is None:
                raise Unreadable(f"unread line {line!r}")
            value = int(match.group(2)) * (-1 if match.group(1) == "-" else 1)
            out["stats"][stat] = out["stats"].get(stat, 0) + value
        elif match := REGEN_LINE.match(line):
            if match.group(2) == "mana":
                out["stats"][34] = out["stats"].get(34, 0) + int(match.group(1))
        elif SET_LINE.match(line):
            raise Unreadable("a set piece")
        elif any(pattern.fullmatch(line) for pattern in NO_FIGHT_EFFECT):
            equip_lines += 1
        elif match := SKILL_LINE.fullmatch(line):
            equip_lines += 1
            pseudo = out.setdefault("pseudo", {})
            index = WEAPON_SKILLS[match.group(1)]
            pseudo[index] = pseudo.get(index, 0) + int(match.group(2))
        elif match := VERSUS_SPELL_LINE.fullmatch(line):
            equip_lines += 1
            mob_type = CREATURE_TYPES[match.group(1).capitalize()]
            value = float(match.group(2))
            if value != round(value):
                raise Unreadable(f"fractional spell damage in {line!r}")
            versus = out.setdefault("versus_spell", {})
            versus[mob_type] = versus.get(mob_type, 0) + round(value)
        elif match := VERSUS_LINE.fullmatch(line):
            equip_lines += 1
            mob_type = CREATURE_TYPES[match.group(2).capitalize()]
            value = float(match.group(1))
            if value != round(value):
                raise Unreadable(f"fractional attack power in {line!r}")
            versus = out.setdefault("versus", {})
            versus[mob_type] = versus.get(mob_type, 0) + round(value)
        elif line.startswith("Equip:") and (stats := equip_stats(line)) is not None:
            equip_lines += 1
            for stat, value in stats.items():
                out["stats"][stat] = out["stats"].get(stat, 0) + value
        elif EFFECT_LINE.match(line):
            raise Unreadable(f"has an effect: {line!r}")
        elif line.startswith("Added in Season of Discovery"):
            raise Unreadable("a Season of Discovery item")
        else:
            raise Unreadable(f"unread line {line!r}")
    if effects and not equip_lines:
        raise Unreadable("has item effects the tooltip does not state")
    if "ilvl" not in out:
        raise Unreadable("no item level")
    return out


# The tooltip's slot word of each InventoryType this tool writes.
SLOT_WORD = {
    1: "Head", 2: "Neck", 3: "Shoulder", 5: "Chest", 6: "Waist", 7: "Legs", 8: "Feet",
    9: "Wrist", 10: "Hands", 11: "Finger", 12: "Trinket", 13: "One-Hand", 14: "Off Hand",
    15: "Ranged", 16: "Back", 17: "Two-Hand", 20: "Chest", 21: "Main Hand", 22: "Off Hand",
    23: "Held In Off-hand", 25: "Thrown", 26: "Ranged",
}  # fmt: skip


def row_for(item_id, sparse, item, response):
    """The items.jsonl row for a candidate, or Unreadable."""
    parsed = parse_tooltip(response)
    inventory = inventory_type(sparse, item)
    subclass = int(item["SubclassID"])
    if sparse is not None:
        if parsed["ilvl"] != int(sparse["ItemLevel"]):
            raise Unreadable(f"item level {parsed['ilvl']} differs from ItemLevel {sparse['ItemLevel']}")
        name = sparse["Display_lang"]
    else:
        # No ItemSparse row to check the tooltip against: its name passes the same filter, and
        # its slot is the Item row's.
        name = parsed["name"]
        if JUNK_NAME.search(name):
            raise Unreadable("development name")
        if parsed.get("slot") != SLOT_WORD.get(inventory):
            raise Unreadable(f"slot {parsed.get('slot')!r} differs from InventoryType {inventory}")
    row = {"id": item_id, "name": name}
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
        if sparse is not None:
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
    if parsed.get("pseudo"):
        pseudo = [0] * PSEUDO_STATS_LEN
        for index, value in parsed["pseudo"].items():
            pseudo[index] = value
        row["pseudo_stats"] = pseudo
    if parsed.get("classes"):
        row["class_allowlist"] = parsed["classes"]
    return row


def bonus_row(item_id, name, parsed):
    """The data/forever-client-item-bonuses.jsonl row of an item whose tooltip states attack power
    or spell damage against creature types, or None."""
    row = {"id": item_id, "name": name}
    for key, field in (("versus", "melee_attack_power_versus"), ("versus_spell", "spell_damage_versus")):
        if parsed.get(key):
            row[field] = dict(sorted(parsed[key].items()))
    return row if len(row) > 2 else None


def reason_kind(reason):
    """A left out item's reason without the line or numbers it names, to count them by."""
    return re.sub(r"\d+", "N", reason.split(": '")[0].split(' \'')[0].split(' "')[0])


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
    rows, bonuses, left_out = [], [], {}
    for item_id, sparse, item in taken:
        path = cache_path(cache, item_id)
        listed = sparse["Display_lang"] if sparse is not None else ""
        if not path.exists():
            left_out[item_id] = (listed, "tooltip not fetched")
            continue
        response = json.loads(path.read_text())
        try:
            row = row_for(item_id, sparse, item, response)
        except Unreadable as err:
            left_out[item_id] = (listed or response.get("name", ""), str(err))
            continue
        rows.append(row)
        if bonus := bonus_row(item_id, row["name"], parse_tooltip(response)):
            bonuses.append(bonus)
    OUT.write_text("".join(dump_row(row) + "\n" for row in rows))
    BONUSES.write_text("".join(dump_row(row) + "\n" for row in bonuses))
    if report:
        Path(report).write_text(
            "".join(f"{i}\t{n}\t{r}\n" for i, (n, r) in sorted(left_out.items()))
        )
    return rows, left_out


def add(tables, cache, ids):
    """Fetch the given candidates' tooltips and merge their rows into OUT and BONUSES, in id
    order, leaving every other row as it is. Returns (added rows, left out by id)."""
    Path(cache).mkdir(parents=True, exist_ok=True)
    taken, _ = candidates(tables)
    wanted = {candidate[0]: candidate for candidate in taken if candidate[0] in ids}
    missing = sorted(set(ids) - set(wanted))
    if missing:
        raise SystemExit(f"not candidates (known, not worn, or not weapon or armor): {missing}")
    for item_id in sorted(wanted):
        if not cache_path(cache, item_id).exists():
            cache_path(cache, item_id).write_text(fetch_tooltip(item_id))
            time.sleep(1.0)
    rows, bonuses, left_out = {}, {}, {}
    for item_id, (_, sparse, item) in sorted(wanted.items()):
        response = json.loads(cache_path(cache, item_id).read_text())
        try:
            row = row_for(item_id, sparse, item, response)
        except Unreadable as err:
            left_out[item_id] = str(err)
            continue
        rows[item_id] = row
        if bonus := bonus_row(item_id, row["name"], parse_tooltip(response)):
            bonuses[item_id] = bonus
    for path, added in ((OUT, rows), (BONUSES, bonuses)):
        kept = {}
        if path.exists():
            kept = {json.loads(line)["id"]: json.loads(line) for line in path.read_text().splitlines()}
        kept.update(added)
        path.write_text("".join(dump_row(kept[key]) + "\n" for key in sorted(kept)))
    return rows, left_out


ROW_KEYS = {
    "id", "name", "type", "armor_type", "weapon_type", "hand_type", "ranged_weapon_type",
    "weapon_speed", "scaling_options", "unique", "pseudo_stats", "class_allowlist",
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
    ids = set()
    for line in OUT.read_text().splitlines():
        ids.add(json.loads(line)["id"])
    previous = 0
    for number, line in enumerate(BONUSES.read_text().splitlines(), 1):
        row = json.loads(line)
        problems = []
        if row["id"] <= previous:
            problems.append("is not in increasing id order")
        if row["id"] not in ids:
            problems.append(f"is not an item {OUT.name} holds")
        fields = set(row) - {"id", "name"}
        if not fields or not fields <= {"melee_attack_power_versus", "spell_damage_versus"}:
            problems.append("does not have id, name and melee_attack_power_versus or spell_damage_versus")
        elif any(
            not row[field] or not set(row[field]) <= set(CREATURE_TYPES.values()) for field in fields
        ):
            problems.append("names a creature type the tool does not write")
        if dump_row(row) != line:
            problems.append("is not written as dump_row writes it")
        if problems:
            raise ValueError(f"{BONUSES.name} line {number} (item {row['id']}): " + ", ".join(problems))
        previous = row["id"]
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("command", choices=["fetch", "build", "check", "add"])
    parser.add_argument("ids", nargs="*", type=int, help="the item ids add takes")
    parser.add_argument("--tables")
    parser.add_argument("--cache")
    parser.add_argument("--report")
    args = parser.parse_args()
    if args.command == "fetch":
        fetch(args.tables, args.cache)
    elif args.command == "add":
        rows, left_out = add(args.tables, args.cache, set(args.ids))
        print(f"{len(rows)} rows added, {len(left_out)} left out")
        for item_id, reason in sorted(left_out.items()):
            print(f"{item_id}\t{reason}")
    elif args.command == "build":
        rows, left_out = build(args.tables, args.cache, args.report)
        print(f"{len(rows)} rows, {len(left_out)} left out")
        kinds = {}
        for _, reason in left_out.values():
            kind = reason_kind(reason)
            kinds[kind] = kinds.get(kind, 0) + 1
        for kind, count in sorted(kinds.items(), key=lambda entry: -entry[1]):
            print(f"{count:6}  {kind}")
    else:
        print(f"{check()} rows ok")


if __name__ == "__main__":
    main()
