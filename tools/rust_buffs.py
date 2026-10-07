#!/usr/bin/env python3
"""Translate the reference's generated raid buffs and debuffs into Rust.

The pinned Go engine generates sim/core/buffs/buffs_auto_gen.go and debuffs_auto_gen.go from its
buff manifest: one Meta per buff, naming the client row and how the aura bids, and the functions
that apply each buff the request sets. This writes the same tables and dispatch as
src/prepare/buffs/generated.rs, so Rust preparation applies exactly the buffs Go does.

write   Read the files from a checkout of the pin (--source, default the oracle cache) and write
        src/prepare/buffs/generated.rs.
check   Translate again and fail when the committed file differs.
"""

import argparse
from pathlib import Path
import re
import subprocess
import sys

from compare import ROOT, PIN

OUTPUT = ROOT / "src" / "prepare" / "buffs" / "generated.rs"
FILES = ["sim/core/buffs/buffs_auto_gen.go", "sim/core/buffs/debuffs_auto_gen.go"]

META = re.compile(r"var (\w+)Meta = &Meta\{\n(.*?)\n\}", re.S)
SPELL = re.compile(r"var (\w+)Spell = spelldata\.MustFind\((\d+)\)")
CATEGORY = re.compile(r"var (\w+) = \"([^\"]*)\"")
CONSTRUCTOR = re.compile(r"func (\w+)Aura\(unit \*core\.Unit, isPlayer bool, talentPoints int32(?:, count float64)?\) \*core\.Aura \{\n\treturn (\w+)\(unit, (\w+)Meta")
APPLY = re.compile(r"func (applyGenerated(?:Buffs|Debuffs))\((.*?)\) \{\n(.*?)\n\}", re.S)
CASE = re.compile(r"\tif (.*?) \{\n\t\t(.*?)\n\t\}")


def snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def rust_string(text):
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def translate_field(name, value, spells, categories):
    """One Meta field as a Rust struct field."""
    value = value.strip().rstrip(",")
    if name == "Label":
        return f"label: {value}"
    if name == "Spell":
        return f"spell: {spells[value.removesuffix('Spell')]}"
    if name == "Cast":
        return f"cast: Some({re.fullmatch(r'spelldata\.MustFind\((\d+)\)', value).group(1)})"
    if name in ("Category", "SharedCategory"):
        literal = categories.get(value, value) if not value.startswith('"') else value.strip('"')
        return f"{snake(name)}: {rust_string(literal)}"
    if name in ("SingleAura", "PerStat", "TalentScalesDuration", "FullComboPoints"):
        return f"{snake(name)}: {value}"
    if name == "Talent":
        spell, ranks = re.fullmatch(r"spelldata\.Talent\((\d+), (\d+)\)", value).groups()
        return f"talent: Some(({spell}, {ranks}))"
    if name == "TalentEffect":
        return f"talent_effect: {value}"
    if name == "SkipAuras":
        auras = re.findall(r"dbcenums\.(A_\w+)", value)
        return "skip_auras: &[" + ", ".join(f"dbcenums::{aura}" for aura in auras) + "]"
    raise ValueError(f"unknown Meta field {name}: {value}")


def translate_condition(condition):
    """`party.BattleShout != proto.TristateEffect_TristateEffectMissing` and friends."""
    match = re.fullmatch(r"(\w+)\.(\w+) != proto\.TristateEffect_TristateEffectMissing", condition)
    if match:
        return f'{match.group(1)}.enum_number("{snake(match.group(2))}") != 0'
    match = re.fullmatch(r"(\w+)\.(\w+) > 0", condition)
    if match:
        return f'{match.group(1)}.int("{snake(match.group(2))}") > 0'
    match = re.fullmatch(r"(\w+)\.(\w+)", condition)
    if match:
        return f'{match.group(1)}.bool("{snake(match.group(2))}")'
    raise ValueError(f"unknown condition {condition}")


