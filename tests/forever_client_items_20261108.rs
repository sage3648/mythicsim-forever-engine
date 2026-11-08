//! Items the Forever client adds whose ItemSparse rows wago.tools cannot read (the client ships
//! them encrypted), taken from Wowhead's Forever tooltips of 8 November 2026 with their Item rows
//! (`tools/rust_client_items.py`, `data/forever-client-items.jsonl`).
//!
//! A Retribution Paladin sim on 7 November was refused with "no item with id 274600": Berserker
//! Shackles, a level 34 mail wrist. Tooltip: Item Level 39, 109 Armor, +7 Strength,
//! "Equip: Increased Defense +6."

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const BERSERKER_SHACKLES: i64 = 274600;
const WRIST: usize = 5;

/// Holy to Holy Power 5/5 in the Forever tree: no talent scales Armor, Strength or Defense.
const HOLY_POWER_BUILD: &str = "0502521022102125";

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// A request with the first player's wrist slot set to the item (0 for none).
fn with_wrist(mut request: Value, id: i64) -> Vec<u8> {
    request["simOptions"]["iterations"] = json!(20);
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"][WRIST] = if id == 0 {
        json!({})
    } else {
        json!({ "id": id })
    };
    serde_json::to_vec(&request).unwrap()
}

/// The holy paladin fixture as a Dwarf without buffs, so the item's numbers reach the sheet
/// unscaled (the fixture's Human scales Strength under Forever's rules).
fn holy_paladin() -> Value {
    let mut request = fixture("fixtures/mage/prepared-v2/holy-paladin-healing.request.json");
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["race"] = json!("RaceDwarf");
    player["talentsString"] = json!(HOLY_POWER_BUILD);
    player["buffs"] = json!({});
    request["raid"]["buffs"] = json!({});
    request["raid"]["parties"][0]["buffs"] = json!({});
    request
}

fn retribution_paladin() -> Value {
    fixture("fixtures/paladin/retribution-paladin-unsealed-wrath-command-echoes.request.json")
}

fn prepared(request: &[u8], rules: Rules) -> Value {
    prepare_json_with(request, "rust", rules).unwrap_or_else(|err| panic!("{err}"))
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap()
}

#[test]
fn berserker_shackles_add_their_armor_strength_and_defense() {
    let without = prepared(&with_wrist(holy_paladin(), 0), Rules::Forever);
    let worn = prepared(
        &with_wrist(holy_paladin(), BERSERKER_SHACKLES),
        Rules::Forever,
    );
    assert_eq!(stat(&worn, "Armor") - stat(&without, "Armor"), 109.0);
    assert_eq!(stat(&worn, "Strength") - stat(&without, "Strength"), 7.0);
    // Increased Defense +6 is 6 Defense rating, as data/items.jsonl holds the same line of a
    // Classic item (Ardent Custodian, Increased Defense +5: 5).
    assert_eq!(
        stat(&worn, "DefenseRating") - stat(&without, "DefenseRating"),
        6.0
    );
}

/// The refused request's shape: a Retribution Paladin wearing the item prepares and runs.
#[test]
fn a_retribution_paladin_wearing_berserker_shackles_runs() {
    let prepared = prepared(
        &with_wrist(retribution_paladin(), BERSERKER_SHACKLES),
        Rules::Forever,
    );
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    check_prepared(&prepared).unwrap();
    simulate_prepared(&prepared).unwrap();
}

#[test]
fn the_reference_rules_still_refuse_berserker_shackles() {
    match prepare_json_with(
        &with_wrist(retribution_paladin(), BERSERKER_SHACKLES),
        "rust",
        Rules::GoReference,
    ) {
        Err(PrepareError::Refused(refusal)) => {
            let reason = refusal.to_string();
            assert!(reason.contains("no item with id 274600"), "{reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}
