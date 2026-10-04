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
fn a_starting_form_needs_its_form_effect() {
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

fn bear() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-feral-bear-druid.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn production_feral_bear_druid_is_supported() {
    assert!(check_prepared(&parse(bear())).is_ok());
}

#[test]
fn a_bear_rotation_may_leave_bear_form_and_shift_back() {
    // Innervate clears the form; the bear casts Bear Form again once out of it.
    let mut value = bear();
    let list = value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    let not_bear = json!({"not": {"val": {"auraIsActive": {"auraId": {"spellId": 9634}}}}});
    list.insert(
        0,
        json!({"action": {"castSpell": {"spellId": {"spellId": 29166}}}}),
    );
    list.insert(
        0,
        json!({"action": {"castSpell": {"spellId": {"spellId": 9634}}, "condition": not_bear}}),
    );
    assert!(check_prepared(&parse(value.clone())).is_ok());
    let log = first_fight_log(value);
    let faded = log
        .find("Aura faded: {SpellID: 9634}")
        .expect("Innervate clears the form");
    let cast = log[faded..]
        .find("Casting {SpellID: 9634}")
        .expect("the bear shifts back");
    assert!(log[faded + cast..].contains("Aura gained: {SpellID: 9634}"));
}

#[test]
fn natural_reaction_on_other_outcomes_is_rejected() {
    let mut value = bear();
    effect(&mut value, "natural_reaction")["outcome"] = json!(["Parry"]);
    assert!(reasons(value).contains(&"Natural Reaction procs on [\"Parry\"]".to_string()));
}

#[test]
fn a_second_attack_power_aura_is_rejected() {
    let mut value = bear();
    let auras = value["enemy"]["attack_power_auras"].as_array_mut().unwrap();
    let copy = auras[0].clone();
    auras.push(copy);
    let reasons = reasons(value);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.ends_with("change the target's attack power together")),
        "{reasons:?}"
    );
}

#[test]
fn demoralizing_roar_lowers_the_target_swing() {
    let log = first_fight_log(bear());
    let roar = log
        .find("[Target 1] Aura gained: {SpellID: 9898}")
        .expect("the first fight roars");
    let swing = "[Target 1] [feral-bear-druid (#1)] {OtherID: 3, Tag: 1} [DEBUG] MAP: 600.0";
    assert!(log[roar..].contains(swing));
    assert!(!log[..roar].contains("MAP: 600.0"));
}

#[test]
fn leaving_bear_form_at_the_end_keeps_the_health_fraction() {
    // A bear alive when the fight ends loses health with the form's maximum; a dead one has
    // none to lose.
    let mut alive = bear();
    alive["encounter"]["duration_ns"] = json!(4_000_000_000_i64);
    alive["encounter"]["duration_variation_ns"] = json!(0);
    let in_form = alive["player"]["stats"]["Health"].as_f64().unwrap();
    let after_form = |log: &str| {
        let faded = log
            .find("Aura faded: {SpellID: 9634}")
            .expect("the form fades");
        log[faded..].lines().nth(1).unwrap().to_string()
    };
    let line = after_form(&first_fight_log(alive));
    let total: f64 = line
        .rsplit("of ")
        .next()
        .and_then(|rest| rest.strip_suffix(" total."))
        .and_then(|number| number.parse().ok())
        .expect("a health line");
    assert!(line.contains("Spent ") && total < in_form, "{line}");
    let line = after_form(&first_fight_log(bear()));
    assert!(!line.contains(" health "), "{line}");
}

/// Wildheart Raiment's five piece Nature's Bounty hears only melee hits the player takes, so
/// it is inert on a cat nothing attacks; without the description it is refused.
#[test]
fn wildheart_raiment_is_inert_on_a_cat() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/feral-druid-wildheart-raiment.prepared.json");
    let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(check_prepared(&parse(value.clone())), Ok(()));
    let mut refused = value;
    refused["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["aura"] != "Wildheart Raiment 5P");
    assert!(reasons(refused).contains(
        &"player aura \"Wildheart Raiment 5P\" listens to combat events without an effect".into()
    ));
}

fn accepted(case: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// A tank's absorb proc hears the target's swings, so it needs its effect.
#[test]
fn absorb_procs_need_their_effect() {
    let mut value = accepted("feral-bear-druid-uthers-strength");
    assert!(check_prepared(&parse(value.clone())).is_ok());
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "spell_data_absorb_proc");
    assert!(reasons(value)
        .contains(&"player aura \"Uther's Strength\" reacts to the target's swings".into()));
}

/// Enchant Chest - Absorption's 25% chance waits out the trigger's 5 second cooldown.
#[test]
fn absorption_enchant_waits_for_its_cooldown() {
    let value = accepted("feral-bear-druid-absorption");
    // The shortest time between two shields in the first fight.
    let shortest_gap = |value: Value| {
        let times: Vec<f64> = first_fight_log(value)
            .lines()
            .filter(|line| line.ends_with("Aura gained: {SpellID: 1249073}"))
            .map(|line| line[1..line.find(']').unwrap()].parse().unwrap())
            .collect();
        times
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .fold(f64::INFINITY, f64::min)
    };
    let mut uncooled = value.clone();
    for aura in uncooled["player"]["auras"].as_array_mut().unwrap() {
        if aura["label"] == "Enchant Chest - Absorption" {
            aura["icd"] = Value::Null;
        }
    }
    assert!(shortest_gap(value) >= 5.0);
    assert!(shortest_gap(uncooled) < 5.0);
}
