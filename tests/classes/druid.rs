//! Druid prepared v2 regressions. The Go goldens of every accepted Druid case, including the
//! production Feral (cat) Druid, are compared with the rest of the fixture family in
//! `mage/prepared_v2.rs`; these tests cover what the gate rejects and what the cat's forms
//! log.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, simulate_prepared, PreparedError,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn feral() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-feral-druid.prepared.json");
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

fn effect<'a>(value: &'a mut Value, kind: &str) -> &'a mut Value {
    value["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap()
}

fn first_fight_log(mut value: Value) -> String {
    value["sim"]["iterations"] = json!(1);
    let report = simulate_prepared(&parse(value)).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

#[test]
fn production_feral_druid_is_supported() {
    assert!(check_prepared(&parse(feral())).is_ok());
}

#[test]
fn only_moonkin_and_cat_starting_forms_are_supported() {
    let mut value = feral();
    effect(&mut value, "druid_forms")["starting_form"] = json!(["bear"]);
    assert!(reasons(value).contains(&"druid starting form [\"bear\"] is unsupported".to_string()));
}

#[test]
fn faerie_fire_readings_beyond_own_and_never_are_rejected() {
    let mut value = feral();
    effect(&mut value, "faerie_fire")["refresh"] = json!(["unknown"]);
    assert!(reasons(value).contains(
        &"Faerie Fire's armor reduction reads [\"unknown\"], which is not modeled".into()
    ));
}

#[test]
fn aura_should_refresh_needs_a_supported_reading() {
    let mut value = feral();
    effect(&mut value, "aura_should_refresh")["modes"] = json!(["unknown"]);
    let reasons = reasons(value);
    assert!(
        reasons.contains(
            &"auraShouldRefresh on target aura \"Faerie Fire (Player)\" has no supported exclusive effect reading"
                .to_string()
        ),
        "{reasons:?}"
    );
}

#[test]
fn blood_frenzy_on_other_outcomes_is_rejected() {
    let mut value = feral();
    effect(&mut value, "blood_frenzy")["outcome"] = json!(["Hit", "Crit"]);
    assert!(reasons(value).contains(&"Blood Frenzy procs on [\"Hit\", \"Crit\"]".to_string()));
}

#[test]
fn innervate_clears_the_form_and_cat_spells_then_fail_with_a_log() {
    let log = first_fight_log(feral());
    let innervate = log
        .find("Casting {SpellID: 29166}")
        .expect("the first fight casts Innervate");
    let before = &log[..innervate];
    assert!(before.ends_with(
        "Movement speed changed from 8.92 (27.50%) to 7.14 (2.00%)\n[103.30] [feral-druid (#1)] "
    ));
    assert!(log[innervate..].contains("[103.30] Failed cast to spell {SpellID: 9830}, wrong form"));
}

#[test]
fn threat_follows_the_form() {
    let log = first_fight_log(feral());
    // Faerie Fire's flat threat in Cat Form, and an auto attack's in caster form.
    assert!(log.contains("{SpellID: 9907} Hit for 0.000 damage (SpellSchool: 8). (Threat: 85.200)"));
    assert!(log.contains("Crit for 699.996 damage (SpellSchool: 1). (Threat: 699.996)"));
}
