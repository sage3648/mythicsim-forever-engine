//! Stormshroud Armor's Revitalize (3 pieces, 23863) under Forever's rules of 21 November 2026:
//! the live server restores 18 energy on 3% of landed melee hits, where the client rows of
//! 1.60.1.71037 still state 4% and 30 (Wowhead's Forever tooltip read the live values that day).
//! The Go reference keeps the client rows.

use forever_engine::contracts::prepared_v2::PreparedV2;
use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The live Combat Rogue reference's talents, which only Forever's rules take.
const COMBAT: &str = "00530320250001-320013220201232115-";

/// The Combat Rogue fixture wearing Stormshroud's chest, legs, gloves and shoulders, on the
/// live tree under Forever's rules.
fn stormshroud_rogue(rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-combat-rogue.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let player = &mut request["raid"]["parties"][0]["players"][0];
    if rules == Rules::Forever {
        player["talentsString"] = json!(COMBAT);
    }
    let items = player["equipment"]["items"].as_array_mut().unwrap();
    for (slot, id) in [(2, 15058), (4, 15056), (8, 15057), (6, 21278)] {
        items[slot] = json!({ "id": id });
    }
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

/// Revitalize's energize effect: (chance on the first hit it rolls on, energy).
fn revitalize(prepared: &Value) -> (f64, f64) {
    let effect = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["kind"] == "energize_proc" && effect["trigger_aura"] == "Stormshroud Armor 3P"
        })
        .expect("Revitalize's energize");
    let chances = effect["chances"].as_array().unwrap();
    assert!(!chances.is_empty(), "Revitalize rolls on no hit");
    assert!(chances
        .iter()
        .all(|chance| chance["chance"] == chances[0]["chance"]));
    (
        chances[0]["chance"].as_f64().unwrap(),
        effect["energy"].as_f64().unwrap(),
    )
}

/// The energy Revitalize restored over a short fight: (events, gain).
fn energy_restored(prepared: Value) -> (i64, f64) {
    let mut prepared = prepared;
    prepared["sim"]["iterations"] = json!(20);
    prepared["sim"]["debug_first_iteration"] = json!(false);
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let result = forever_engine::simulate_prepared(&prepared).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["id"]["spellId"] == 23864)
        .map(|resource| {
            (
                resource["events"].as_i64().unwrap(),
                resource["gain"].as_f64().unwrap(),
            )
        })
        .expect("Revitalize's energy row")
}

#[test]
fn revitalize_restores_18_energy_on_3_percent_under_forevers_rules() {
    let prepared = stormshroud_rogue(Rules::Forever);
    assert_eq!(revitalize(&prepared), (0.03, 18.0));
    let (events, gain) = energy_restored(prepared);
    assert!(events > 0);
    assert_eq!(gain, 18.0 * events as f64);
}

#[test]
fn the_reference_keeps_the_client_rows() {
    let prepared = stormshroud_rogue(Rules::GoReference);
    assert_eq!(revitalize(&prepared), (0.04, 30.0));
}
