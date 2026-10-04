"""Raid buff, party buff, individual buff and debuff toggles of the production requests.

The production application's builder lists these assumptions by name
(web/src/lib/forever-assumption-items.ts BUFF_NAMES and DEBUFF_NAMES, switched off under
Advanced as "<group>.<field>" in worker/forever/advanced.go). Each variant changes one field
of the request (proto/buffs.proto): a boolean the request leaves off is turned on and one it
sets is removed, the application's "off"; a tristate takes each level it does not have,
Regular, Improved or Missing (removed). Each request also runs unchanged. The same inputs
always give the same variants. Uses only Python's standard library.

python3 validation/2026-10-04-production-buff-toggles/generate.py OUT_DIR ITERATIONS
"""
from collections import Counter
import glob
import json
import sys
from pathlib import Path

# The builder's fields by request group, with the proto type of each.
FIELDS = {
    "raidBuffs": {
        "arcaneBrilliance": "bool",
        "fireResistanceAura": "bool",
        "fireResistanceTotem": "bool",
        "giftOfTheWild": "bool",
        "prayerOfFortitude": "bool",
        "prayerOfSpirit": "bool",
    },
    "partyBuffs": {
        "battleShout": "tristate",
        "devotionAura": "bool",
        "graceOfAirTotem": "bool",
        "leaderOfThePack": "bool",
        "manaSpringTotem": "tristate",
        "moonkinAura": "bool",
        "strengthOfEarthTotem": "bool",
        "trueshotAura": "bool",
        "windfuryTotem": "bool",
    },
    "buffs": {
        "greaterBlessingOfKings": "bool",
        "greaterBlessingOfMight": "bool",
        "greaterBlessingOfWisdom": "bool",
    },
    "debuffs": {
        "curseOfElements": "bool",
        "curseOfRecklessness": "bool",
        "exposeArmor": "bool",
        "faerieFire": "bool",
        "giftOfArthas": "bool",
        "huntersMark": "bool",
        "insectSwarm": "bool",
        "judgementOfTheCrusader": "bool",
        "judgementOfWisdom": "bool",
        "sunderArmor": "bool",
    },
}
TRISTATE = ["TristateEffectMissing", "TristateEffectRegular", "TristateEffectImproved"]


def group_map(request, group):
    """The request map a builder group names, created when absent."""
    raid = request["raid"]
    party = raid["parties"][0]
    if group == "raidBuffs":
        return raid.setdefault("buffs", {})
    if group == "partyBuffs":
        return party.setdefault("buffs", {})
    if group == "buffs":
        return party["players"][0].setdefault("buffs", {})
    return raid.setdefault("debuffs", {})


def current(values, field, kind):
    value = values.get(field)
    if kind == "bool":
        return bool(value)
    return value if value in TRISTATE else "TristateEffectMissing"


OUT = Path(sys.argv[1])
ITERATIONS = int(sys.argv[2])
OUT.mkdir(parents=True, exist_ok=True)
index = []


def write(name, request, base, change, description):
    variant = json.loads(json.dumps(request))
    change(variant)
    variant["simOptions"]["iterations"] = ITERATIONS
    json.dump(variant, open(OUT / f"{name}.request.json", "w"))
    index.append({"scenario": name, "base": base, "change": description})


requests = {
    Path(f).name.replace(".request.json", ""): json.load(open(f))
    for f in sorted(glob.glob(str(Path(__file__).parent / "requests" / "*.json")))
}
for base, request in requests.items():
    for group, fields in FIELDS.items():
        values = group_map(json.loads(json.dumps(request)), group)
        for field, kind in fields.items():
            have = current(values, field, kind)
            targets = [not have] if kind == "bool" else [level for level in TRISTATE if level != have]
            for target in targets:

                def change(variant, group=group, field=field, target=target):
                    values = group_map(variant, group)
                    if target in (False, "TristateEffectMissing"):
                        values.pop(field, None)
                    else:
                        values[field] = target

                label = {True: "on", False: "off"}.get(target, str(target).replace("TristateEffect", "").lower())
                write(f"{base}--{group}-{field}-{label}", request, base, change,
                      {"kind": "toggle", "group": group, "field": field,
                       "from": have, "to": target})
    # The base itself, to tell rejections a toggle causes from the base's own.
    write(f"{base}--base", request, base, lambda variant: None, {"kind": "base"})

json.dump(index, open(OUT / "index.json", "w"), indent=1)
print(len(index), Counter(entry["change"].get("group", "base") for entry in index))
