#!/usr/bin/env python3
"""Translate the reference's generated item and enchant effect registrations into Rust.

The pinned Go engine generates sim/common/forever/stat_bonus_cds_auto_gen.go,
stat_bonus_procs_auto_gen.go and enchants_auto_gen.go from the client's spell rows: one call of
a shared constructor per item or enchant, and a great many commented-out ones the generator could
not resolve. This writes the live calls, in Go's registration order, as the tables in
src/prepare/forever_items_generated.rs, which src/prepare/forever_items.rs applies.

write   Read the files from a checkout of the pin (--source, default the oracle cache) and write
        src/prepare/forever_items_generated.rs.
check   Translate again and fail when the committed file differs.
"""

import argparse
from pathlib import Path
import re
import sys

from compare import ROOT

OUTPUT = ROOT / "src" / "prepare" / "forever_items_generated.rs"
DIRECTORY = "sim/common/forever/"
# RegisterAllEffects: RegisterAllOnUseCds, RegisterAllProcs, RegisterAllEnchants.
ITEM_FILES = ["stat_bonus_cds_auto_gen.go", "stat_bonus_procs_auto_gen.go"]
ENCHANT_FILES = ["enchants_auto_gen.go"]

STRING = r'"((?:[^"\\]|\\.)*)"'
ON_USE = {
    "NewSimpleStatActive": "SimpleStatActive",
    "NewSpellDataDamageOnUse": "Damage",
    "NewSpellDataHealOnUse": "Heal",
    "NewSpellDataAbsorbOnUse": "Absorb",
    "NewSpellDataSpeedOnUse": "Speed",
    "NewSpellDataAuraOnUse": "Aura",
    "NewSpellDataEnergizeOnUse": "Energize",
}
PROC = {
    "NewSpellDataProc": "Proc",
    "NewSpellDataDamageProc": "Damage",
    "NewSpellDataHealProc": "Heal",
    "NewSpellDataAbsorbProc": "Absorb",
    "NewSpellDataDebuffProc": "Debuff",
    "NewSpellDataAuraProc": "Aura",
    "NewSpellDataEquipAura": "EquipAura",
}
CALLBACKS = {
    "CallbackOnSpellHitDealt": "ON_SPELL_HIT_DEALT",
    "CallbackOnSpellHitTaken": "ON_SPELL_HIT_TAKEN",
    "CallbackOnPeriodicDamageDealt": "ON_PERIODIC_DAMAGE_DEALT",
    "CallbackOnHealDealt": "ON_HEAL_DEALT",
    "CallbackOnPeriodicHealDealt": "ON_PERIODIC_HEAL_DEALT",
    "CallbackOnCastComplete": "ON_CAST_COMPLETE",
    "CallbackOnApplyEffects": "ON_APPLY_EFFECTS",
    "CallbackOnPeriodicDamageTaken": "ON_PERIODIC_DAMAGE_TAKEN",
}


def snake(name):
    """CamelCase with acronyms as snake_case: MeleeMHAuto is melee_mh_auto, SpellID is spell_id."""
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", name).lower()


def rust_string(text):
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def live_code(text):
    """The file without its comment lines."""
    return "\n".join(line for line in text.split("\n") if not line.lstrip().startswith("//"))


def trim_comment(line):
    return line.split("//")[0]


def terms(value):
    """The names in `core.A | core.B`."""
    return [part.strip().removeprefix("core.") for part in value.split("|")]


def duration(value):
    match = re.fullmatch(r"time\.Millisecond \* (\d+)", value.strip())
    if not match:
        raise ValueError(f"unknown duration {value}")
    return f"{match.group(1)} * MILLISECOND"


def mask(value, rust_type, convert):
    parts = [f"{rust_type}::{convert(part)}.0" for part in terms(value)]
    return f"{rust_type}({' | '.join(parts)})"


def proc_mask_name(part):
    return snake(part.removeprefix("ProcMask")).upper()


def flag_name(part):
    return snake(part.removeprefix("SpellFlag")).upper()


def outcome_name(part):
    return snake(part.removeprefix("Outcome")).upper()


def stacking(body):
    fields = dict(re.findall(r"(\w+):\s+(.*?),\n", body + ",\n"))
    out = []
    for name, value in fields.items():
        value = value.strip()
        if name == "Name":
            out.append(f"name: {value}")
        elif name == "ID":
            out.append(f"id: {value}")
        elif name == "CD":
            out.append(f"cd: {duration(value)}")
        elif name == "Callback":
            parts = [f"CallbackMask::{CALLBACKS[part]}.0" for part in terms(value)]
            out.append(f"callback: CallbackMask({' | '.join(parts)})")
        elif name == "ProcMask":
            out.append("proc_mask: " + mask(value, "ProcMask", proc_mask_name))
        elif name == "SpellFlags":
            out.append("spell_flags: " + mask(value, "SpellFlag", flag_name))
        elif name == "SpellFlagsExclude":
            out.append("spell_flags_exclude: " + mask(value, "SpellFlag", flag_name))
        elif name == "Outcome":
            out.append("outcome: " + mask(value, "HitOutcome", outcome_name))
        elif name in ("RequireDamageDealt", "TrinketLimitsDuration", "CanProcFromProcs"):
            out.append(f"{snake(name)}: {value}")
        else:
            raise ValueError(f"unknown StackingStatBonusCD field {name}")
    return out


