#!/usr/bin/env python3
"""Add data/forever-live-items.jsonl rows for the reference items whose live Wowhead Forever
tooltip states other base stats, weapon damage or speed than their data/items.jsonl row.

Under Forever's rules preparation reads a live row before the reference row of the same id
(src/prepare/items.rs). Each row this tool writes is the reference row with only what the tooltip
states otherwise changed, so its name, kind, effects and set stay the reference row's.

    https://nether.wowhead.com/forever/tooltip/item/<id>?x=<random>

Wowhead's Forever tooltip decodes the live client's item, the encrypted rows included, and
answers 404 for an item whose Forever stats Blizzard has not revealed (such an item keeps its
reference row). The rules, per stat of the row's base stats (scaling_options "0"):

- a stat the tooltip states (a "+N Stat" line, the armor and block lines, an equip line that is a
  plain stat, read as tools/rust_client_items.py reads it) replaces the row's, unless the row
  carries that value as an item effect's stat instead;
- a stat the tooltip does not state is dropped only when it is a primary stat, a resistance,
  spell penetration, mana per 5 sec, defense or bonus armor, the row's effects do not carry it,
  and its name appears nowhere on the tooltip;
- a generic "+N Attack Power" equip line is melee and ranged: a row that holds the same ranged
  attack power as attack power takes N for both, and a row that holds attack power alone keeps
  it alone, since Forever's preparation gives an item's attack power to ranged as well
  (src/prepare/items.rs);
- Forever adds an item's melee and spell hit (and crit) together and gives the sum to both
  (src/prepare/items.rs unify_gear_hit_and_crit), so a tooltip that states one kind ("Improves your
  chance to get a critical strike by 1.0%") drops the row's other kind (Classic's spell crit):
  keeping both sims 2% where the item gives 1% (Tempest Talisman, Ban'thok Sash);
- hit and crit within one rating point of the tooltip's percentage are the same, and a rating
  the row carries as a percent pseudo stat (Eye of the Beast's 1% spell crit) is not added again;
- the weapon skill lines ("Increased Daggers +1") replace the row's weapon skill pseudo stats,
  and a skill the tooltip no longer states is dropped: Forever cut Edgemaster's Handguards from
  +7 Axes, Swords and Daggers to +1 each, and Mugger's Belt from +5 Daggers to +1;
- weapon damage and speed follow the tooltip, and so does the item level of a row that changes;
- the cooldown of the "Use:" line, "(5 Min Cooldown)", replaces the on use cooldown of a row
  with one item effect on use, when the tooltip has one such line (Adaptive Combat Assistant,
  272437: 5 minutes where the reference row says 90 seconds). The shared category cooldown
  stays the row's.

Existing live rows keep their stats, damage and speed as they are; only their on use cooldown
<<<<<<< HEAD
and weapon skills follow a tooltip the cache holds. So a row written from a hotfix push the tooltip does not show
yet stays: Rage of the Storm (280604) is the ItemSparse row of hotfix push 113886 (docs/mechanics.md). An item with a "+N" line this tool cannot read is left out
=======
follows a tooltip the cache holds. So a row written from a hotfix push the tooltip does not show
yet stays: Rage of the Storm (280604) is the ItemSparse row of hotfix push 113886 (docs/mechanics.md). An item with a "+N" line this tool cannot read is left out
>>>>>>> 842da72a5d (Shorten the README and move mechanics notes to docs/mechanics.md)
and listed.

    python3 tools/rust_live_items.py fetch --cache DIR [ID ...]
    python3 tools/rust_live_items.py build --cache DIR [--report FILE]
    python3 tools/rust_live_items.py check

fetch asks for every reference weapon and armor tooltip the cache lacks (the cache format is
tools/rust_client_items.py's, so one cache serves both), or for the given ids alone, live rows
included; build and check run offline.
"""

import argparse
import json
from pathlib import Path
import re
import sys
import time
import urllib.error

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rust_client_items as client  # noqa: E402

