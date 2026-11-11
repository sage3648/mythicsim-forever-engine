//! A levelling Warlock's demons against real combat logs (docs/levels.md): foreverlogs.gg public
//! reports of 5 to 8 November 2026, client 1.60.1.70862. Data credit: foreverlogs.gg public
//! reports; only derived numbers are kept, no character or player names.
//!
//! Each fixture under `fixtures/levels/logs` is MythicSim's levelling request for one logged
//! Warlock: the reference build at the logged level, the logged gear, the logged sheet's
//! attributes and no talents (a capture carries only the points in each tree), summoning the
//! logged demon. The level is the capture's talent points plus 9, a lower bound that the
//! Warlock's own spell ranks agree with.
//!
//! The level 60 demon the engine had at every level put a level 12 Imp's Firebolt at 44 a hit
//! where the log reads 8.5, and a level 25 Succubus's swing at 93 to 118 after armor where the
//! log reads 32 to 41.

use forever_engine::{prepare_json_with, Rules};
use serde_json::Value;
use std::{fs, path::Path};

fn prepared(name: &str, summon: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/levels/logs/{name}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["raid"]["parties"][0]["players"][0]["warlock"]["options"]["classOptions"]["summon"] =
        summon.into();
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{name}: {err}"))
}

fn pet<'a>(prepared: &'a Value, name: &str) -> &'a Value {
    prepared["pets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|pet| pet["name"] == name)
        .unwrap_or_else(|| panic!("no {name}"))
}

fn effect<'a>(prepared: &'a Value, kind: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind}"))
}

/// The demon's one damage spell: its id and its mean non-crit hit, the base plus the coefficient
/// on the demon's spell damage (a tenth of its Warlock's).
fn spell_mean(pet: &Value, base: f64) -> (i64, f64) {
    let spell = pet["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["spell_id"].is_i64())
        .unwrap();
    let coefficient = spell["bonus_coefficient"].as_f64().unwrap();
    let multiplier = spell["damage_multiplier"].as_f64().unwrap();
    let spell_damage = pet["stats"]["SpellDamage"].as_f64().unwrap();
    let mean = (base + coefficient * spell_damage) * multiplier;
    (spell["action_id"]["spell_id"].as_i64().unwrap(), mean)
}

/// The Imp's Firebolt is the rank row at the level plus the row's coefficient on a tenth of its
/// Warlock's spell damage. The measured column is the mean non-crit hit against creatures (the
/// sample size in the comment), logged before the class tuning of 11 November 2026 raised ranks
/// 1 to 4. Against the rows of that time (the third column) every log read 3 to 21% above the
/// engine; the Warlock spells of the same reports read 3% high too
/// (docs/levels-logs-2026-11-09.md). The live rows now read above those logs, most at the low
/// ranks, as the tuning says ("higher increase at lower levels"), and within 3% at rank 4.
#[test]
fn the_imps_firebolt_matches_the_logs() {
    // fixture, rank, engine mean on the rows the logs were made on, measured mean, engine mean
    // on the live rows
    let cases = [
        // Rank 2 at 12, 188 hits: 7 and 0.2 a level from 8 then, 11 and 0.4 now.
        ("affliction-warlock-L12-imp", 7799, 7.03, 8.45, 12.03),
        // Rank 2 at its cap of 13 and 30 spell damage, 386 hits; 7 to 8 Demonology points.
        ("affliction-warlock-L16", 7799, 8.94, 9.58, 13.94),
        // Rank 3 at 18, 83 hits, and at 20, 95 hits: 12 and 0.3 a level then, 15 and 0.4 now.
        ("affliction-warlock-L18-imp", 7800, 12.48, 12.90, 15.48),
        ("affliction-warlock-L20-imp", 7800, 12.74, 13.46, 15.74),
        // Rank 4 at 31 and 72 spell damage, 270 hits: 17 and 0.3 a level then, 18 and 0.4 now.
        ("affliction-warlock-L31-imp", 7801, 21.11, 23.76, 23.11),
    ];
    for (name, rank, before, measured, engine) in cases {
        let prepared = prepared(name, "Imp");
        let base = effect(&prepared, "firebolt")["average"].as_f64().unwrap();
        let (id, mean) = spell_mean(pet(&prepared, "Imp"), base);
        assert_eq!(id, rank, "{name}");
        assert!((mean - engine).abs() < 0.01, "{name}: {mean}");
        // The logs sit between the rows they were made on and the raised ones, or just over.
        assert!(measured > before, "{name}");
        assert!(
            measured < mean * 1.03,
            "{name}: log {measured} against {mean}"
        );
        // Rank 7 at the level, 44 and up, read four times the log or more at 12 to 20 and
        // twice it at 31.
        assert!(44.0 / measured > 1.8, "{name}");
    }
}

/// A level 25 Succubus's Lash of Pain rank 1: 16 plus 0.429 of a tenth of 36 spell damage, 17.5,
/// where 233 logged non-crit hits average 17.2. Rank 6's 50 was three times it.
#[test]
fn the_succubus_lash_of_pain_matches_the_logs() {
    let prepared = prepared("affliction-warlock-L25-succubus", "Succubus");
    let base = effect(&prepared, "lash_of_pain")["base_damage"]
        .as_f64()
        .unwrap();
    let (id, mean) = spell_mean(pet(&prepared, "Succubus"), base);
    assert_eq!(id, 7814);
    assert!((mean / 17.2 - 1.0).abs() < 0.035, "{mean}");
}

/// A level 25 Succubus's white swings against a level 26 caster in Blackfathom Deeps (46 non-crit
/// hits, 2.0 sec apart): 32 at the tenth percentile, 36 the median and 41 the most. The engine's
/// swing at 25, 95 to 131 times the level's creature dps share plus attack power over 14, through
/// the client's level 26 creature armor (`ExpectedStat` 1072) at attacker level 25, spans 32.4 to
/// 41.1. The armor is the client row's, not the creature's own, which the client does not carry;
/// the same swings against the more armored Blackfathom Myrmidons read 21 to 36.
#[test]
fn the_succubus_swing_matches_the_logs() {
    let prepared = prepared("affliction-warlock-L25-succubus", "Succubus");
    let succubus = pet(&prepared, "Succubus");
    let weapon = &succubus["melee"]["main_hand"];
    let attack_power = succubus["stats"]["AttackPower"].as_f64().unwrap();
    let fixed = attack_power / 14.0 * weapon["swing_speed"].as_f64().unwrap();
    let armor = 1072.0;
    let through = 1.0 - armor / (armor + 400.0 + 85.0 * 25.0);
    let low = (weapon["base_damage_min"].as_f64().unwrap() + fixed) * through;
    let high = (weapon["base_damage_max"].as_f64().unwrap() + fixed) * through;
    assert!((low - 32.0).abs() < 1.0, "{low}");
    assert!((high - 41.0).abs() < 1.0, "{high}");
    // The level 60 Succubus, 95 to 131 and 258 attack power of its own (129 Strength), swings up
    // to 118 there.
    let inherited = attack_power - 2.0 * succubus["stats"]["Strength"].as_f64().unwrap();
    let level_60 = (131.0 + (258.0 + inherited) / 7.0) * through;
    assert!(level_60 > 2.5 * 41.0, "{level_60}");
}
