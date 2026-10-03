//! Warrior tests: the prepared v2 gate on the production Fury Warrior request. Its Go result
//! and first-fight log are compared with the rest of the fixture family.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepared_coverage, PreparedError,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn fury_json() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-warrior.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn reasons(value: Value) -> Vec<String> {
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    match check_prepared(&prepared) {
        Err(PreparedError::Unsupported(reasons)) => reasons,
        other => panic!("expected unsupported, got {other:?}"),
    }
}

fn effect_mut<'a>(value: &'a mut Value, kind: &str) -> &'a mut Value {
    value["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap()
}

#[test]
fn production_fury_request_is_supported() {
    let prepared: PreparedV2 = serde_json::from_value(fury_json()).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
}

/// A stance change runs only with the stance category and the stance passives exported.
#[test]
fn a_stance_change_needs_the_stance_passives() {
    let mut value = fury_json();
    let last = value["player"]["rotation"]["priorityList"]
        .as_array()
        .unwrap()
        .len()
        - 1;
    let item = &mut value["player"]["rotation"]["priorityList"][last]["action"];
    assert_eq!(item["castSpell"]["spellId"]["spellId"], 2458);
    item["castSpell"]["spellId"]["spellId"] = json!(2457);
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "pseudo_stat_auras");
    assert!(reasons(value).contains(&"rotation reaches spell 2457, a stance change".into()));
}

/// The raid's Expose Armor holds the armor category for good, so the warrior's own Sunder
/// Armor can never be cast. Without it the stacks would change the target's armor.
#[test]
fn sunder_armor_needs_a_blocked_category() {
    let mut value = fury_json();
    effect_mut(&mut value, "sunder_armor")["blocked"] = json!(false);
    assert_eq!(
        reasons(value),
        ["rotation reaches spell 11597, which stacks the warrior's own Sunder Armor"]
    );
}

/// A DPS warrior casts Retaliation only by hand, so it has no behavior of its own.
#[test]
fn retaliation_in_the_rotation_has_no_behavior() {
    let mut value = fury_json();
    value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            json!({"action": {"castSpell": {"spellId": {"spellId": 20230}}}}),
        );
    assert_eq!(
        reasons(value),
        ["rotation casts spell 20230, which has no behavior"]
    );
}

/// Without Improved Slam the cast stops the swings, which the runtime lacks.
#[test]
fn slam_that_stops_the_swings_is_unsupported() {
    let mut value = fury_json();
    value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            json!({"action": {"castSpell": {"spellId": {"spellId": 11605}}}}),
        );
    assert_eq!(
        reasons(value),
        ["rotation reaches spell 11605, whose cast stops the swings"]
    );
}

/// Go gives `currentRage` no value without a rage bar, which drops the term.
#[test]
fn rage_without_a_rage_bar_is_unsupported() {
    let mut value = fury_json();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "rage_bar");
    let reasons = reasons(value);
    assert!(reasons.contains(&"rotation item 15 reads rage, which the player lacks".into()));
    assert!(reasons
        .contains(&"player aura \"RageBar\" listens to combat events without an effect".into()));
}
