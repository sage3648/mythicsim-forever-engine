//! Levelling characters against real combat logs (docs/levels-logs-2026-11-09.md): the public
//! reports on foreverlogs.gg of 5 to 8 November 2026, client 1.60.1.70862, levels 9 to 35.
//!
//! Each fixture under `fixtures/levels/logs` is MythicSim's levelling request for one logged
//! character: the class's reference build at the logged level, wearing the logged gear, with the
//! logged sheet's attributes and no talents (a capture carries only the points in each tree). The
//! tests pin what the logs measured where the engine agrees:
//!
//! - a low rank's base damage at the character's level, scaled by the client row's points per
//!   level up to the rank's level cap;
//! - the client row's spell power coefficient on the spell damage the logged gear gives, with no
//!   further low level penalty on top of the row's own coefficient;
//! - the share of attack power a Garrote tick takes.
//!
//! The armory's own item summary understates the gear's spell damage (3 where the items give 27);
//! the engine's item rows are what reproduce the logged hits.

use forever_engine::{prepare_json_with, Rules};
use serde_json::Value;
use std::{fs, path::Path};

fn prepared(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/levels/logs/{name}.request.json"));
    let request = fs::read(path).unwrap();
    prepare_json_with(&request, "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{name}: {err}"))
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap()
}

/// The player's prepared spell of this id that carries `field` (a direct roll or a dot).
fn spell<'a>(prepared: &'a Value, spell_id: i64, field: &str) -> &'a Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["spell_id"] == spell_id && !spell[field].is_null())
        .unwrap_or_else(|| panic!("no spell {spell_id} with {field}"))
}

/// The effect entry that states this spell's tick, wherever the class exports it.
fn tick_row(value: &Value, spell_id: i64) -> Option<&Value> {
    match value {
        Value::Object(map) => {
            if map.get("spell_id") == Some(&spell_id.into())
                && (map.contains_key("tick_base") || map.contains_key("tick_damage"))
            {
                return Some(value);
            }
            map.values().find_map(|value| tick_row(value, spell_id))
        }
        Value::Array(items) => items.iter().find_map(|value| tick_row(value, spell_id)),
        _ => None,
    }
}

/// A direct spell's mean non-crit hit: the roll's average plus the coefficient on spell damage.
fn direct_mean(prepared: &Value, spell_id: i64) -> (f64, f64, f64) {
    let spell = spell(prepared, spell_id, "damage_effect");
    let average = spell["damage_effect"]["average"].as_f64().unwrap();
    let coefficient = spell["bonus_coefficient"].as_f64().unwrap();
    let multiplier = spell["damage_multiplier"].as_f64().unwrap();
    let mean = (average + coefficient * stat(prepared, "SpellDamage")) * multiplier;
    (average, coefficient, mean)
}

/// A spell dot's non-crit tick: the row's tick plus the dot's coefficient on spell damage.
fn spell_tick(prepared: &Value, spell_id: i64) -> (f64, f64, f64) {
    let base = tick_row(&prepared["effects"], spell_id).unwrap()["tick_base"]
        .as_f64()
        .unwrap();
    let dot = &spell(prepared, spell_id, "dot")["dot"];
    let coefficient = dot["bonus_coefficient"].as_f64().unwrap();
    let multiplier = dot["periodic_damage_multiplier"].as_f64().unwrap();
    let tick = (base + coefficient * stat(prepared, "SpellDamage")) * multiplier;
    (base, coefficient, tick)
}

/// The gear's spell damage: the logged characters' items, read from the engine's item rows.
#[test]
fn the_logged_gear_gives_its_spell_damage() {
    for (name, spell_damage) in [
        ("frost-mage-L14", 27.0),
        ("frost-mage-L21", 32.0),
        ("affliction-warlock-L16", 30.0),
        ("shadow-priest-L22", 35.0),
        ("balance-druid-L25", 37.0),
    ] {
        assert_eq!(stat(&prepared(name), "SpellDamage"), spell_damage, "{name}");
    }
}

