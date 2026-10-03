//! The production Affliction Warlock build: coverage regressions around Nightfall and its
//! summoned Succubus. Its Go result and first-fight log goldens are compared with every other
//! supported case in tests/classes/mage/prepared_v2.rs.

use forever_engine::{check_prepared, contracts::prepared_v2::PreparedV2, PreparedError};
use serde_json::Value;
use std::{fs, path::Path};

fn production() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-affliction-warlock.prepared.json");
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

#[test]
fn production_build_is_supported() {
    let prepared: PreparedV2 = serde_json::from_value(production()).unwrap();
    assert_eq!(check_prepared(&prepared), Ok(()));
    let kinds: Vec<&str> = prepared
        .effects
        .iter()
        .map(|effect| effect.kind())
        .collect();
    for kind in [
        "corruption",
        "bane_of_agony",
        "amplify_curse",
        "nightfall",
        "warlock_pet",
        "lash_of_pain",
    ] {
        assert!(kinds.contains(&kind), "missing {kind}");
    }
    assert_eq!(prepared.pets.len(), 1);
}

/// Nightfall's trigger listens to periodic damage; without its effect the listener is
/// unclaimed and the build is refused.
#[test]
fn nightfall_trigger_needs_its_effect() {
    let mut value = production();
    remove_effect(&mut value, "nightfall");
    let reasons = reasons(value);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("\"Nightfall\"")),
        "{reasons:?}"
    );
}

/// The demon's AI casts Lash of Pain, which needs a behavior.
#[test]
fn succubus_needs_lash_of_pain() {
    let mut value = production();
    remove_effect(&mut value, "lash_of_pain");
    let reasons = reasons(value);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.starts_with("the demon casts") && reason.contains("11780")),
        "{reasons:?}"
    );
}

/// The simulated demon is a unit only through its AI.
#[test]
fn summoned_demon_needs_its_ai() {
    let mut value = production();
    remove_effect(&mut value, "warlock_pet");
    let reasons = reasons(value);
    assert!(
        reasons.iter().any(|reason| reason.contains("Succubus")),
        "{reasons:?}"
    );
}

/// One simulated pet is in scope.
#[test]
fn a_second_simulated_pet_is_refused() {
    let mut value = production();
    let pet = value["pets"][0].clone();
    value["pets"].as_array_mut().unwrap().push(pet);
    assert!(!reasons(value).is_empty());
}