DATA = client.DATA
ITEMS = client.ITEMS
LIVE = DATA / "forever-live-items.jsonl"
SKILL_INDICES = set(client.WEAPON_SKILLS.values())

STAT_ARMOR, STAT_BONUS_ARMOR, STAT_BLOCK_VALUE = 30, 31, 27
ATTACK_POWER, RANGED_ATTACK_POWER = 17, 18
MELEE_HIT, MELEE_CRIT = 20, 21
RESISTANCES = {"Arcane": 35, "Fire": 36, "Frost": 37, "Nature": 38, "Shadow": 39}
# Stats a tooltip shows by name: dropped from a row when the name is nowhere on the tooltip.
DROPPABLE = {
    0: "Strength", 1: "Agility", 2: "Stamina", 3: "Intellect", 16: "Spirit",
    15: "Spell Penetration", 34: "mana per 5", 25: "Defense", STAT_BONUS_ARMOR: "Armor",
    **{stat: f"{school} Resistance" for school, stat in RESISTANCES.items()},
}  # fmt: skip
# "+N Stat" lines the reference rows hold that client items do not carry.
EXTRA_STAT_LINES = {"Ranged Attack Power": RANGED_ATTACK_POWER}
# A rating stat the reference row may carry as a percent pseudo stat instead (Eye of the Beast's
# spell crit): rating index -> (pseudo stat index, rating per percent).
PSEUDO_PERCENT = {
    MELEE_HIT: (12, client.HIT_RATING),
    12: (13, client.HIT_RATING),
    MELEE_CRIT: (14, client.CRIT_RATING),
    13: (15, client.CRIT_RATING),
    28: (4, client.DODGE_RATING),
    29: (5, client.PARRY_RATING),
}
ALL_RESISTANCES = re.compile(r"^\+(\d+) (?:to )?All Resistances\.?$")
# The cooldown at the end of a "Use:" line: "(5 Min Cooldown)", "(30 Sec Cooldown)",
# "(1 Hour Cooldown)".
USE_COOLDOWN = re.compile(r"^Use:.*\((\d+) (Sec|Min|Hour|Hours) Cooldown\)$")
COOLDOWN_UNIT_MS = {"Sec": 1000, "Min": 60_000, "Hour": 3_600_000, "Hours": 3_600_000}
SET_BONUS = re.compile(r"^\(\d+\) Set ?:")


def reference_rows():
    with open(ITEMS) as rows:
        return {row["id"]: row for row in map(json.loads, rows)}


def live_rows():
    with open(LIVE) as rows:
        return {row["id"]: row for row in map(json.loads, rows)}


def is_gear(row):
    return row.get("type") not in (None, "ItemTypeUnknown")


def fetch(cache, ids=None):
    """Fetch every reference weapon and armor tooltip the cache lacks, or the given ids'."""
    Path(cache).mkdir(parents=True, exist_ok=True)
    live = live_rows()
    if ids:
        wanted = [item_id for item_id in ids if not client.cache_path(cache, item_id).exists()]
    else:
        wanted = [
            item_id
            for item_id, row in sorted(reference_rows().items())
            if is_gear(row) and item_id not in live and not client.cache_path(cache, item_id).exists()
        ]
    print(f"{len(wanted)} tooltips to fetch", file=sys.stderr)
    delay = client.REQUEST_INTERVAL
    index = 0
    while index < len(wanted):
        item_id = wanted[index]
        started = time.monotonic()
        try:
            text = client.fetch_tooltip(item_id)
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
        client.cache_path(cache, item_id).write_text(text)
        index += 1
        delay = max(client.REQUEST_INTERVAL, delay * 0.9)
        time.sleep(max(0.0, delay - (time.monotonic() - started)))
        if index % 250 == 0:
            print(f"{index}/{len(wanted)}", file=sys.stderr)