/// Direct spells: the rank's base at the level and the row's coefficient. The measured column is
/// the median non-crit hit (sample size in the comment); the engine's mean lands within 3.5%,
/// the room the capture leaves for talents it cannot show (Frostbolt rank 3 and Shadow Bolt rank 3
/// read 3% high, as a 1 to 3 point damage talent would).
#[test]
fn low_rank_direct_spells_match_the_logs() {
    // fixture, spell, rank base at the level, row coefficient, measured median non-crit hit
    let cases = [
        // Frostbolt rank 3 (spell level 14, cap 18), 185 hits.
        ("frost-mage-L14", 837, 46.0, 0.597, 64.0),
        // Arcane Explosion rank 1 (14, cap 19), 43 hits.
        ("frost-mage-L14", 1449, 32.0, 0.143, 36.0),
        // Fire Blast rank 2 (14, cap 19), 38 hits.
        ("frost-mage-L14", 2137, 58.0, 0.429, 68.0),
        // Frostbolt rank 4 (20, cap 24): 0.8 a level floors to 70 at 21. 52 hits.
        ("frost-mage-L21", 7322, 70.0, 0.706, 94.0),
        // Arcane Explosion rank 1 at its cap, 501 hits.
        ("frost-mage-L21", 1449, 34.0, 0.143, 38.0),
        // Fire Blast rank 2 at its cap, 12 hits.
        ("frost-mage-L21", 2137, 63.0, 0.429, 77.0),
        // Shadow Bolt rank 3 (12, cap 17), 53 hits.
        ("affliction-warlock-L16", 705, 42.0, 0.8, 68.0),
        // Wrath rank 4 (22, cap 28), 60 hits.
        ("balance-druid-L25", 5179, 38.0, 0.571, 60.0),
    ];
    for (name, spell_id, base, row_coefficient, measured) in cases {
        let prepared = prepared(name);
        let (average, coefficient, mean) = direct_mean(&prepared, spell_id);
        assert_eq!(average, base, "{name} {spell_id}: base");
        assert!(
            (coefficient - row_coefficient).abs() < 1e-6,
            "{name} {spell_id}: coefficient {coefficient}"
        );
        assert!(
            (mean / measured - 1.0).abs() <= 0.035,
            "{name} {spell_id}: engine {mean:.1} against the logged {measured}"
        );
    }
}

/// Dot ticks have no roll and no level scaling, so a tick reads the coefficient directly: the
/// logged median tick is the engine's to the whole point.
#[test]
fn low_rank_dot_ticks_match_the_logs() {
    // fixture, spell, row tick, row coefficient, measured median non-crit tick
    let cases = [
        // Corruption rank 2, 177 ticks: 13 + 0.2 x 30.
        ("affliction-warlock-L16", 6222, 13.0, 0.2, 19.0),
        // Drain Life rank 1, 32 ticks: 10 + 0.1 x 30.
        ("affliction-warlock-L16", 689, 10.0, 0.1, 13.0),
        // Immolate rank 2's tick, 71 ticks: 6 + 0.13 x 30.
        ("affliction-warlock-L16", 707, 6.0, 0.13, 10.0),
        // Shadow Word: Pain rank 3, 110 ticks: 18 + 0.2 x 35.
        ("shadow-priest-L22", 970, 18.0, 0.2, 25.0),
    ];
    for (name, spell_id, row_tick, row_coefficient, measured) in cases {
        let prepared = prepared(name);
        let (base, coefficient, tick) = spell_tick(&prepared, spell_id);
        assert_eq!(base, row_tick, "{name} {spell_id}: tick");
        assert!(
            (coefficient - row_coefficient).abs() < 1e-6,
            "{name} {spell_id}: coefficient {coefficient}"
        );
        assert!(
            (tick - measured).abs() <= 0.5,
            "{name} {spell_id}: engine {tick:.2} against the logged {measured}"
        );
    }
}

/// Garrote ticks take 3% of attack power on the rank's tick, rank 1 at level 18 (90 ticks) and
/// rank 2 at 24 (33 ticks), both outside Subtlety (whose talent adds 20%, see the doc).
#[test]
fn garrote_ticks_match_the_logs() {
    for (name, spell_id, row_tick, attack_power, measured) in [
        ("assassination-rogue-L18", 703, 24.0, 135.0, 28.0),
        ("combat-rogue-L24", 8631, 34.0, 173.0, 40.0),
    ] {
        let prepared = prepared(name);
        assert_eq!(stat(&prepared, "AttackPower"), attack_power, "{name}");
        let row = tick_row(&prepared["effects"], spell_id).unwrap();
        assert_eq!(row["tick_damage"].as_f64().unwrap(), row_tick, "{name}");
        let share = row["attack_power_share"].as_f64().unwrap();
        assert_eq!(share, 0.03, "{name}");
        let tick = row_tick + share * attack_power;
        assert!(
            (tick - measured).abs() <= 1.0,
            "{name}: engine {tick:.2} against the logged {measured}"
        );
    }
}
