//! Levelling characters (docs/levels.md, 8 November 2026).
//!
//! The fixtures are MythicSim's levelling requests for three reference builds, written by the
//! app's `TestDumpLevelledRequests`: the build's gear, its talents trimmed to the level's points,
//! no group buffs, and a character sheet consistent with the gear. They prepare at their level,
//! pass the gate, cast only ranks the level teaches and match their sheet's health and mana.

use forever_engine::contracts::prepared_v2::PreparedV2;
use forever_engine::{
    prepare_json_with, prepared_refusals, simulate_prepared, PrepareError, Rules,
};
use serde_json::Value;
use std::{fs, path::Path};

fn request(name: &str) -> Value {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/levels/{name}.request.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn prepare(request: &Value) -> Result<Value, PrepareError> {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// The DPS a prepared request simulates, after the gate has passed it.
fn dps(prepared: &Value) -> f64 {
    let typed: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    let refusals = prepared_refusals(&typed);
    assert!(refusals.is_empty(), "gate refusals: {refusals:?}");
    let report = simulate_prepared(&typed).unwrap();
    report.result["raidMetrics"]["parties"][0]["players"][0]["dps"]["avg"]
        .as_f64()
        .unwrap_or(0.0)
}

/// The player's spells the rotation can press, by spell id.
fn pressable(prepared: &Value) -> Vec<i64> {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|spell| {
            spell["flags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|flag| flag == "SpellFlagAPL")
        })
        .filter_map(|spell| spell["action_id"]["spell_id"].as_i64())
        .collect()
}

#[test]
fn levelling_characters_sim_at_their_level() {
    for name in ["frost-mage-L20", "combat-rogue-L30", "hunter-L20"] {
        let request = request(name);
        let prepared = prepare(&request).unwrap_or_else(|err| panic!("{name}: {err}"));
        let level = request["raid"]["parties"][0]["players"][0]["level"].as_i64();
        assert_eq!(prepared["player"]["level"].as_i64(), level, "{name}");
        let dps = dps(&prepared);
        assert!(dps > 50.0, "{name} does {dps} DPS");
    }
}

/// Frostbolt rank 4 (7322) is learned at 20 and rank 5 (8406) at 26: a level 20 mage presses
/// rank 4, and the rotation's top rank (25304) is pointed at it.
#[test]
fn a_level_20_mage_presses_the_frostbolt_it_knows() {
    let prepared = prepare(&request("frost-mage-L20")).unwrap();
    let spells = pressable(&prepared);
    assert!(spells.contains(&7322), "{spells:?}");
    for unknown in [8406, 25304] {
        assert!(!spells.contains(&unknown), "{unknown} in {spells:?}");
    }
    let rotation = prepared["player"]["rotation"].to_string();
    assert!(
        !rotation.contains("25304"),
        "the rotation still names 25304"
    );
}

/// The sheet's health and mana are the character's, buffs and all: the calibration solves the
/// level's base health and mana so the engine's sheet is the exported one.
#[test]
fn the_sheet_sets_health_and_mana() {
    let request = request("frost-mage-L20");
    let sheet = request["raid"]["parties"][0]["players"][0]["characterSheet"].clone();
    let prepared = prepare(&request).unwrap();
    let stats = &prepared["player"]["stats"];
    for (stat, field) in [("Health", "health"), ("Mana", "mana")] {
        let engine = stats[stat].as_f64().unwrap();
        let exported = sheet[field].as_f64().unwrap();
        assert!(
            (engine - exported).abs() < 1.0,
            "{stat}: {engine} against {exported}"
        );
    }
}

#[test]
fn a_levelling_character_needs_its_sheet_and_ten_levels() {
    let mut without_sheet = request("frost-mage-L20");
    player(&mut without_sheet)
        .as_object_mut()
        .unwrap()
        .remove("characterSheet");
    let mut level_five = request("frost-mage-L20");
    player(&mut level_five)["level"] = 5.into();
    for (request, says) in [(without_sheet, "character sheet"), (level_five, "level 5")] {
        match prepare(&request) {
            Err(PrepareError::Refused(refusal)) => {
                assert_eq!(refusal.code, "level_unsupported");
                assert!(refusal.reason.contains(says), "{}", refusal.reason);
            }
            other => panic!("not refused: {:?}", other.map(|_| ())),
        }
    }
}