def proc_config(body):
    out = []
    for name, value in re.findall(r"(\w+):\s*([^,]+)", body):
        value = value.strip()
        if name == "Name":
            out.append(f"name: {value}")
        elif name in ("EnchantID", "TriggerSpellID", "BuffSpellID"):
            out.append(f"{snake(name)}: {value}")
        elif name == "IsWeaponProc":
            out.append(f"is_weapon_proc: {value}")
        else:
            raise ValueError(f"unknown SpellDataProc field {name}")
    return out


def variants(text):
    return [
        f"ItemVariant {{ item_id: {item}, item_name: {rust_string(name)} }}"
        for item, name in re.findall(r"\{ItemID: (\d+), ItemName: " + STRING + r"\}", text)
    ]


def translate_file(text):
    """The live registrations of one generated file, in file order."""
    code = live_code(text)
    entries = []
    pattern = re.compile(
        r"shared\.(New\w+)\(((?:[^()]|\((?:[^()]|\([^()]*\))*\))*)\)",
        re.S,
    )
    for match in pattern.finditer(code):
        constructor, arguments = match.groups()
        arguments = re.sub(r"\s*//[^\n]*", "", arguments)
        if constructor in ON_USE:
            item = re.fullmatch(r"\s*(\d+)\s*", arguments)
            if not item:
                raise ValueError(f"unknown on-use arguments {arguments}")
            entries.append(f"Registration::OnUse(OnUseKind::{ON_USE[constructor]}, {item.group(1)})")
        elif constructor == "NewStackingStatBonusCD":
            body = re.fullmatch(r"\s*shared\.StackingStatBonusCD\{(.*)\}\s*", arguments, re.S).group(1)
            entries.append(
                "Registration::Stacking(StackingStatBonusCd { " + ", ".join(stacking(body)) + ", ..StackingStatBonusCd::EMPTY })"
            )
        elif constructor in PROC:
            struct = re.match(r"\s*shared\.SpellDataProc\{(.*?)\},\s*(nil|\[\]shared\.ItemVariant\{.*\})\s*$", arguments, re.S)
            fields = proc_config(struct.group(1))
            items = [] if struct.group(2) == "nil" else variants(struct.group(2))
            lines = ", ".join(fields)
            entries.append(
                f"Registration::Proc(ProcKind::{PROC[constructor]}, SpellDataProc {{ {lines}, ..SpellDataProc::EMPTY }}, "
                + "&[" + ", ".join(items) + "])"
            )
        else:
            raise ValueError(f"unknown constructor {constructor}")
    return entries


def translate(source):
    items, enchants = [], []
    for name in ITEM_FILES:
        items += translate_file((source / DIRECTORY / name).read_text())
    for name in ENCHANT_FILES:
        enchants += translate_file((source / DIRECTORY / name).read_text())
    lines = [
        "//! Generated by tools/rust_item_effects.py from the pinned reference's",
        "//! sim/common/forever stat_bonus_cds_auto_gen.go, stat_bonus_procs_auto_gen.go and",
        "//! enchants_auto_gen.go: the live registrations, in Go's registration order. Do not edit.",
        "",
        "use super::aura_helpers::{CallbackMask, HitOutcome};",
        "use super::forever_items::Registration;",
        "use super::shared_items::{ItemVariant, ProcKind, SpellDataProc};",
        "use super::shared_on_use::{OnUseKind, StackingStatBonusCd};",
        "use super::sim::MILLISECOND;",
        "use super::spell::ProcMask;",
        "",
        "/// `RegisterAllOnUseCds`, then `RegisterAllProcs`.",
        "pub(crate) const ITEMS: &[Registration] = &[",
    ]
    lines += [f"    {entry}," for entry in items]
    lines += ["];", "", "/// `RegisterAllEnchants`.", "pub(crate) const ENCHANTS: &[Registration] = &["]
    lines += [f"    {entry}," for entry in enchants]
    lines += ["];", ""]
    text = "\n".join(lines)
    if "SpellFlag(" in text:
        text = text.replace("use super::spell::ProcMask;", "use super::spell::{ProcMask, SpellFlag};")
    return text


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["write", "check"])
    parser.add_argument("--source", type=Path, default=ROOT / "oracle-cache" / "source",
                        help="a checkout of the pinned reference")
    args = parser.parse_args()
    text = translate(args.source)
    if args.command == "write":
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(text)
        print(f"Wrote {OUTPUT.relative_to(ROOT)}")
        return 0
    if not OUTPUT.exists() or OUTPUT.read_text() != text:
        print(f"{OUTPUT.relative_to(ROOT)} is not the translation of the pinned registrations", file=sys.stderr)
        return 1
    print("Generated item effects match the pinned reference.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
