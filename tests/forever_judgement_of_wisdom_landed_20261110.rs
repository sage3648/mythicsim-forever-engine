//! Judgement of Wisdom pays only on landed hits under Forever's rules. A level 41 log counted its
//! mana on 164 of 336 landed melee swings, 59 of 116 direct spells and 44 of 102 ranged shots
//! (about the 50% chance), against 1 of 52 misses, 3 of 401 dot ticks and none of 33 damage
//! shield hits. The reference lets a melee or ranged miss pay as well.
//!
//! The request is the turret Hunter of `fixtures/prepull`, whose raid judges the target with
//! Judgement of Wisdom. Its physical attack table is set to miss every time.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const JUDGEMENT_OF_WISDOM: i64 = 20355;

fn prepared(rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/prepull/hunter-turret-aimed-shot.request.json");
    let mut prepared = prepare_json_with(&fs::read(path).unwrap(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"));
    prepared["sim"]["iterations"] = json!(50);
    prepared
}

/// Every physical attack of the Hunter and its pet misses.
fn always_missing(mut prepared: Value) -> Value {
    prepared["melee"]["base_miss_chance"] = json!(2.0);
    for pet in prepared["pets"].as_array_mut().unwrap() {
        pet["melee"]["base_miss_chance"] = json!(2.0);
    }
    prepared
}

fn landed_only(prepared: &Value) -> bool {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "judgement_of_wisdom")
        .expect("the raid's Judgement of Wisdom")
        .get("landed_only")
        .is_some_and(|value| value == true)
}

/// The mana Judgement of Wisdom returned to the Hunter over the run.
fn judgement_mana(prepared: Value) -> f64 {
    let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
    check_prepared(&typed).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&typed).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|resource| resource["id"]["spellId"].as_i64() == Some(JUDGEMENT_OF_WISDOM))
        .map(|resource| resource["gain"].as_f64().unwrap())
        .sum()
}

#[test]
fn an_always_missing_hunter_gets_no_judgement_of_wisdom_mana() {
    let forever = prepared(Rules::Forever);
    assert!(landed_only(&forever));
    assert_eq!(judgement_mana(always_missing(forever)), 0.0);
}

#[test]
fn a_hunter_that_lands_its_shots_still_gets_the_mana() {
    assert!(judgement_mana(prepared(Rules::Forever)) > 0.0);
}

/// The reference's rules carry no flag, and their missing Hunter still gains mana.
#[test]
fn the_reference_pays_on_a_miss() {
    let reference = prepared(Rules::GoReference);
    assert!(!landed_only(&reference));
    assert!(judgement_mana(always_missing(reference)) > 0.0);
}
