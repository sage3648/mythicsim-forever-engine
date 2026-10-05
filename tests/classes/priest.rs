//! Priest prepared v2 regressions. The Go goldens of every accepted Priest case, including
//! the production Shadow Priest, are compared with the rest of the fixture family in
//! `mage/prepared_v2.rs`; these tests cover what the gate rejects and what the runtime
//! reports for the inert and the summoned Shadowfiend.

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

fn smite() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-smite-priest.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
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
fn production_smite_priest_is_supported() {
    assert!(check_prepared(&parse(smite())).is_ok());
}

#[test]
fn searing_light_outside_periodic_damage_is_rejected() {
    let mut value = smite();
    let searing = effect(&mut value, "searing_light");
    searing["callbacks"] = json!(["on_spell_hit_dealt"]);
    searing["outcome"] = json!(["Crit"]);
    let reasons = reasons(value);
    assert!(reasons.contains(&"Searing Light listens to [\"on_spell_hit_dealt\"]".to_string()));
    assert!(reasons.contains(&"Searing Light procs on [\"Crit\"]".to_string()));
}

#[test]
fn power_in_light_needs_a_burning_holy_fire() {
    // With no Holy Fire named, the modifier never applies and the build deals less.
    let mut value = smite();
    value["sim"]["iterations"] = json!(20);
    value["sim"]["debug_first_iteration"] = json!(false);
    let mut unlit = value.clone();
    effect(&mut unlit, "power_in_light")["holy_fire_spells"] = json!([]);
    let dps = |value: Value| {
        simulate_prepared(&parse(value)).unwrap().result["raidMetrics"]["dps"]["avg"]
            .as_f64()
            .unwrap()
    };
    assert!(dps(value) > dps(unlit));
}

fn accepted(case: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn holy_nova_reads_the_live_healing_power() {
    // Talisman of Ephemeral Power raises healing power for a while, and the heal follows it.
    let mut value = accepted("smite-priest-holy-nova-ephemeral-power");
    value["sim"]["iterations"] = json!(1);
    let report = simulate_prepared(&parse(value)).unwrap();
    let logs = report.result["logs"].as_str().unwrap();
    let mut powers: Vec<&str> = logs
        .lines()
        .filter(|line| line.contains("{SpellID: 27805} [DEBUG] HealingPower: "))
        .map(|line| line.split("HealingPower: ").nth(1).unwrap())
        .map(|rest| rest.split(',').next().unwrap())
        .collect();
    powers.dedup();
    assert!(powers.len() > 1, "{powers:?}");
}

#[test]
fn shadowfiend_inherits_attack_power_and_restores_mana() {
    let mut value = accepted("shadow-priest-shadowfiend");
    value["sim"]["iterations"] = json!(1);
    let report = simulate_prepared(&parse(value)).unwrap();
    let logs = report.result["logs"].as_str().unwrap();
    assert!(logs
        .lines()
        .any(|line| line.contains("- Shadowfiend]") && line.ends_with("Pet summoned")));
    assert!(logs
        .lines()
        .any(|line| line.contains("Pet inherited stats: {\"AttackPower\": ")));
    assert!(logs
        .lines()
        .any(|line| line.contains("mana from {SpellID: 401988}")));
}

#[test]
fn a_shadowfiend_effect_for_another_pet_is_rejected() {
    let mut value = accepted("shadow-priest-shadowfiend");
    effect(&mut value, "shadowfiend")["pet"] = json!("another pet");
    let reasons = reasons(value);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.starts_with("pet ") && reason.ends_with("has no behavior")),
        "{reasons:?}"
    );
}

fn shadow_against(targets: u32) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/mage/prepared-v2/production-shadow-priest-{targets}-targets.prepared.json"
    ));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The application's multidot puts Shadow Word: Pain on every target, each with its own dot,
/// while Mind Flay stays on the first.
#[test]
fn multidot_keeps_shadow_word_pain_on_every_target() {
    let mut value = shadow_against(5);
    value["sim"]["iterations"] = json!(1);
    value["sim"]["debug_first_iteration"] = json!(true);
    let report = simulate_prepared(&parse(value)).unwrap();
    let logs = report.result["logs"].as_str().unwrap();
    for target in 1..=5 {
        let ticks = logs
            .lines()
            .filter(|line| line.contains(&format!("[Target {target}] {{SpellID: 10894}} tick")))
            .count();
        assert!(ticks > 0, "Target {target}: no Shadow Word: Pain ticks");
    }
    assert!(!logs
        .lines()
        .any(|line| line.contains("[Target 2] {SpellID: 18807}")));
}

/// The exporter writes a target count only for two to five targets.
#[test]
fn target_counts_outside_the_contract_are_refused() {
    for count in [1, 6] {
        let mut value = production();
        value["encounter"]["target_count"] = json!(count);
        assert!(reasons(value).contains(&format!(
            "target count {count}: a fight has one target, or from 2 to 5 copies of it"
        )));
    }
}
