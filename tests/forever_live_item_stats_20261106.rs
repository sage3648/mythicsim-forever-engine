//! Items whose live row differs from the reference database, found by reading Wowhead's Forever
//! tooltip of every item the reference database states differently from Wowhead's gear planner or
//! from the client's item data (6 November 2026). Forever's rules read the live rows
//! (`data/forever-live-items.jsonl`); the reference's rules keep the reference rows.
//!
//! Each case pins what the live tooltip states beside what the reference row carries, for a few
//! items of every kind of difference: a spell power or feral attack power figure the reference
//! scaled differently, an armor, block value or primary stat the reference row lacks or states
//! otherwise, a weapon the live game gave a lower caster damage roll or another speed, and a
//! stat-bearing equip effect the reference row left out.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The holy paladin fixture with no talents (no talent moves these stats, and the two rules'
/// trees differ) and without its blessings (Kings scales the attributes), wearing only the
/// given items.
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

/// Each item, the stats its tooltip states under Forever's rules, and the reference's.
const STAT_CASES: [StatCase; 11] = [
    // Atiesh (the damage and healing version): spell power 150 (was 216).
    (
        22630,
        &["SpellDamage", "HealingPower"],
        &[150.0, 150.0],
        &[216.0, 216.0],
    ),
    // Atiesh (the healing version): healing 300 beside damage 120 (was 424).
    (
        22631,
        &["HealingPower", "SpellDamage"],
        &[300.0, 120.0],
        &[424.0, 120.0],
    ),
    // Mace of Unending Life: "+140 Attack Power in Cat, Bear, and Dire Bear forms only" (was 280).
    (21407, &["FeralAttackPower"], &[140.0], &[280.0]),
    // Hammer of Bestial Fury: its tooltip carries no feral attack power effect (was 154).
    (20580, &["FeralAttackPower"], &[0.0], &[154.0]),
    // Spellshifter Rod: 6 Stamina, 17 Intellect, spell power 40 (was 9, 23 and 54).
    (
        9527,
        &["SpellDamage", "HealingPower", "Intellect", "Stamina"],
        &[40.0, 40.0, 17.0, 6.0],
        &[54.0, 54.0, 23.0, 9.0],
    ),
    // Premier Grand Marshal's Barricade: 44 block value (was 58).
    (278468, &["BlockValue"], &[44.0], &[58.0]),
    // Wormscale Blocker: 57 Block, no Block Value stat (was 75).
    (21610, &["BlockValue"], &[57.0], &[75.0]),
    // Ironbark Shield: 1803 Armor, which the reference row lacks.
    (20502, &["Armor"], &[1803.0], &[0.0]),
    // Gemshale Pauldrons: 347 Armor, 5 Strength and 10 Stamina (was 382, 7 and 13).
    (
        9531,
        &["Armor", "Strength", "Stamina"],
        &[347.0, 5.0, 10.0],
        &[382.0, 7.0, 13.0],
    ),
    // Dragonclaw Ring: 7 Stamina (was 10, and a Spirit the tooltip does not carry).
    (10710, &["Stamina"], &[7.0], &[10.0]),
    // Shard of the Defiler: "Improves your chance to get a critical strike by 15.0%" and
    // "Increases your attack speed and casting speed by 15.0%", which the reference left out.
    (
        17142,
        &["MeleeCritRating", "MeleeHasteRating", "SpellHasteRating"],
        &[210.0, 150.0, 150.0],
        &[0.0, 0.0, 0.0],
    ),
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

const WEAPON_CASES: [WeaponCase; 5] = [
    // Witching Stave, 40 - 61 at 3.60 (was 55 - 83): a caster weapon rolls lower damage live.
    (1484, (40.0, 61.0, 3.6), (55.0, 83.0, 3.6)),
    // Elemental Mage Staff, 109 - 164 (was 147 - 221).
    (944, (109.0, 164.0, 3.2), (147.0, 221.0, 3.2)),
    // Mograine's Might, 105 - 159 at 3.40 (was 87 - 131 at 2.80).
    (7723, (105.0, 159.0, 3.4), (87.0, 131.0, 2.8)),
    // Staff of Noh'Orahil, 46 - 70 (was 62 - 94).
    (15105, (46.0, 70.0, 2.7), (62.0, 94.0, 2.7)),
    // Searing Golden Blade, 14 - 26 (was 21 - 39).
    (12260, (14.0, 26.0, 1.4), (21.0, 39.0, 1.4)),
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
    for id in [22630, 21407, 17142, 1484] {
        let prepared = prepared(&[id], Rules::Forever);
        let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
        check_prepared(&prepared).unwrap();
        simulate_prepared(&prepared).unwrap();
    }
}
