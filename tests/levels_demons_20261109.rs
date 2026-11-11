//! A Warlock's demons by level (docs/levels.md, docs/mechanics.md), 9 November 2026.
//!
//! A levelling sweep found every demon at level 60: the Imp cast Firebolt rank 7 and the Succubus
//! Lash of Pain rank 6 at any level, and both swung and cast from level 60 stats, so a level 10
//! Succubus did what a level 47 one did. Under Forever's rules a demon now casts the rank its
//! warlock's level teaches, from the client's rank rows, and its attributes, mana and melee
//! weapon follow the client's creature auto attack dps for the level. Level 60 is unchanged.

use forever_engine::contracts::prepared_v2::PreparedV2;
use forever_engine::{prepare_json_with, prepared_refusals, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn request(level: i32, summon: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/levels/affliction-warlock-L30.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["warlock"]["options"]["classOptions"]["summon"] = summon.into();
    if level == 60 {
        let player = player.as_object_mut().unwrap();
        player.remove("level");
        player.remove("characterSheet");
        player.remove("knownSpells");
    } else {
        player["level"] = level.into();
    }
    // A boss two levels up at every level, as the app sims a levelling character.
    for target in request["encounter"]["targets"].as_array_mut().unwrap() {
        target["level"] = (level + 2).into();
    }
    request
}

fn prepare(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("prepares: {err}"))
}

fn pet<'a>(prepared: &'a Value, name: &str) -> &'a Value {
    prepared["pets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|pet| pet["name"] == name)
        .unwrap_or_else(|| panic!("no {name}"))
}

/// The demon's spell of one of these ids, with its id.
fn pet_spell<'a>(pet: &'a Value, ids: &[i64]) -> Option<(i64, &'a Value)> {
    pet["spells"].as_array().unwrap().iter().find_map(|spell| {
        let id = spell["action_id"]["spell_id"].as_i64()?;
        ids.contains(&id).then_some((id, spell))
    })
}

fn effect<'a>(prepared: &'a Value, kind: &str) -> Option<&'a Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
}

const FIREBOLT: [i64; 7] = [3110, 7799, 7800, 7801, 7802, 11762, 11763];
const LASH_OF_PAIN: [i64; 6] = [7814, 7815, 7816, 11778, 11779, 11780];

/// The client's creature auto attack dps rows the demons scale with (`ExpectedStat`, wowf).
fn creature_dps(level: i32) -> f64 {
    match level {
        16 => 13.58711147308,
        25 => 20.82846641541,
        40 => 32.58558273315,
        60 => 60.45291519165,
        _ => unreachable!("no row copied for {level}"),
    }
}

/// The Imp casts the Firebolt rank its warlock's level teaches, at that rank's cost and
/// coefficient, and the roll is the rank's row at the level. Ranks 1 to 4 are the rows live since
/// the class tuning of 11 November 2026: rank 2 at 10 (11 and 0.4 a level from 8, 11.8), rank 3
/// at 18 (15), rank 4 at 30 (18 and 0.4 a level from 28, 18.8). Rank 6 at 50 (35 and 0.5 a
/// level, 36) and rank 7 at 59 (44.6) and 60 (45.2) are as before.
#[test]
fn the_imp_casts_the_firebolt_rank_its_level_teaches() {
    // level, rank, cost, coefficient, average, variance
    let cases = [
        (10, 7799, 20, 0.314, 11.0, 2.0 / 11.0),
        (18, 7800, 35, 0.529, 15.0, 4.0 / 17.0),
        (30, 7801, 50, 0.571, 18.0, 2.0 / 9.0),
        (50, 11762, 95, 0.571, 36.0, 0.112_676_046_79),
        (59, 11763, 115, 0.571, 44.0, 0.113_636_367_02),
        (60, 11763, 115, 0.571, 45.0, 0.113_636_367_02),
    ];
    for (level, rank, cost, coefficient, average, variance) in cases {
        let prepared = prepare(&request(level, "Imp"));
        let imp = pet(&prepared, "Imp");
        let (id, spell) = pet_spell(imp, &FIREBOLT).expect("the Imp has a Firebolt");
        assert_eq!(id, rank, "level {level}");
        assert_eq!(spell["cost"]["base_cost"], cost, "level {level}");
        assert_eq!(spell["bonus_coefficient"], coefficient, "level {level}");
        let firebolt = effect(&prepared, "firebolt").unwrap();
        assert_eq!(firebolt["average"], average, "level {level}");
        assert_eq!(firebolt["variance"], variance, "level {level}");
        let demon = effect(&prepared, "warlock_pet").unwrap();
        assert_eq!(demon["min_mana"], f64::from(cost), "level {level}");
    }
}

/// The Succubus lashes with the rank its warlock's level teaches, a fixed base from 20 (16) to 60
/// (50), and knows no Lash of Pain below 20, where its warlock cannot summon it.
#[test]
fn the_succubus_lashes_with_the_rank_its_level_teaches() {
    for (level, rank, cost, base) in [
        (20, 7814, 65, 16.0),
        (30, 7815, 80, 22.0),
        (59, 11779, 145, 43.0),
        (60, 11780, 160, 50.0),
    ] {
        let prepared = prepare(&request(level, "Succubus"));
        let succubus = pet(&prepared, "Succubus");
        let (id, spell) = pet_spell(succubus, &LASH_OF_PAIN).expect("a Lash of Pain");
        assert_eq!(id, rank, "level {level}");
        assert_eq!(spell["cost"]["base_cost"], cost, "level {level}");
        assert_eq!(spell["bonus_coefficient"], 0.429, "level {level}");
        let lash = effect(&prepared, "lash_of_pain").unwrap();
        assert_eq!(lash["base_damage"], base, "level {level}");
    }
    let prepared = prepare(&request(15, "Succubus"));
    assert!(pet_spell(pet(&prepared, "Succubus"), &LASH_OF_PAIN).is_none());
    assert!(effect(&prepared, "lash_of_pain").is_none());
    assert_eq!(
        effect(&prepared, "warlock_pet").unwrap()["autocast_spells"],
        json!([])
    );
}

