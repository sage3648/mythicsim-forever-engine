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

/// Expose Armor sets the target's armor through the major armor category, unless a permanent
/// member blocks it for good.
#[test]
fn expose_armor_needs_the_armor_category() {
    for name in [
        "combat-expose-armor",
        "combat-improved-expose-armor",
        "combat-expose-armor-raid-expose",
    ] {
        let prepared: PreparedV2 = serde_json::from_value(fixture(name)).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{name}");
    }
    let mut value = fixture("combat-expose-armor");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "exclusive_category");
    assert!(reasons(value)
        .iter()
        .any(|reason| reason.ends_with("without the target's armor category")));
}

/// Against a target that tanks the player, Kidney Shot's stun pauses its swings, Ghostly
/// Strike's dodge is a stat aura in its rolls and parries ready Riposte, whose trigger needs
/// its effect.
#[test]
fn tanking_rogue_handles_stuns_and_parries() {
    for name in [
        "combat-kidney-shot-tank",
        "combat-riposte-tank",
        "subtlety-ghostly-strike-tank",
    ] {
        let prepared: PreparedV2 = serde_json::from_value(fixture(name)).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{name}");
    }
    let mut value = fixture("combat-riposte-tank");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "riposte");
    let reasons = reasons(value);
    assert!(
        reasons.contains(&"player aura \"Riposte Trigger\" reacts to the target's swings".into())
    );
}

/// The pinned engine registers neither Evasion nor Sprint, so a rotation or cooldown timing that
/// names them reaches nothing, and Vanish casts from the rotation while tanking.
#[test]
fn defensive_cooldowns_reach_only_vanish() {
    let value = fixture("combat-rogue-defensive-cooldowns-tank");
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
    let ids: Vec<i32> = prepared
        .player
        .spells
        .iter()
        .filter_map(|spell| spell.action_id.as_ref().map(|id| id.spell_id))
        .collect();
    assert!(ids.contains(&1856));
    assert!(!ids.contains(&5277) && !ids.contains(&11305));
}

fn against(spec: &str, targets: u32) -> Value {
    fixture(&format!("production-{spec}-rogue-{targets}-targets"))
}

fn first_fight_log(mut value: Value) -> String {
    value["sim"]["iterations"] = serde_json::json!(1);
    value["sim"]["debug_first_iteration"] = serde_json::json!(true);
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    let report = forever_engine::simulate_prepared(&prepared).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

/// Blade Flurry's extra hit lands on the target after the one each melee hit struck. Only the
/// first target takes melee hits, and the extra hit has no proc mask to strike again, so the
/// second target takes every one.
#[test]
fn blade_flurry_strikes_the_next_target() {
    let logs = first_fight_log(against("combat", 3));
    assert!(logs
        .lines()
        .any(|line| line.contains("[Target 2] {SpellID: 22482} Hit")));
    assert!(!logs
        .lines()
        .any(|line| line.contains("[Target 1] {SpellID: 22482}")));
}

/// The Goblin Sapper Charge and Dragonbreath Chili hit every target, though a Subtlety Rogue's
/// rotation gains no AoE lines.
#[test]
fn area_consumables_reach_every_target() {
    let logs = first_fight_log(against("subtlety", 5));
    for target in 1..=5 {
        for id in ["{ItemID: 10646}", "{SpellID: 15851}"] {
            assert!(
                logs.lines()
                    .any(|line| line.contains(&format!("[Target {target}] {id}"))),
                "Target {target}: no {id}"
            );
        }
    }
}
