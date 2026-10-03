//! Rogue tests: the prepared v2 gate on the production Combat Rogue request. Its Go result and
//! first-fight log are compared with the rest of the fixture family.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepared_coverage, PreparedError,
};
use serde_json::Value;
use std::{fs, path::Path};

fn combat_json() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-combat-rogue.prepared.json");
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
fn production_combat_request_is_supported() {
    let prepared: PreparedV2 = serde_json::from_value(combat_json()).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
}

#[test]
fn poison_listener_needs_its_effect() {
    let mut value = combat_json();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "deadly_poison");
    assert!(reasons(value).contains(
        &"player aura \"Deadly Poison\" listens to combat events without an effect".into()
    ));
}

/// Energy and combo point values need the energy bar Go would read them from.
#[test]
fn energy_values_need_an_energy_bar() {
    let mut value = combat_json();
    value["player"].as_object_mut().unwrap().remove("energy");
    let reasons = reasons(value);
    assert!(reasons
        .iter()
        .any(|reason| reason.ends_with("reads energy or combo points, which the player lacks")));
    assert!(reasons
        .iter()
        .any(|reason| reason.ends_with("which costs energy the player lacks")));
}

/// Once the sapper can hit the player, Chance of Death acts and needs its effect.
#[test]
fn chance_of_death_acts_once_the_player_can_take_damage() {
    let mut value = combat_json();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "chance_of_death");
    assert!(reasons(value).contains(
        &"player aura \"Chance of Death\" listens to combat events without an effect".into()
    ));
}

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{name}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn production_assassination_and_subtlety_requests_are_supported() {
    for name in [
        "production-assassination-rogue",
        "production-subtlety-rogue",
    ] {
        let prepared: PreparedV2 = serde_json::from_value(fixture(name)).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{name}");
    }
}

/// Each talent proc trigger is claimed by its own effect.
#[test]
fn talent_proc_triggers_need_their_effects() {
    let mut value = fixture("production-subtlety-rogue");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "rogue_proc");
    let reasons = reasons(value);
    for label in [
        "Initiative Trigger",
        "Cutthroat Trigger",
        "Thousand Cuts Trigger",
    ] {
        assert!(reasons.contains(&format!(
            "player aura \"{label}\" listens to combat events without an effect"
        )));
    }
}
