//! The seven reissued Soulforge Armor pieces that refused a Retribution Paladin's Quick Sim on 27
//! November 2026 ("no item with id 226997").
//!
//! The Paladin wore all eight pieces of the set (ItemSet 1673): Wrists 226993, Waist 226994,
//! Shoulders 226995, Legs 226996, Helm 226997, Gloves 226998, Boots 226999 and Chest 227000. The
//! engine held only the boots. Each row is the item's Wowhead Forever tooltip of 28 November 2026
//! checked against its ItemSparse row of client 1.60.1.71139.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const PALADIN: &str =
    "fixtures/forever-gaps/retribution-paladin-soulforge-eight-pieces.request.json";

/// Each piece's item id and the armor its tooltip states.
const PIECES: [(i64, f64); 8] = [
    (226993, 296.0),
    (226994, 380.0),
    (226995, 507.0),
    (226996, 601.0),
    (226997, 556.0),
    (226998, 393.0),
    (226999, 470.0),
    (227000, 684.0),
];

fn request() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(PALADIN);
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    request
}

fn without(id: i64) -> Value {
    let mut request = request();
    let equipment = request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    let at = equipment
        .iter()
        .position(|item| item["id"] == id)
        .unwrap_or_else(|| panic!("the Paladin wears no {id}"));
    equipment.remove(at);
    request
}

fn prepared(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

fn armor(prepared: &Value) -> f64 {
    prepared["player"]["stats"]["Armor"].as_f64().unwrap_or(0.0)
}

fn set_bonuses(prepared: &Value) -> Vec<String> {
    prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|aura| aura["label"].as_str())
        .filter(|label| label.starts_with("Soulforge Armor"))
        .map(str::to_string)
        .collect()
}

#[test]
fn the_paladins_eight_soulforge_pieces_prepare_with_every_set_bonus_and_sim() {
    let with = prepared(&request());
    assert_eq!(
        set_bonuses(&with),
        [
            "Soulforge Armor 2P",
            "Soulforge Armor 3P",
            "Soulforge Armor 4P",
            "Soulforge Armor 5P",
            "Soulforge Armor 6P"
        ]
    );
    let value: PreparedV2 = serde_json::from_value(with).unwrap();
    check_prepared(&value).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&value).unwrap().result;
    let dps = result["raidMetrics"]["parties"][0]["players"][0]["dps"]["avg"]
        .as_f64()
        .unwrap_or(0.0);
    assert!(dps > 0.0, "the Paladin deals damage: {dps}");
}

#[test]
fn each_piece_gives_the_armor_its_tooltip_states_and_counts_toward_the_set() {
    let with = prepared(&request());
    for (id, tooltip_armor) in PIECES {
        let bare = prepared(&without(id));
        assert!(
            armor(&with) - armor(&bare) >= tooltip_armor,
            "{id} gives at least {tooltip_armor} armor"
        );
    }
    // Seven pieces still reach the six piece bonus; five pieces stop at the five piece bonus.
    assert_eq!(set_bonuses(&prepared(&without(226993))).len(), 5);
    let mut five = request();
    let equipment = five["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    equipment.retain(|item| ![226993, 226994, 226996].contains(&item["id"].as_i64().unwrap()));
    assert_eq!(
        set_bonuses(&prepared(&five)),
        [
            "Soulforge Armor 2P",
            "Soulforge Armor 3P",
            "Soulforge Armor 4P",
            "Soulforge Armor 5P"
        ]
    );
}
