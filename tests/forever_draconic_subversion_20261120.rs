//! Amulet of Draconic Subversion (16787), a quest neck that refused an Enhancement Shaman's Quick
//! Sims five times on 19 November 2026 ("no item with id 16787"), under Forever's rules.
//!
//! The client's Item row (wago.tools `product=wowf`, 1.60.1.70959) is of the quest class (12)
//! with the neck slot, and its ItemSparse row has item level 1 and no stats. The live tooltip
//! (Wowhead's Forever tooltip, 20 November 2026): Item Level 1, "Quest Item", Unique, Neck, "Use:
//! Use to disguise yourself as a member of the Black Dragonflight." The use is Illusion: Black
//! Dragonkin (19937), a transform and a forced faction reaction: nothing a fight reads. So the row
//! in `data/forever-client-items.jsonl` is a neck with no stats and no effect, and wearing it is
//! wearing nothing on the neck.
//!
//! The tests run the refused request, rebuilt from the player's export with the application's
//! own request builder (`fixtures/forever-gaps/enhancement-shaman-amulet-of-draconic-subversion`,
//! 50 iterations), with the amulet and with nothing on the neck.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const AMULET_OF_DRACONIC_SUBVERSION: i64 = 16787;

fn shaman() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "fixtures/forever-gaps/enhancement-shaman-amulet-of-draconic-subversion.request.json",
    );
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(50);
    request
}

fn items(request: &mut Value) -> &mut Vec<Value> {
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap()
}

/// The refused request, with the amulet or with nothing on the neck.
fn shaman_with_amulet(worn: bool) -> Value {
    let mut request = shaman();
    let equipment = items(&mut request);
    let at = equipment
        .iter()
        .position(|item| item["id"] == AMULET_OF_DRACONIC_SUBVERSION)
        .expect("the refused request wears the amulet");
    if !worn {
        equipment.remove(at);
    }
    request
}

fn prepare(request: &Value, rules: Rules) -> Result<Value, PrepareError> {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
}

/// The request under Forever's rules, prepared, through the gate and run: the prepared input and
/// the result.
fn run(request: &Value) -> (Value, Value) {
    let value = prepare(request, Rules::Forever).unwrap_or_else(|err| panic!("{err}"));
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    (value, result)
}

/// The prepared input without the hash of the request it came from, which names the item.
fn without_request_hash(mut prepared: Value) -> Value {
    prepared.as_object_mut().unwrap().remove("request_sha256");
    prepared
}

/// The amulet prepares, passes the gate and runs.
#[test]
fn the_amulet_prepares_and_runs() {
    let (_, result) = run(&shaman_with_amulet(true));
    let dps = result["raidMetrics"]["parties"][0]["players"][0]["dps"]["avg"]
        .as_f64()
        .unwrap();
    assert!(dps > 0.0);
}

/// It has no stats and no effect: the fight is the fight of the same character with nothing on
/// the neck, roll for roll.
#[test]
fn wearing_the_amulet_is_wearing_nothing_on_the_neck() {
    let (with, with_result) = run(&shaman_with_amulet(true));
    let (without, without_result) = run(&shaman_with_amulet(false));
    assert_eq!(without_request_hash(with), without_request_hash(without));
    assert_eq!(with_result, without_result);
}

/// The reference's rules have no row for the amulet and refuse it as unknown.
#[test]
fn the_reference_rules_still_refuse_the_amulet() {
    let request = shaman_with_amulet(true);
    match prepare(&request, Rules::GoReference) {
        Err(PrepareError::Refused(refusal)) => {
            let reason = refusal.to_string();
            assert!(reason.contains("no item with id 16787"), "{reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}