/// A demon's melee weapon, Strength and Agility are its level 60 ones times the creature auto
/// attack dps of its level over level 60's: a level 25 Succubus swings 95 to 131 times 0.345 and
/// has 0.345 of the level 60 Strength. Its Intellect, Spirit and mana stay, so a levelling Imp
/// keeps casting as the logs show. At 60 every number is the reference's.
#[test]
fn a_demons_swing_and_stats_follow_the_creature_dps_row() {
    // demon, level 60 weapon, level 60 Strength, Agility, Intellect, Spirit and mana; the first
    // two scale
    let demons = [
        (
            "Voidwalker",
            (31.0, 46.0),
            [129.0, 85.0, 70.0, 150.0, 1066.0],
        ),
        (
            "Succubus",
            (95.0, 131.0),
            [129.0, 85.0, 70.0, 150.0, 1066.0],
        ),
        (
            "Felhunter",
            (70.0, 97.0),
            [129.0, 85.0, 70.0, 150.0, 1066.0],
        ),
        ("Imp", (0.0, 0.0), [122.0, 35.0, 264.0, 260.0, 576.0]),
    ];
    for (name, (low, high), base) in demons {
        for level in [16, 25, 40, 60] {
            if level < 20 && name != "Voidwalker" && name != "Imp" {
                continue;
            }
            let share = creature_dps(level) / creature_dps(60);
            let prepared = prepare(&request(level, name));
            let demon = pet(&prepared, name);
            assert_eq!(demon["level"], level, "{name} {level}");
            if name != "Imp" {
                let weapon = &demon["melee"]["main_hand"];
                let (min, max) = (
                    weapon["base_damage_min"].as_f64().unwrap(),
                    weapon["base_damage_max"].as_f64().unwrap(),
                );
                assert!((min - low * share).abs() < 1e-9, "{name} {level}: {min}");
                assert!((max - high * share).abs() < 1e-9, "{name} {level}: {max}");
                assert_eq!(weapon["swing_speed"], 2.0);
            }
            let stats = &demon["stats_without_deps"];
            for (i, stat) in ["Strength", "Agility", "Intellect", "Spirit", "Mana"]
                .iter()
                .enumerate()
            {
                let want = if i < 2 { base[i] * share } else { base[i] };
                let have = stats[stat].as_f64().unwrap();
                assert!((have - want).abs() < 1e-9, "{name} {level} {stat}: {have}");
            }
        }
    }
}

/// The reference's rules keep the level 60 demon whatever the level: the Go goldens never see a
/// levelling request, and the prepared demon is the reference's.
#[test]
fn the_reference_rules_keep_the_level_60_demon() {
    let prepared = prepare_json_with(
        request(60, "Succubus").to_string().as_bytes(),
        "rust",
        Rules::GoReference,
    )
    .unwrap();
    let succubus = pet(&prepared, "Succubus");
    assert_eq!(pet_spell(succubus, &LASH_OF_PAIN).unwrap().0, 11780);
    assert_eq!(succubus["melee"]["main_hand"]["base_damage_min"], 95.0);
    assert_eq!(succubus["melee"]["main_hand"]["base_damage_max"], 131.0);
}

fn pet_dps(prepared: &Value, name: &str) -> f64 {
    let typed: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    let refusals = prepared_refusals(&typed);
    assert!(refusals.is_empty(), "gate refusals: {refusals:?}");
    let result = simulate_prepared(&typed).unwrap().result;
    let player = &result["raidMetrics"]["parties"][0]["players"][0];
    player["pets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|pet| pet["name"].as_str().is_some_and(|n| n.ends_with(name)))
        .map_or(0.0, |pet| pet["dps"]["avg"].as_f64().unwrap_or(0.0))
}

/// Each demon's damage rises with its warlock's level and reaches the level 60 demon's only at
/// 60. Before, a level 20 Succubus dealt what a level 59 one did. The Imp's rises to level 25
/// and then holds near 12 dps: from there its mana runs dry in a long fight at any rank, sooner
/// since the tuning of 11 November 2026 raised the low ranks' damage and not their cost.
#[test]
fn each_demons_damage_rises_with_the_level() {
    for (name, levels) in [
        ("Imp", &[12, 18, 25][..]),
        ("Voidwalker", &[12, 25, 40, 55, 60][..]),
        ("Succubus", &[20, 30, 45, 59, 60][..]),
        ("Felhunter", &[30, 45, 59, 60][..]),
    ] {
        let mut last = 0.0;
        for &level in levels {
            let mut request = request(level, name);
            request["simOptions"]["iterations"] = 20.into();
            let dps = pet_dps(&prepare(&request), name);
            assert!(dps > last, "{name} at {level}: {dps} after {last}");
            last = dps;
        }
    }
}
