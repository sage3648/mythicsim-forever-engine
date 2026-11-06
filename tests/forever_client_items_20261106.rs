//! Low level gear the live client has and the reference database lacks (wago.tools
//! `product=wowf` and Wowhead's Forever tooltips, 6 November 2026), such as Worn Cloak, which a
//! leveling character wears. Forever's rules know these items (`data/forever-client-items.jsonl`,
//! `tools/rust_client_items.py`); the reference's rules refuse them as an unknown item.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const WORN_CLOAK: i64 = 1733;
const CRUDE_BASTARD_SWORD: i64 = 1412;
const FEEBLE_SWORD: i64 = 1413;
const CRACKED_BUCKLER: i64 = 2212;

const BACK: usize = 3;
const MAIN_HAND: usize = 14;
const OFF_HAND: usize = 15;

/// Holy to Holy Power 5/5 in the Forever tree.
const HOLY_POWER_BUILD: &str = "0502521022102125";

/// The holy paladin fixture with the given back, main hand and off hand items (0 for none).
fn request(back: i64, main_hand: i64, off_hand: i64) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/holy-paladin-healing.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!(HOLY_POWER_BUILD);
    for (slot, id) in [(BACK, back), (MAIN_HAND, main_hand), (OFF_HAND, off_hand)] {
        player["equipment"]["items"][slot] = if id == 0 {
            json!({})
        } else {
            json!({ "id": id })
        };
    }
    serde_json::to_vec(&request).unwrap()
}

fn prepared(request: &[u8], rules: Rules) -> Value {
    prepare_json_with(request, "rust", rules).unwrap_or_else(|err| panic!("{err}"))
}

fn refusal(request: &[u8], rules: Rules) -> String {
    match prepare_json_with(request, "rust", rules) {
        Err(PrepareError::Refused(refusal)) => refusal.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn armor(prepared: &Value) -> f64 {
    prepared["player"]["stats"]["Armor"].as_f64().unwrap()
}

#[test]
fn worn_cloak_and_a_grey_sword_prepare_with_the_tooltip_numbers() {
    let without = prepared(&request(0, 0, 0), Rules::Forever);
    let worn = prepared(&request(WORN_CLOAK, CRUDE_BASTARD_SWORD, 0), Rules::Forever);
    // Worn Cloak: 13 Armor.
    assert_eq!(armor(&worn) - armor(&without), 13.0);
    // Crude Bastard Sword: 7 - 12 Damage, Speed 3.20.
    let main_hand = &worn["melee"]["main_hand"];
    assert_eq!(main_hand["base_damage_min"], 7.0);
    assert_eq!(main_hand["base_damage_max"], 12.0);
    assert_eq!(main_hand["swing_speed"], 3.2);
}

#[test]
fn a_grey_shield_adds_its_armor_and_block_value() {
    let without = prepared(&request(0, FEEBLE_SWORD, 0), Rules::Forever);
    let shielded = prepared(&request(0, FEEBLE_SWORD, CRACKED_BUCKLER), Rules::Forever);
    // Cracked Buckler: 61 Armor, 1 Block.
    assert_eq!(armor(&shielded) - armor(&without), 61.0);
    let block = |prepared: &Value| prepared["player"]["stats"]["BlockValue"].as_f64().unwrap();
    assert_eq!(block(&shielded) - block(&without), 1.0);
    assert_eq!(shielded["melee"]["main_hand"]["swing_speed"], 2.0);
}

#[test]
fn the_prepared_request_runs() {
    let prepared = prepared(&request(WORN_CLOAK, CRUDE_BASTARD_SWORD, 0), Rules::Forever);
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    check_prepared(&prepared).unwrap();
    simulate_prepared(&prepared).unwrap();
}

#[test]
fn the_reference_rules_still_refuse_the_items() {
    for (request, id) in [
        (request(WORN_CLOAK, 0, 0), WORN_CLOAK),
        (request(0, CRUDE_BASTARD_SWORD, 0), CRUDE_BASTARD_SWORD),
    ] {
        let reason = refusal(&request, Rules::GoReference);
        assert!(reason.contains("unknown_item"), "{reason}");
        assert!(
            reason.contains(&format!("no item with id {id}")),
            "{reason}"
        );
    }
}

#[test]
fn an_item_in_neither_table_is_still_refused() {
    let reason = refusal(&request(0, 999_999_999, 0), Rules::Forever);
    assert!(reason.contains("no item with id 999999999"), "{reason}");
}

/// The reference's own rows win: an id both tables could hold reads from the reference.
#[test]
fn reference_rows_are_not_shadowed() {
    let worn = prepared(&request(0, 25, 0), Rules::Forever);
    // Worn Shortsword (25): 1 - 3 Damage, Speed 1.9, in the reference database.
    assert_eq!(worn["melee"]["main_hand"]["base_damage_min"], 1.0);
    assert_eq!(worn["melee"]["main_hand"]["swing_speed"], 1.9);
}
