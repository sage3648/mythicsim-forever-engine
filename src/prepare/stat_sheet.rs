//! The character sheet a simulation starts with, for the application to show beside a result:
//! what the engine made of the character, so a player can check it against the game.
//!
//! Each row's value is read off the reset simulation, the state every fight starts in, with
//! every chosen buff up. Its parts say where the value came from: the stats as each step of
//! building the character left them (race and class, gear, the request's bonus stats, talents,
//! buffs, consumables), as Go's `applyAllEffects` measures them for its own sheet, and what the
//! reset's own auras (a form, a stance, a self buff) add on top.
//!
//! Reading the sheet changes nothing: a step is measured on a copy of the stat dependencies,
//! and the final values on a copy of the environment. The prepared contract does not carry it.

use std::panic::{catch_unwind, AssertUnwindSafe};

use serde_json::{json, Value};

use super::env::Environment;
use super::items;
use super::sim::{
    BuildPhase, Sim, UnitId, PHYSICAL_HASTE_RATING_PER_HASTE_PERCENT,
    SPELL_HASTE_RATING_PER_HASTE_PERCENT,
};
use super::stats::{Stat, Stats};
use super::weapon_skill::{self, WeaponSkill};

/// The sheet's layout, raised when a row changes meaning.
pub(crate) const VERSION: u32 = 1;

/// The steps a character is built in, then what a reset adds: one part of every row each.
/// "bonus" is the request's bonus stats, which apply with the equipment but are not gear: the
/// application sends a food the engine has no item for that way.
pub(crate) const SOURCES: [&str; 7] = [
    "base", "gear", "bonus", "talents", "buffs", "consumes", "other",
];

/// The steps before the equipment applies: an item's weapon skill is the gear's.
const BASE_STEP: usize = 0;

/// The spell schools a character has a damage stat for, with the row each makes.
const SCHOOLS: [(&str, Stat); 6] = [
    ("arcaneDamage", Stat::ArcaneDamage),
    ("fireDamage", Stat::FireDamage),
    ("frostDamage", Stat::FrostDamage),
    ("holyDamage", Stat::HolyDamage),
    ("natureDamage", Stat::NatureDamage),
    ("shadowDamage", Stat::ShadowDamage),
];

/// One row of a reading: its key, its value and, for a weapon skill, the skill line.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Row {
    pub key: &'static str,
    pub value: f64,
    pub detail: Option<&'static str>,
}

/// The sheet as one moment of the build has it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Reading(pub Vec<Row>);

impl Reading {
    fn value(&self, key: &str) -> Option<f64> {
        self.0
            .iter()
            .find(|row| row.key == key)
            .map(|row| row.value)
    }
}

/// The stats a unit that is still being built would have with its dependencies applied: Go
/// `measureStats`, on a copy of the dependencies so the build goes on untouched. `less` is
/// taken off first, to read the unit as it would be without those stats.
pub(crate) fn built_stats(sim: &Sim, unit: UnitId, less: Option<&Stats>) -> Stats {
    let unit = sim.unit(unit);
    let mut dependencies = unit.sdm.clone();
    let stats = match less {
        Some(less) => unit.stats.subtract(less),
        None => unit.stats,
    };
    dependencies
        .sort_and_apply_stat_dependencies(stats)
        .floor_game_stats()
}

