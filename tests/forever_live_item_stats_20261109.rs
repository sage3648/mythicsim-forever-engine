//! Reference items whose live Wowhead Forever tooltip (9 November 2026) states other base stats,
//! weapon damage or speed than their reference row, written by `tools/rust_live_items.py` into
//! `data/forever-live-items.jsonl`. Forever's rules read the live rows; the reference's rules keep
//! the reference rows.
//!
//! Each case pins what the live tooltip states beside what the reference row carries, for the
//! items references and trinket boards wear most.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The holy paladin fixture with no talents and without its blessings (Kings scales the
/// attributes), wearing only the given items.
fn request(items: &[i64]) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/holy-paladin-healing.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!("");
    player["buffs"] = json!({});
    let items: Vec<Value> = items.iter().map(|id| json!({ "id": id })).collect();
    player["equipment"] = json!({ "items": items });
    serde_json::to_vec(&request).unwrap()
}

fn prepared(items: &[i64], rules: Rules) -> Value {
    prepare_json_with(&request(items), "rust", rules).unwrap_or_else(|err| panic!("{err}"))
}

/// What the item adds to each named stat, under the given rules.
fn added(item: i64, rules: Rules, stats: &[&str]) -> Vec<f64> {
    let without = prepared(&[], rules);
    let with = prepared(&[item], rules);
    stats
        .iter()
        .map(|stat| {
            let value = |prepared: &Value| prepared["player"]["stats"][stat].as_f64().unwrap();
            value(&with) - value(&without)
        })
        .collect()
}

/// An item, the stats it moves, their values on its live tooltip and in the reference row.
type StatCase = (i64, &'static [&'static str], &'static [f64], &'static [f64]);

const STAT_CASES: [StatCase; 8] = [
    // Hand of Justice: "Equip: +12 Attack Power." (was 20, melee and ranged).
    (
        11815,
        &["AttackPower", "RangedAttackPower"],
        &[12.0, 12.0],
        &[20.0, 20.0],
    ),
    // Tarnished Elven Ring: +10 Agility and +10 Arcane Resistance (was 15 Agility, no resistance).
    (
        18500,
        &["Agility", "ArcaneResistance"],
        &[10.0, 10.0],
        &[15.0, 0.0],
    ),
    // Crimson Felt Hat: +10 Intellect and +10 Spirit (was 8 and 8; the fixture's race scales
    // Spirit, so Intellect stands for both).
    (18727, &["Intellect"], &[10.0], &[8.0]),
    // Barbarous Blade: "Equip: +50 Attack Power." (was 60).
    (18520, &["AttackPower"], &[50.0], &[60.0]),
    // Skyshroud Leggings: 11 Stamina, 12 Intellect, spell power 30 (was 8, 8 and 34).
    (
        13170,
        &["Stamina", "Intellect", "SpellDamage"],
        &[11.0, 12.0, 30.0],
        &[8.0, 8.0, 34.0],
    ),
    // Blood-etched Blade: spell power 66 (was 6).
    (
        11922,
        &["SpellDamage", "HealingPower"],
        &[66.0, 66.0],
        &[6.0, 6.0],
    ),
    // Ward of the Elements: "+7 to All Resistances" (was 8).
    (
        12065,
        &["FireResistance", "ShadowResistance"],
        &[7.0, 7.0],
        &[8.0, 8.0],
    ),
    // Mark of Tyranny: no Arcane Resistance on its tooltip (was 10).
    (13966, &["ArcaneResistance"], &[0.0], &[10.0]),
];

#[test]
fn the_items_carry_their_live_tooltip_numbers() {
    for (id, stats, live, reference) in STAT_CASES {
        assert_eq!(added(id, Rules::Forever, stats), live, "item {id}");
        assert_eq!(
            added(id, Rules::GoReference, stats),
            reference,
            "item {id} under the reference's rules"
        );
    }
}

/// A weapon, its damage range and speed on its live tooltip, and the reference row's.
type WeaponCase = (i64, (f64, f64, f64), (f64, f64, f64));

const WEAPON_CASES: [WeaponCase; 2] = [
    // Blood-etched Blade, 26 - 49 at 1.50 (was 39 - 74): a caster weapon now.
    (11922, (26.0, 49.0, 1.5), (39.0, 74.0, 1.5)),
    // Argent Avenger, 62 - 117 at 2.20 (was 71 - 108).
    (13246, (62.0, 117.0, 2.2), (71.0, 108.0, 2.2)),
];

fn main_hand(item: i64, rules: Rules) -> (f64, f64, f64) {
    let prepared = prepared(&[item], rules);
    let hand = &prepared["melee"]["main_hand"];
    let value = |field: &str| hand[field].as_f64().unwrap();
    (
        value("base_damage_min"),
        value("base_damage_max"),
        value("swing_speed"),
    )
}

#[test]
fn the_weapons_swing_for_their_live_damage_and_speed() {
    for (id, live, reference) in WEAPON_CASES {
        assert_eq!(main_hand(id, Rules::Forever), live, "item {id}");
        assert_eq!(
            main_hand(id, Rules::GoReference),
            reference,
            "item {id} under the reference's rules"
        );
    }
}

#[test]
fn the_prepared_requests_run() {
    for id in [11815, 18500, 11922, 13246] {
        let prepared = prepared(&[id], Rules::Forever);
        let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
        check_prepared(&prepared).unwrap();
        simulate_prepared(&prepared).unwrap();
    }
}