def read_tooltip(response):
    """What a live tooltip states: item level, base stats by index, weapon damage and speed, and
    its whole text; None for an item with no live tooltip. Raises client.Unreadable for a "+N"
    line it cannot read."""
    if response.get("missing") or response.get("error") or "tooltip" not in response:
        return None
    lines, _ = client.tooltip_lines(response["tooltip"])
    out = {"stats": {}, "text": " ".join(lines), "use_cooldowns": [], "skills": {}}

    def add(stat, value):
        out["stats"][stat] = out["stats"].get(stat, 0) + value

    for line in lines:
        if SET_BONUS.match(line) or client.SET_LINE.match(line):
            continue
        if match := re.fullmatch(r"Item Level (\d+)", line):
            out["ilvl"] = int(match.group(1))
        elif match := re.fullmatch(r"(\d+) Armor", line):
            add(STAT_ARMOR, int(match.group(1)))
        elif match := re.fullmatch(r"(\d+) Block", line):
            add(STAT_BLOCK_VALUE, int(match.group(1)))
        elif match := re.fullmatch(r"(\d+)(?: - (\d+))? (?:[A-Z][a-z]+ )?Damage", line):
            out["damage"] = (int(match.group(1)), int(match.group(2) or match.group(1)))
        elif match := re.fullmatch(r"Speed (\d+\.\d+)", line):
            out["speed"] = float(match.group(1))
        elif match := ALL_RESISTANCES.match(line):
            for stat in RESISTANCES.values():
                add(stat, int(match.group(1)))
        elif re.fullmatch(r"\+\d+ Holy Resistance", line):
            pass  # the engine has no holy resistance stat
        elif match := client.STAT_LINE.match(line):
            stat = EXTRA_STAT_LINES.get(match.group(3), client.STAT_INDEX.get(match.group(3)))
            if stat is None:
                raise client.Unreadable(f"unread line {line!r}")
            add(stat, int(match.group(2)) * (-1 if match.group(1) == "-" else 1))
        elif match := client.REGEN_LINE.match(line):
            if match.group(2) == "mana":
                add(34, int(match.group(1)))
        elif match := client.SKILL_LINE.fullmatch(line):
            index = client.WEAPON_SKILLS[match.group(1)]
            out["skills"][index] = out["skills"].get(index, 0) + int(match.group(2))
        elif line.startswith("Equip:"):
            try:
                stats = client.equip_stats(line)
            except client.Unreadable:
                stats = None
            stats = dict(stats or {})
            # A generic attack power line is read as the reference rows hold it, attack power
            # alone: preparation gives it to ranged too (changes() moves a row's equal ranged).
            if stats.get(ATTACK_POWER) and stats.get(RANGED_ATTACK_POWER) == stats[ATTACK_POWER]:
                del stats[RANGED_ATTACK_POWER]
            for stat, value in stats.items():
                add(stat, value)
        elif match := USE_COOLDOWN.match(line):
            out["use_cooldowns"].append(int(match.group(1)) * COOLDOWN_UNIT_MS[match.group(2)])
    return out


def on_use_effect(row):
    """The index of the row's one item effect on use, or None when it has none or several."""
    uses = [i for i, effect in enumerate(row.get("item_effects") or []) if effect.get("on_use")]
    return uses[0] if len(uses) == 1 else None


def cooldown_change(row, live):
    """The on use cooldown the tooltip states when it differs from the row's, else None."""
    index = on_use_effect(row)
    if index is None or len(live.get("use_cooldowns", [])) != 1:
        return None
    current = row["item_effects"][index]["on_use"].get("cooldown_ms", 0)
    stated = live["use_cooldowns"][0]
    return stated if current and stated != current else None


def skill_change(row, live):
    """The row's pseudo stats with the weapon skills the tooltip states, else None when they agree."""
    pseudo = list(row.get("pseudo_stats") or [])
    skills = live.get("skills", {})
    held = {i: v for i, v in enumerate(pseudo) if v and i in SKILL_INDICES}
    if held == skills:
        return None
    pseudo += [0] * (client.PSEUDO_STATS_LEN - len(pseudo))
    for index in SKILL_INDICES:
        pseudo[index] = skills.get(index, 0)
    return pseudo


# Hit and crit stat pairs Forever's preparation adds together: melee index, spell index.
UNIFIED_PAIRS = ((MELEE_HIT, 12), (MELEE_CRIT, 13))