/// The sheet of `unit` with `stats` as its stats. `step` is the build step just applied, or
/// `None` for the reset simulation.
pub(crate) fn read(sim: &Sim, unit_id: UnitId, stats: &Stats, step: Option<usize>) -> Reading {
    let unit = sim.unit(unit_id);
    let pseudo = &unit.pseudo_stats;
    let mut rows = Vec::new();
    let mut row = |key: &'static str, value: f64| {
        rows.push(Row {
            key,
            value,
            detail: None,
        });
    };

    row("health", stats[Stat::Health]);
    if unit.mana_bar.enabled {
        row("mana", stats[Stat::Mana]);
    }
    if unit.energy_bar.enabled {
        row("energy", unit.energy_bar.max_energy);
    }
    row("strength", stats[Stat::Strength]);
    row("agility", stats[Stat::Agility]);
    row("stamina", stats[Stat::Stamina]);
    row("intellect", stats[Stat::Intellect]);
    row("spirit", stats[Stat::Spirit]);

    let physical_haste =
        1.0 + stats[Stat::MeleeHasteRating] / (PHYSICAL_HASTE_RATING_PER_HASTE_PERCENT * 100.0);
    row("attackPower", stats[Stat::AttackPower]);
    row("meleeHit", stats[Stat::PhysicalHitPercent]);
    row("meleeCrit", stats[Stat::PhysicalCritPercent]);
    row(
        "meleeHaste",
        (pseudo.attack_speed_multiplier * pseudo.melee_speed_multiplier * physical_haste - 1.0)
            * 100.0,
    );
    row("rangedAttackPower", stats[Stat::RangedAttackPower]);
    row(
        "rangedHit",
        stats[Stat::PhysicalHitPercent] + stats[Stat::RangedHitPercent],
    );
    row(
        "rangedCrit",
        stats[Stat::PhysicalCritPercent] + stats[Stat::RangedCritPercent],
    );
    row(
        "rangedHaste",
        (pseudo.attack_speed_multiplier * pseudo.ranged_speed_multiplier * physical_haste - 1.0)
            * 100.0,
    );

    row("spellDamage", stats[Stat::SpellDamage]);
    for (key, school) in SCHOOLS {
        row(key, stats[Stat::SpellDamage] + stats[school]);
    }
    row("healingPower", stats[Stat::HealingPower]);
    row("spellHit", stats[Stat::SpellHitPercent]);
    row("spellCrit", stats[Stat::SpellCritPercent]);
    row(
        "spellHaste",
        (pseudo.cast_speed_multiplier
            * (1.0
                + stats[Stat::SpellHasteRating] / (SPELL_HASTE_RATING_PER_HASTE_PERCENT * 100.0))
            - 1.0)
            * 100.0,
    );
    row("mp5", stats[Stat::MP5]);
    row("spellPenetration", stats[Stat::SpellPiercing]);

    row("armor", stats[Stat::Armor]);
    row(
        "defense",
        f64::from(5 * unit.level) + stats[Stat::DefenseRating].floor(),
    );

    rows.extend(skill_rows(sim, unit_id, step));
    Reading(rows)
}

/// The skill each hand attacks with, as the character sheet counts it: 5 a level and what
/// races, talents, sets and the gear itself add. A hand without a weapon has no row.
fn skill_rows(sim: &Sim, unit_id: UnitId, step: Option<usize>) -> Vec<Row> {
    let unit = sim.unit(unit_id);
    let base = f64::from(5 * unit.level);
    // Go `updateWeaponSkills`: a Druid's forms attack with paws, on the Feral Combat line.
    if sim.character(unit_id).class == "ClassDruid" {
        return vec![Row {
            key: "mainHandSkill",
            value: base + unit.feral_combat_skill,
            detail: Some("Feral Combat"),
        }];
    }
    let geared = step != Some(BASE_STEP);
    let equipment = &sim.character(unit_id).equipment;
    [
        ("mainHandSkill", items::slot::MAIN_HAND),
        ("offHandSkill", items::slot::OFF_HAND),
        ("rangedSkill", items::slot::RANGED),
    ]
    .into_iter()
    .filter_map(|(key, slot)| {
        let skill = weapon_skill::weapon_skill_of(&equipment[slot])?;
        let above = if geared {
            sim.weapon_skill(unit_id, skill)
        } else {
            unit.weapon_skill_bonus[skill as usize]
        };
        Some(Row {
            key,
            value: base + above,
            detail: Some(skill_name(skill)),
        })
    })
    .collect()
}

