//! Shaman tests: the prepared v2 gate on the production Elemental Shaman request. Its Go
//! result and first-fight log are compared with the rest of the fixture family.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepared_coverage, PreparedError,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn elemental_json() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-elemental-shaman.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn reasons(value: Value) -> Vec<String> {
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    match check_prepared(&prepared) {
        Err(PreparedError::Unsupported(reasons)) => reasons,
        other => panic!("expected unsupported, got {other:?}"),
    }
}

#[test]
fn production_elemental_request_is_supported() {
    let prepared: PreparedV2 = serde_json::from_value(elemental_json()).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
}

/// The rotation's Magma Totem and its Chain Lightning on several targets wait for two
/// targets, which never happens with the one target in scope, so Magma Totem needs no
/// behavior. Without that condition it does.
#[test]
fn magma_totem_is_reachable_only_without_its_target_count_condition() {
    let mut value = elemental_json();
    let item = &mut value["player"]["rotation"]["priorityList"][1]["action"];
    assert_eq!(item["castSpell"]["spellId"]["spellId"], 10587);
    item.as_object_mut().unwrap().remove("condition");
    assert_eq!(
        reasons(value),
        ["rotation reaches spell 10587 without a known behavior"]
    );
}

#[test]
fn elemental_focus_listener_needs_its_effect() {
    let mut value = elemental_json();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "elemental_focus");
    assert!(reasons(value).contains(
        &"player aura \"Elemental Focus\" listens to combat events without an effect".into()
    ));
}

/// Go panics when a product with a float reads a non-constant integer as a float.
#[test]
fn math_reading_an_operand_as_another_type_is_unsupported() {
    let mut value = elemental_json();
    value["player"]["rotation"]["priorityList"][6]["action"]["condition"] = json!({"cmp": {
        "op": "OpGe",
        "lhs": {"currentMana": {}},
        "rhs": {"math": {"op": "OpMul", "lhs": {"numberTargets": {}}, "rhs": {"currentManaPercent": {}}}},
    }});
    assert!(reasons(value).contains(
        &"rotation item 7: math that reads an operand as another type is unsupported".into()
    ));
}

fn enhancement_json() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/enhancement-shaman-no-battle-shout.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn production_enhancement_request_is_supported() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-enhancement-shaman.prepared.json");
    let prepared: PreparedV2 = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
}

#[test]
fn enhancement_request_without_battle_shout_is_supported() {
    let prepared: PreparedV2 = serde_json::from_value(enhancement_json()).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
}

/// Strength of Earth Totem is the only basic totem with a behavior.
#[test]
fn other_basic_totems_are_rejected() {
    let mut value = enhancement_json();
    let item = &mut value["player"]["rotation"]["priorityList"][0]["action"];
    assert_eq!(item["castSpell"]["spellId"]["spellId"], 25361);
    item["castSpell"]["spellId"]["spellId"] = json!(25359);
    assert!(reasons(value)
        .contains(&"rotation reaches spell 25359, a totem without a known behavior".into()));
}

/// Go reads totem slots only on a Shaman.
#[test]
fn totem_remaining_time_needs_a_shaman() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/frost-reference.prepared.json");
    let mut value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    value["player"]["rotation"]["priorityList"][0]["action"]["condition"] = json!({"cmp": {
        "op": "OpLe",
        "lhs": {"totemRemainingTime": {"totemType": "Fire"}},
        "rhs": {"const": {"val": "0s"}},
    }});
    assert!(reasons(value).contains(&"rotation item 1: totemRemainingTime needs a Shaman".into()));
}