def translate_statement(statement, unit_expr):
    match = re.fullmatch(r"core\.MakePermanent\((\w+)Aura\(&char\.Unit, false, (.*)\)\)", statement) or \
        re.fullmatch(r"core\.MakePermanent\((\w+)Aura\(target, false, (.*)\)\)", statement)
    if match:
        name, talent = match.groups()
        tristate = re.fullmatch(r"core\.GetTristateValueInt32\((\w+)\.(\w+), (\d+), (\d+)\)", talent)
        if tristate:
            message, field, regular, improved = tristate.groups()
            talent = f'tristate({message}.enum_number("{snake(field)}"), {regular}, {improved})'
        return f"permanent(env, {unit_expr}, &{snake(name).upper()}, false, {talent})?;"
    match = re.fullmatch(r"(drive\w+)\((\w+), (\w+)(?:, (\w+))?\)", statement)
    if match:
        function = snake(match.group(1))
        args = ", ".join(arg for arg in match.groups()[2:] if arg)
        return f"super::drivers::{function}(env, {unit_expr}, {args})?;"
    raise ValueError(f"unknown statement {statement}")


def translate(source):
    metas, applies = [], []
    for file in FILES:
        text = (source / file).read_text()
        spells = {name: spell for name, spell in SPELL.findall(text)}
        categories = {name: value for name, value in CATEGORY.findall(text)}
        constructors = {meta: kind for _, kind, meta in CONSTRUCTOR.findall(text)}
        for name, body in META.findall(text):
            fields = []
            for line in body.split("\n"):
                key, _, value = line.strip().partition(":")
                fields.append(translate_field(key, value, spells, categories))
            kind = {"newBuff": "Buff", "newDebuff": "Debuff", "newItemCountBuff": "ItemCountBuff",
                    "newDamageShield": "DamageShield"}[constructors[name]]
            metas.append((name, kind, fields))
        for function, params, body in APPLY.findall(text):
            unit_expr = "unit" if "char" in params else "target"
            cases = [(translate_condition(cond), translate_statement(stmt.strip(), unit_expr))
                     for cond, stmt in CASE.findall(body)]
            applies.append((function, params, cases))
    out = [
        "//! Translated by tools/rust_buffs.py from the reference's sim/core/buffs/buffs_auto_gen.go and",
        f"//! debuffs_auto_gen.go at {PIN}. Do not edit; run `python3 tools/rust_buffs.py write`.",
        "",
        "use crate::contracts::request::Message;",
        "",
        "use super::super::dbcenums;",
        "use super::super::env::Environment;",
        "use super::super::sim::UnitId;",
        "use super::super::Refusal;",
        "use super::{permanent, tristate, Meta, MetaKind};",
        "",
    ]
    for name, kind, fields in metas:
        out.append(f"pub(crate) static {snake(name).upper()}: Meta = Meta {{")
        out.append(f"    kind: MetaKind::{kind},")
        for field in fields:
            out.append(f"    {field},")
        out.append("    ..Meta::DEFAULT")
        out.append("};")
        out.append("")
    for function, params, cases in applies:
        if function == "applyGeneratedBuffs":
            signature = ("pub(crate) fn apply_generated_buffs(env: &mut Environment, unit: UnitId, raid: &Message, "
                         "party: &Message, individual: &Message) -> Result<(), Refusal> {")
        else:
            signature = ("pub(crate) fn apply_generated_debuffs(env: &mut Environment, target: UnitId, debuffs: &Message, "
                         "raid: &Message) -> Result<(), Refusal> {")
        out.append(signature)
        for condition, statement in cases:
            out.append(f"    if {condition} {{")
            out.append(f"        {statement}")
            out.append("    }")
        out.append("    let _ = raid;")
        out.append("    Ok(())")
        out.append("}")
        out.append("")
    return rustfmt("\n".join(out).rstrip() + "\n")


def rustfmt(text):
    """The file is committed formatted, as `cargo fmt` leaves it, so check compares formatted text."""
    result = subprocess.run(["rustfmt", "--edition", "2021", "--emit", "stdout"], input=text,
                            capture_output=True, text=True, check=True)
    return result.stdout


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
        print(f"{OUTPUT.relative_to(ROOT)} is not the translation of the pinned buffs", file=sys.stderr)
        return 1
    print("Generated buffs match the pinned reference.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