fn skill_name(skill: WeaponSkill) -> &'static str {
    match skill {
        WeaponSkill::Axes => "Axes",
        WeaponSkill::Swords => "Swords",
        WeaponSkill::Maces => "Maces",
        WeaponSkill::Daggers => "Daggers",
        WeaponSkill::Unarmed => "Fist Weapons",
        WeaponSkill::TwoHandedAxes => "Two-Handed Axes",
        WeaponSkill::TwoHandedSwords => "Two-Handed Swords",
        WeaponSkill::TwoHandedMaces => "Two-Handed Maces",
        WeaponSkill::Polearms => "Polearms",
        WeaponSkill::Staves => "Staves",
        WeaponSkill::Bows => "Bows",
        WeaponSkill::Guns => "Guns",
        WeaponSkill::Crossbows => "Crossbows",
        WeaponSkill::Thrown => "Thrown",
    }
}

/// The sheet as build step `step` leaves it, or `None` when reading it failed: a sheet that
/// cannot be read costs the result its sheet, never the simulation. With `less`, the sheet as
/// it would be without those stats.
pub(crate) fn measure(
    sim: &Sim,
    unit: UnitId,
    step: usize,
    less: Option<&Stats>,
) -> Option<Reading> {
    catch_unwind(AssertUnwindSafe(|| {
        read(sim, unit, &built_stats(sim, unit, less), Some(step))
    }))
    .ok()
}

/// The sheet of a prepared environment's player, or `None` when a reading failed. The
/// environment is reset and has every build step's reading.
///
/// The values are the reset simulation's with every chosen buff up, as Go's `FillPlayerStats`
/// reads its final stats: a buff an ally casts during the fight (Battle Shout) is not up when
/// the fight starts, and a player checking the sheet against the game has it. The buffs go up
/// on a copy, so the environment the export reads is untouched.
pub(crate) fn of_environment(env: &Environment) -> Option<Value> {
    if env.build_readings.len() != SOURCES.len() - 1 {
        return None;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let mut buffed = env.clone();
        let player = buffed.player;
        buffed.sim.apply_build_phase_auras(player, BuildPhase::ALL);
        let reset = read(&buffed.sim, player, &buffed.sim.stats(player), None);
        sheet(&env.build_readings, &reset)
    }))
    .ok()
}

/// The sheet as the application reads it: every row of the reset simulation's reading with its
/// parts in [`SOURCES`] order. A part is what its step added to the one before; the last is
/// what the reset left over the finished build. `steps` are the readings of the build's steps,
/// in order.
///
/// A row a step did not have yet (a bar its talents enabled) starts from nothing at that step.
pub(crate) fn sheet(steps: &[Reading], reset: &Reading) -> Value {
    let rows: Vec<Value> = reset
        .0
        .iter()
        .map(|row| {
            let mut parts = Vec::with_capacity(SOURCES.len());
            let mut before = 0.0;
            for step in steps {
                let value = step.value(row.key).unwrap_or(before);
                parts.push(rounded(value - before));
                before = value;
            }
            parts.push(rounded(row.value - before));
            let mut out = json!({
                "key": row.key,
                "value": rounded(row.value),
                "parts": parts,
            });
            if let Some(detail) = row.detail {
                out["detail"] = json!(detail);
            }
            out
        })
        .collect();
    json!({ "version": VERSION, "sources": SOURCES, "rows": rows })
}

