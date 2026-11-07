# Levelling characters

Status: foundation, 7 Nov 2026. Part of MythicSim's Forever Droptimizer with levelling dungeons (app `docs/forever-droptimizer.md`).

## What a request carries

A request for a character below level 60 sets three Forever-only player fields (`contracts/request/schema.rs`, numbered from 1000 like the other Rust-only fields, so the Go reference refuses them):

- `level`: the character's level. 0 reads as 60.
- `characterSheet`: what MythicSim addon 0.4.0 reads off the character sheet (`proto.CharacterSheet`): base attributes without gear or buffs, the sheet's totals, health, mana, melee and per-school spell crit, and the helpful auras up when it was read.
- `knownSpells`: the spell ids the spellbook says the character has trained.

A level 60 request without these fields prepares exactly as before.

## How preparation uses them

`prepare/levels.rs` holds the character being prepared for the whole of preparation (`with_character`, the pattern `rotation::with_current_target` uses). Two readers follow it:

- `character_level()` replaced the `CHARACTER_LEVEL` constant: the caster level spell amounts scale to (`Effect::average`), the player unit's level, pets' level and the default target's.
- `Ladder::highest()` and `highest_for()` resolve to the highest rank the character knows: the highest rank the spellbook lists, or, for a ladder the spellbook lists none of (a talent, an item's spell or a triggered effect) and for a request without a spellbook, the highest rank whose spell level is at or below the character's level. At 60 that is every rank, so nothing changes there.

Base stats (`character::base_stats`) move to the level (`levels::base_stats_at_level`):

- Attributes are the sheet's base attributes.
- Base attack power follows the class's per-level rule: the table's level 60 value is the class's attack power per level times 60, less 20 (Warrior and Paladin 3 per level, Hunter, Rogue and Shaman 2), or a constant for the casters and Druid.
- Base health, base mana and crit per agility and per intellect are solved from the sheet (`levels::calibrate`). Preparation runs once with no buffs, debuffs or consumables, reads the player's stats, and solves the numbers that make the engine's sheet the exported one: crit per agility `k = (C_s - C_e + k_used A_e) / A_s`, base health `H_used + (H_s - H_e) - 10 (Sta_s - Sta_e)`, base mana the same with 15 per intellect. Attribute differences from buffs up at export carry through those terms. The second pass prepares the real request with the solved numbers.

The client carries no per-level crit tables and wago.tools was down on 7 Nov, so the calibration is the source today. It makes the character's total crit, health and mana match its sheet; the marginal crit of a point of agility is then exact only to the extent that the constant part of crit (base and gear) is what the engine has.

## Gate

`levels::level_refusal` refuses, before preparation and again in the coverage gate:

- a level outside 10 to 59 (other than 60);
- a class not in `LEVELLED_CLASSES`. A class joins the list once every spell it registers is one the character knows or falls back to one it does (a level 20 Mage set to Mage Armor wears Frost Armor), checked with the levelled request dump below;
- a levelling character without a sheet.

The target may be from the player's level to 3 above (`levels::target_levels`); at 60 it stays 60 to 63.

## Checking a class

The app's worker writes every reference build at levels 10 to 50, talents trimmed to the level's points (smallest tree first, deepest talent first) and a synthetic sheet:

```sh
cd worker && FOREVER_LEVEL_DUMP_DIR=/tmp/levels go test ./forever -run TestDumpLevelledRequests
```

Run each through `forever-engine sim --request <file> --gate`. A class is ready when no request prepares a fault, every spell it casts is one known at the level, and DPS rises with level.

## Open

- Spell coefficients below level 20: Classic scaled a low-level spell's coefficient down; whether Forever keeps it is unchecked.
- Dungeon boss armor: the worker scales the level 63 boss's 3731 armor by level. The client does not carry creature armor.
- Spirit regen, weapon skill caps and pet stats below 60 follow the existing level formulas; pets still use level 60 base stats.