def unified_change(row, live):
    """{stat: None} for each hit or crit stat the row holds whose twin the tooltip states alone."""
    base = {int(k): v for k, v in (row["scaling_options"]["0"].get("stats") or {}).items() if v}
    effects = effect_stats(row)
    out = {}
    for melee, spell in UNIFIED_PAIRS:
        for stated, other in ((melee, spell), (spell, melee)):
            if live["stats"].get(stated) and not live["stats"].get(other) and other in base and other not in effects:
                out[other] = None
    return out


def effect_stats(row):
    """The stats the row's item effects and area stats carry, by index."""
    out = {}
    option = row.get("scaling_options", {}).get("0", {})
    for effect in row.get("item_effects") or []:
        for stat, value in (effect.get("scaling_options", {}).get("0", {}).get("stats") or {}).items():
            out[int(stat)] = out.get(int(stat), 0) + value
    for area in option.get("area_stats") or []:
        for stat, value in area.get("stats", {}).items():
            out[int(stat)] = out.get(int(stat), 0) + value
    return out


def changes(row, live):
    """What the live tooltip states otherwise than the reference row: {stats: {index: value or
    None}, damage, speed, ilvl}, empty when they agree."""
    option = row.get("scaling_options", {}).get("0", {})
    base = {int(k): v for k, v in (option.get("stats") or {}).items() if v}
    effects = effect_stats(row)
    stated = dict(live["stats"])
    if (
        ATTACK_POWER in stated
        and RANGED_ATTACK_POWER not in stated
        and ATTACK_POWER in base
        and base.get(RANGED_ATTACK_POWER) == base[ATTACK_POWER]
    ):
        stated[RANGED_ATTACK_POWER] = stated[ATTACK_POWER]
    text = live["text"].lower()
    stats = {}
    for stat in sorted(set(base) | set(stated)):
        old, new = base.get(stat), stated.get(stat)
        if old == new:
            continue
        if stat in (MELEE_HIT, MELEE_CRIT) and old and new and abs(old - new) <= 1:
            continue
        if new is None:
            name = DROPPABLE.get(stat)
            if name and name.lower() not in text and stat not in effects:
                stats[stat] = None
            continue
        if old is None and effects.get(stat) == new:
            continue
        if old is None and stat in PSEUDO_PERCENT:
            index, per_percent = PSEUDO_PERCENT[stat]
            pseudo = row.get("pseudo_stats") or []
            if index < len(pseudo) and abs(pseudo[index] * per_percent - new) <= 1:
                continue
        stats[stat] = new
    stats.update(unified_change(row, live))
    out = {}
    if stats:
        out["stats"] = stats
    damage = (option.get("weapon_damage_min"), option.get("weapon_damage_max"))
    if live.get("damage") and tuple(live["damage"]) != damage and any(damage):
        out["damage"] = list(live["damage"])
    speed = row.get("weapon_speed")
    if speed and live.get("speed") and abs(speed - live["speed"]) > 0.001:
        out["speed"] = live["speed"]
    cooldown = cooldown_change(row, live)
    if cooldown is not None:
        out["cooldown_ms"] = cooldown
    pseudo = skill_change(row, live)
    if pseudo is not None:
        out["pseudo_stats"] = pseudo
    if out and live.get("ilvl") and live["ilvl"] != option.get("ilvl"):
        out["ilvl"] = live["ilvl"]
    return out