/// A sheet value to the fourth decimal: the sums behind it carry float noise the sheet has no
/// use for, and a part that is nothing reads as exactly nothing.
fn rounded(value: f64) -> f64 {
    let rounded = (value * 10_000.0).round() / 10_000.0;
    if rounded == 0.0 {
        0.0
    } else {
        rounded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(rows: &[(&'static str, f64)]) -> Reading {
        Reading(
            rows.iter()
                .map(|(key, value)| Row {
                    key,
                    value: *value,
                    detail: None,
                })
                .collect(),
        )
    }

    /// Each part is what its step added, and the parts of a row add up to its value.
    #[test]
    fn parts_are_the_steps_differences_and_add_up() {
        let steps = [
            reading(&[("intellect", 120.0)]),
            reading(&[("intellect", 300.0)]),
            reading(&[("intellect", 305.0)]),
            reading(&[("intellect", 330.0)]),
            reading(&[("intellect", 361.0)]),
            reading(&[("intellect", 361.0)]),
        ];
        let out = sheet(&steps, &reading(&[("intellect", 371.0)]));
        let row = &out["rows"][0];
        assert_eq!(row["key"], "intellect");
        assert_eq!(row["value"], 371.0);
        assert_eq!(
            row["parts"],
            json!([120.0, 180.0, 5.0, 25.0, 31.0, 0.0, 10.0])
        );
        assert_eq!(out["sources"], json!(SOURCES));
    }

    /// A row the early steps lack, such as a bar a talent enables, has nothing from them.
    #[test]
    fn a_row_missing_from_a_step_adds_nothing_there() {
        let steps = [
            reading(&[]),
            reading(&[]),
            reading(&[]),
            reading(&[("energy", 110.0)]),
            reading(&[("energy", 110.0)]),
            reading(&[("energy", 110.0)]),
        ];
        let out = sheet(&steps, &reading(&[("energy", 110.0)]));
        assert_eq!(
            out["rows"][0]["parts"],
            json!([0.0, 0.0, 0.0, 110.0, 0.0, 0.0, 0.0])
        );
    }

    /// A warrior's sheet off a prepared environment: the gloves' dagger skill is the gear's
    /// part of the main hand's row, over the 300 a level 60 character has, every row's parts
    /// add up to its value, and a class without mana or energy has neither row.
    #[test]
    fn a_prepared_environment_gives_its_players_sheet() {
        let mut listing = vec![(0, 0); items::slot::MAIN_HAND + 1];
        // Alcor's Sunrazor, a dagger, and Aged Core Leather Gloves, 5 dagger skill.
        listing[items::slot::MAIN_HAND] = (14555, 0);
        listing[items::slot::HANDS] = (18823, 0);
        let env = crate::prepare::item_test_support::environment(&listing);
        let before = env.sim.stats(env.player);
        let out = of_environment(&env).expect("a sheet");
        assert_eq!(
            env.sim.stats(env.player),
            before,
            "reading changed the stats"
        );

        let rows = out["rows"].as_array().unwrap();
        let row = |key: &str| rows.iter().find(|row| row["key"] == key);
        let skill = row("mainHandSkill").expect("a main hand skill row");
        assert_eq!(skill["value"], 305.0);
        assert_eq!(skill["detail"], "Daggers");
        assert_eq!(skill["parts"], json!([300.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0]));
        assert!(row("offHandSkill").is_none(), "no weapon, no row");
        assert!(row("mana").is_none() && row("energy").is_none());
        assert_eq!(
            row("strength").unwrap()["value"],
            env.sim.stat(env.player, Stat::Strength)
        );
        for row in rows {
            let sum: f64 = row["parts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|part| part.as_f64().unwrap())
                .sum();
            let value = row["value"].as_f64().unwrap();
            assert!(
                (sum - value).abs() < 1e-3,
                "{}: {sum} != {value}",
                row["key"]
            );
        }
    }

    /// A sheet is given only with every build step read.
    #[test]
    fn a_missing_step_reading_means_no_sheet() {
        let mut env = crate::prepare::item_test_support::environment(&[]);
        env.build_readings.pop();
        assert!(of_environment(&env).is_none());
    }

    /// Float noise from the sums does not reach the sheet, and nothing is never negative zero.
    #[test]
    fn values_are_rounded_and_zero_is_plain() {
        assert_eq!(rounded(0.1 + 0.2), 0.3);
        assert!(rounded(-0.000_000_1).is_sign_positive());
    }
}
