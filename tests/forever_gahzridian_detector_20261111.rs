//! Gahz'ridian Detector (9978), a quest reward that refused a Feral Druid's Quick Sims twice on 10
//! November 2026 ("no item with id 9978"), under Forever's rules.
//!
//! The live tooltip (Wowhead's Forever tooltip, 11 November 2026): Item Level 1, "Quest Item",
//! Unique, Head, "Equip: Nearby Gahz'ridian appears on the minimap." Its Item row is of the
//! client's quest class (12) with the head slot, and its equip spell Detect Gahz'ridian (11992,
//! ItemEffect 98962) is one resource tracking aura (SpellEffect 688737, aura 45): nothing a
//! fight reads. So the row in `data/forever-client-items.jsonl` is a head piece with no armor, no
//! stats and no effect, and wearing it is wearing nothing in that slot.
//!
//! No refused request was at hand, so the tests add the detector to the equipment of the Feral
//! Druid of `fixtures/forever-gaps/feral-druid-defias-leather`, who wears nothing on the head (50
//! iterations).

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const GAHZRIDIAN_DETECTOR: i64 = 9978;
/// The Elemental Shaman's head piece, Coif of Elements.
const SHAMAN_HEAD: usize = 0;

/// A fixture's request.
fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/forever-gaps/{name}.request.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn items(request: &mut Value) -> &mut Vec<Value> {
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap()
}

/// The Feral Druid, who wears nothing on the head.
fn druid() -> Value {
    fixture("feral-druid-defias-leather")
}

/// The Feral Druid with the detector on.
fn druid_with_the_detector() -> Value {
    let mut request = druid();
    items(&mut request).push(json!({ "id": GAHZRIDIAN_DETECTOR }));
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

/// The detector prepares, passes the gate and runs.
#[test]
fn the_detector_prepares_and_runs() {
    let (_, result) = run(&druid_with_the_detector());
    let dps = result["raidMetrics"]["parties"][0]["players"][0]["dps"]["avg"]
        .as_f64()
        .unwrap();
    assert!(dps > 0.0);
}

/// It has no armor, no stats and no effect: the fight is the fight of the same character with
/// nothing on the head, roll for roll.
#[test]
fn wearing_the_detector_is_wearing_nothing_on_the_head() {
    let (with, with_result) = run(&druid_with_the_detector());
    let (without, without_result) = run(&druid());
    assert_eq!(without_request_hash(with), without_request_hash(without));
    assert_eq!(with_result, without_result);
    // The added piece is read: one with an id no row holds is refused.
    let mut unknown = druid();
    items(&mut unknown).push(json!({ "id": 999_999_999 }));
    assert!(matches!(
        prepare(&unknown, Rules::Forever),
        Err(PrepareError::Refused(_))
    ));
}

/// The reference's rules have no row for the detector and refuse it as unknown, on a request
/// they prepare otherwise (the Elemental Shaman's).
#[test]
fn the_reference_rules_still_refuse_the_detector() {
    let mut request = fixture("elemental-shaman-the-elements");
    assert!(prepare(&request, Rules::GoReference).is_ok());
    items(&mut request)[SHAMAN_HEAD] = json!({ "id": GAHZRIDIAN_DETECTOR });
    match prepare(&request, Rules::GoReference) {
        Err(PrepareError::Refused(refusal)) => {
            let reason = refusal.to_string();
            assert!(reason.contains("no item with id 9978"), "{reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}
