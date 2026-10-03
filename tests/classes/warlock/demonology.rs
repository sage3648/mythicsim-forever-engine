//! The production Demonology Warlock build: coverage regressions around Decimation, Demonic
//! Brand and the inert demons. Its Go result and first-fight log goldens are compared with
//! every other supported case in tests/classes/mage/prepared_v2.rs.

use forever_engine::{check_prepared, contracts::prepared_v2::PreparedV2, PreparedError};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn production() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-demonology-warlock.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn reasons(value: Value) -> Vec<String> {
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    match check_prepared(&prepared) {
        Err(PreparedError::Unsupported(reasons)) => reasons,
        other => panic!("expected unsupported, got {other:?}"),
    }
}

fn remove_effect(value: &mut Value, kind: &str) {
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != kind);
}

fn effect<'a>(value: &'a mut Value, kind: &str) -> &'a mut Value {
    value["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap()
}

#[test]
fn production_build_is_supported() {
    let prepared: PreparedV2 = serde_json::from_value(production()).unwrap();
    assert_eq!(check_prepared(&prepared), Ok(()));
    let kinds: Vec<&str> = prepared
        .effects
        .iter()
        .map(|effect| effect.kind())
        .collect();
    for kind in ["decimation", "demonic_brand", "warlock_pet", "lash_of_pain"] {
        assert!(kinds.contains(&kind), "missing {kind}");
    }
}

/// Decimation's trigger and Demonic Brand's trigger and consumer listen to hits; each needs
/// its effect.
#[test]
fn triggers_need_their_effects() {
    let mut value = production();
    remove_effect(&mut value, "decimation");
    assert!(reasons(value).contains(
        &"player aura \"Decimation Trigger\" listens to combat events without an effect".into()
    ));
    let mut value = production();
    remove_effect(&mut value, "demonic_brand");
    let reasons = reasons(value);
    assert!(reasons.contains(
        &"player aura \"Demonic Brand Trigger\" listens to combat events without an effect".into()
    ));
    assert!(reasons.contains(
        &"pet aura \"Demonic Brand consumer\" listens to combat events without an effect".into()
    ));
}

/// Only the 35% execute phase is modeled.
#[test]
fn decimation_needs_the_35_percent_phase() {
    let mut value = production();
    effect(&mut value, "decimation")["execute_phase"] = json!(20);
    assert!(reasons(value).contains(&"Decimation's execute phase 20 is unsupported".into()));
}

/// The brand hit reads the warlock's shadow power as fixed.
#[test]
fn brand_needs_fixed_school_power() {
    let mut value = production();
    value["effects"].as_array_mut().unwrap().push(json!({
        "kind": "stat_auras", "auras": [], "combos": [], "changed": ["ShadowDamage"]
    }));
    assert!(reasons(value).contains(
        &"Demonic Brand reads ShadowDamage, which an aura changes during the fight".into()
    ));
}

/// The inert demons export their permanent auras, which Go activates at every reset, and
/// the brand marker the rotation's auraIsKnown guard looks up on the first of them.
#[test]
fn inert_demons_carry_their_permanent_auras() {
    let value = production();
    let inert: Vec<&Value> = value["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == "inert_pet")
        .collect();
    assert_eq!(inert.len(), 3);
    for pet in inert {
        let permanent = pet["permanent_auras"].as_array().unwrap();
        assert!(permanent.contains(&json!({"spell_id": 19028})));
        assert!(pet["auras"]
            .as_array()
            .unwrap()
            .contains(&json!({"spell_id": 1293696})));
    }
}