def live_row(row, change):
    """The reference row with the change applied, keys in the reference row's order."""
    row = json.loads(json.dumps(row))
    option = row["scaling_options"]["0"]
    stats = {int(k): v for k, v in (option.get("stats") or {}).items()}
    for stat, value in change.get("stats", {}).items():
        if value is None:
            stats.pop(stat, None)
        else:
            stats[stat] = value
    if stats:
        option["stats"] = {str(k): stats[k] for k in sorted(stats)}
    else:
        option.pop("stats", None)
    if "damage" in change:
        option["weapon_damage_min"], option["weapon_damage_max"] = change["damage"]
    if "speed" in change:
        row["weapon_speed"] = change["speed"]
    if "ilvl" in change:
        option["ilvl"] = change["ilvl"]
    if "cooldown_ms" in change:
        row["item_effects"][on_use_effect(row)]["on_use"]["cooldown_ms"] = change["cooldown_ms"]
    if "pseudo_stats" in change:
        if any(change["pseudo_stats"]):
            row["pseudo_stats"] = change["pseudo_stats"]
        else:
            row.pop("pseudo_stats", None)
    order = ["rand_prop_points", "weapon_damage_min", "weapon_damage_max", "stats", "ilvl"]
    row["scaling_options"]["0"] = {
        **{k: option[k] for k in order if k in option},
        **{k: v for k, v in option.items() if k not in order},
    }
    return row


def build(cache, report=None):
    """Add the changed reference items to LIVE; returns (added, left_out) by id."""
    reference = reference_rows()
    live = live_rows()
    added, left_out = {}, {}
    for item_id, row in sorted(reference.items()):
        if not is_gear(row):
            continue
        path = client.cache_path(cache, item_id)
        if not path.exists():
            continue
        if item_id in live:
            # A live row keeps what it has but its on use cooldown and weapon skills.
            try:
                stated = read_tooltip(json.loads(path.read_text()))
            except client.Unreadable:
                continue
            if not stated:
                continue
            change = {}
            if cooldown := cooldown_change(live[item_id], stated):
                change["cooldown_ms"] = cooldown
            if (pseudo := skill_change(live[item_id], stated)) is not None:
                change["pseudo_stats"] = pseudo
            if unified := unified_change(live[item_id], stated):
                change["stats"] = unified
            if change:
                added[item_id] = (row["name"], change)
                live[item_id] = live_row(live[item_id], change)
            continue
        try:
            stated = read_tooltip(json.loads(path.read_text()))
        except client.Unreadable as err:
            left_out[item_id] = (row["name"], str(err))
            continue
        if stated is None:
            continue
        change = changes(row, stated)
        if change:
            added[item_id] = (row["name"], change)
            live[item_id] = live_row(row, change)
    LIVE.write_text("".join(client.dump_row(live[i]) + "\n" for i in sorted(live)))
    if report:
        lines = [f"{i}\t{n}\t{json.dumps(c, sort_keys=True)}\n" for i, (n, c) in sorted(added.items())]
        lines += [f"{i}\t{n}\tleft out: {r}\n" for i, (n, r) in sorted(left_out.items())]
        Path(report).write_text("".join(lines))
    return added, left_out


def check():
    """Offline audit of LIVE: sorted unique ids that items.jsonl holds, each row differing from
    its reference row and keeping its name, and every line as dump_row writes it."""
    reference = reference_rows()
    previous = 0
    count = 0
    for number, line in enumerate(LIVE.read_text().splitlines(), 1):
        row = json.loads(line)
        problems = []
        if row["id"] <= previous:
            problems.append("is not in increasing id order")
        if row["id"] not in reference:
            problems.append("is not an id items.jsonl holds")
        elif row == reference[row["id"]]:
            problems.append("equals its reference row")
        elif row["name"] != reference[row["id"]]["name"]:
            problems.append("renames its reference row")
        if client.dump_row(row) != line:
            problems.append("is not written as dump_row writes it")
        if problems:
            raise ValueError(f"{LIVE.name} line {number} (item {row['id']}): " + ", ".join(problems))
        previous = row["id"]
        count += 1
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("command", choices=["fetch", "build", "check"])
    parser.add_argument("ids", nargs="*", type=int)
    parser.add_argument("--cache")
    parser.add_argument("--report")
    args = parser.parse_args()
    if args.command == "fetch":
        fetch(args.cache, args.ids)
    elif args.command == "build":
        added, left_out = build(args.cache, args.report)
        print(f"{len(added)} rows added, {len(left_out)} left out")
    else:
        print(f"{check()} rows ok")


if __name__ == "__main__":
    main()
