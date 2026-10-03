//! Priest prepared v2 regressions. The Go goldens of every accepted Priest case, including
//! the production Shadow Priest, are compared with the rest of the fixture family in
//! `mage/prepared_v2.rs`; these tests cover what the gate rejects and what the runtime
//! reports for the inert Shadowfiend.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, simulate_prepared, PreparedError,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn production() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-shadow-priest.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn parse(value: Value) -> PreparedV2 {
    serde_json::from_value(value).unwrap()
}

fn reasons(value: Value) -> Vec<String> {
    match check_prepared(&parse(value)) {
        Err(PreparedError::Unsupported(reasons)) => reasons,
        other => panic!("expected unsupported, got {other:?}"),
    }
}

/// The rotation item holding an action, by its operator.
fn item<'a>(value: &'a mut Value, action: &str) -> &'a mut Value {
    value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|item| item["action"].get(action).is_some())
        .unwrap()
}

#[test]
fn production_shadow_priest_is_supported() {
    assert!(check_prepared(&parse(production())).is_ok());
}

#[test]
fn inert_shadowfiend_is_dismissed_logged_and_reported() {
    let mut value = production();
    value["sim"]["iterations"] = json!(3);
    let report = simulate_prepared(&parse(value)).unwrap();
    let player = &report.result["raidMetrics"]["parties"][0]["players"][0];
    let pet = &player["pets"][0];
    assert_eq!(pet["name"], "Shadowfiend");
    assert_eq!(pet["unitIndex"], 2);
    assert_eq!(pet["auras"][0]["aggregatorData"]["n"], 3);
    // Every action lists the target, the player and the pet.
    for action in player["actions"].as_array().unwrap() {
        assert_eq!(action["targets"].as_array().unwrap().len(), 3);
    }
    let logs = report.result["logs"].as_str().unwrap();
    let pet_lines: Vec<&str> = logs
        .lines()
        .filter(|line| line.contains("- Shadowfiend]"))
        .collect();
    assert_eq!(pet_lines.len(), 3, "{pet_lines:?}");
    assert!(pet_lines[0].ends_with("Pet dismissed"));
    assert!(pet_lines[2].ends_with("No pet summoned"));
}

#[test]
fn a_pet_that_may_act_is_rejected() {
    let mut value = production();
    let effects = value["effects"].as_array_mut().unwrap();
    effects.retain(|effect| effect["kind"] != "inert_pet");
    value["unrepresented"] = json!(["pets are unsupported"]);
    assert_eq!(
        reasons(value),
        ["unrepresented by the exporter: pets are unsupported"]
    );
}

#[test]
fn interrupt_conditions_follow_the_622_guard() {
    let mut value = production();
    let channel = &mut item(&mut value, "channelSpell")["action"]["channelSpell"];
    channel["interruptIf"] = json!({"and": {"vals": [
        channel["interruptIf"].clone(),
        {"auraIsActive": {"auraId": {"spellId": 44404}}},
    ]}});
    let reasons = reasons(value);
    assert!(
        reasons.iter().any(|reason| reason.starts_with(
            "rotation item 9: auraIsActive names spell 44404, which the character lacks"
        )),
        "{reasons:?}"
    );
}

#[test]
fn unsupported_sequence_steps_and_channel_targets_are_named() {
    let mut value = production();
    let sequence = &mut item(&mut value, "strictSequence")["action"]["strictSequence"];
    sequence["actions"][0] = json!({"wait": {"duration": {"const": {"val": "1s"}}}});
    let channel = &mut item(&mut value, "channelSpell")["action"]["channelSpell"];
    channel["target"] = json!({"type": "Self"});
    let reasons = reasons(value);
    assert!(reasons.contains(&"rotation item 6: strictSequence action wait is unsupported".into()));
    assert!(reasons.contains(&"rotation item 9: channelSpell field target is unsupported".into()));
}

#[test]
fn shadow_weaving_outside_spell_hits_is_rejected() {
    let mut value = production();
    let weaving = value["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "shadow_weaving")
        .unwrap();
    weaving["callbacks"] = json!(["on_periodic_damage_dealt"]);
    assert!(reasons(value)
        .contains(&"Shadow Weaving listens to [\"on_periodic_damage_dealt\"]".to_string()));
}
