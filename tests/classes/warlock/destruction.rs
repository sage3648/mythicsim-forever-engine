//! The production Destruction Warlock build: coverage regressions around its mechanics.
//! Its Go result and first-fight log goldens are compared with every other supported case
//! in tests/classes/mage/prepared_v2.rs.

use forever_engine::{check_prepared, contracts::prepared_v2::PreparedV2, PreparedError};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn production() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-destruction-warlock.prepared.json");
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

/// The Curse of the Elements rule reads the curse on the target, Go `GetSourceUnit` with the
/// current target.
const CURSE_ITEM: usize = 7;

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
        "shadow_bolt",
        "immolate",
        "corruption",
        "bane_of_agony",
        "curse_of_the_elements",
        "life_tap",
        "conflagrate",
        "shadowburn",
        "improved_shadow_bolt",
        "shadow_and_flame",
        "sunder_armor_ramp",
    ] {
        assert!(kinds.contains(&kind), "missing {kind}");
    }
}

/// Improved Shadow Bolt's trigger listens to the warlock's hits; without its effect the
/// listener is unclaimed and the build is refused.
#[test]
fn improved_shadow_bolt_trigger_needs_its_effect() {
    let mut value = production();
    remove_effect(&mut value, "improved_shadow_bolt");
    assert!(reasons(value).contains(
        &"player aura \"Improved Shadow Bolt Trigger\" listens to combat events without an effect"
            .to_string()
    ));
}

#[test]
fn curse_of_the_elements_needs_its_effect() {
    let mut value = production();
    remove_effect(&mut value, "curse_of_the_elements");
    assert!(reasons(value)
        .contains(&"rotation reaches spell 1311680 without a known behavior".to_string()));
}

/// Only the player and the current target are units in scope for a source unit.
#[test]
fn aura_source_units_outside_scope_are_unsupported() {
    let mut value = production();
    let condition =
        &mut value["player"]["rotation"]["priorityList"][CURSE_ITEM]["action"]["condition"];
    condition["not"]["val"]["auraIsActive"]["sourceUnit"] = json!({"type": "PreviousTarget"});
    let reasons = reasons(value);
    assert!(
        reasons.iter().any(|reason| reason.starts_with(&format!(
            "rotation item {}: auraIsActive sourceUnit",
            CURSE_ITEM + 1
        ))),
        "{reasons:?}"
    );
}

/// An aura the target lacks reads as inactive (community fix #622), alone or under `not`,
/// so both rules stand.
#[test]
fn target_auras_the_target_lacks_read_as_inactive() {
    let mut negated = production();
    let condition =
        &mut negated["player"]["rotation"]["priorityList"][CURSE_ITEM]["action"]["condition"];
    condition["not"]["val"]["auraIsActive"]["auraId"] = json!({"spellId": 17800});
    let prepared: PreparedV2 = serde_json::from_value(negated).unwrap();
    assert_eq!(check_prepared(&prepared), Ok(()));

    let mut value = production();
    value["player"]["rotation"]["priorityList"][CURSE_ITEM]["action"]["condition"] = json!({
        "auraIsActive": {"auraId": {"spellId": 17800}, "sourceUnit": {"type": "CurrentTarget"}}
    });
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    assert_eq!(check_prepared(&prepared), Ok(()));
}

/// The rotation's potion action resolves to the combat potion, which has a behavior.
#[test]
fn potion_action_resolves_to_the_combat_potion() {
    let mut value = production();
    remove_effect(&mut value, "potion_mana");
    assert!(reasons(value).contains(&"rotation reaches item 13444 without a known behavior".into()));
}
