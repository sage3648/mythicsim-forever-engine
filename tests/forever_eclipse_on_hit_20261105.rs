//! Eclipse under Forever's rules (5 November 2026).
//!
//! Eclipse (408248, client 1.60.1.70790) has SpellAuraOptions ProcTypeMask_0 65536, a harmful
//! spell's damage, so only a Wrath that lands banks its two charges (effect 2 base points 2,
//! aura 408255 stacking to 4). Tested on the beta: a Wrath that misses gives none. Each Starfire
//! then casts faster and spends one. The Go reference banks them on every completed Wrath cast.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const WRATH: i64 = 9912;
const STARFIRE: i64 = 25298;
/// The rank Forever's rules resolve the rotation's Starfire to.
const STARFIRE_RANK_7: i64 = 9876;
const ECLIPSE: i64 = 408255;
const ITERATIONS: i64 = 200;

/// The production Balance board request with only Starfire while Eclipse has a charge, else
/// Wrath while 15 seconds remain, in a fixed one minute fight: the last charges are spent before
/// the end, so every banked charge becomes a Starfire.
fn balance() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-balance-druid.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["encounter"]["duration"] = json!(60);
    request["encounter"]["durationVariation"] = json!(0);
    request["simOptions"]["iterations"] = json!(ITERATIONS);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    let charged = json!({"cmp": {"op": "OpGe",
        "lhs": {"auraNumStacks": {"auraId": {"spellId": ECLIPSE}}},
        "rhs": {"const": {"val": "1"}}}});
    let time_left = json!({"cmp": {"op": "OpGe",
        "lhs": {"remainingTime": {}},
        "rhs": {"const": {"val": "15s"}}}});
    request["raid"]["parties"][0]["players"][0]["rotation"] = json!({
        "type": "TypeAPL",
        "prepullActions": [],
        "priorityList": [
            {"action": {"castSpell": {"spellId": {"spellId": STARFIRE}}, "condition": charged}},
            {"action": {"castSpell": {"spellId": {"spellId": WRATH}}, "condition": time_left}},
        ],
    });
    request
}

/// Casts and misses of the spells in `spell_ids` over every iteration.
fn casts_and_misses(player: &Value, spell_ids: &[i64]) -> (f64, f64) {
    let targets: Vec<&Value> = player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| spell_ids.contains(&action["id"]["spellId"].as_i64().unwrap_or(0)))
        .map(|action| &action["targets"][0])
        .collect();
    let total = |field: &str| {
        targets
            .iter()
            .map(|t| t[field].as_f64().unwrap_or(0.0))
            .sum()
    };
    (total("casts"), total("misses"))
}

fn run(rules: Rules) -> (f64, f64, f64) {
    let prepared = prepare_json_with(balance().to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"));
    let prepared = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    let player = &report.result["raidMetrics"]["parties"][0]["players"][0];
    let (wraths, wrath_misses) = casts_and_misses(player, &[WRATH]);
    let (starfires, _) = casts_and_misses(player, &[STARFIRE, STARFIRE_RANK_7]);
    (wraths, wrath_misses, starfires)
}

#[test]
fn a_missed_wrath_banks_no_charges() {
    let (wraths, misses, starfires) = run(Rules::Forever);
    assert!(misses > 0.05 * wraths, "{misses} misses in {wraths} Wraths");
    assert_eq!(starfires, 2.0 * (wraths - misses));
}

#[test]
fn the_go_reference_banks_on_every_cast() {
    let (wraths, misses, starfires) = run(Rules::GoReference);
    assert!(misses > 0.05 * wraths, "{misses} misses in {wraths} Wraths");
    assert_eq!(starfires, 2.0 * wraths);
}
