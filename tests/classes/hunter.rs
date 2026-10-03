//! Hunter prepared v2 regressions. The Go goldens of the production Marksmanship Hunter are
//! compared with the rest of the fixture family in `mage/prepared_v2.rs`; these tests pin the
//! ranged auto attack, the ranged cast time, physical ticks and what the gate rejects.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, simulate_prepared, PreparedError,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn production() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-marksmanship-hunter.prepared.json");
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

/// The first fight's log of a short run.
fn first_fight_log(mut value: Value) -> String {
    value["sim"]["iterations"] = json!(1);
    value["sim"]["debug_first_iteration"] = json!(true);
    let report = simulate_prepared(&parse(value)).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

fn player_line<'a>(logs: &'a str, at: &str, text: &str) -> Option<&'a str> {
    logs.lines().find(|line| {
        line.starts_with(at) && line.contains("[marksmanship-hunter (#1)]") && line.contains(text)
    })
}

#[test]
fn production_marksmanship_hunter_is_supported() {
    assert!(check_prepared(&parse(production())).is_ok());
}

/// Go attack.go: the first Auto Shot fires at the pull and lands after travel, 12 yards at
/// missile speed 40.
#[test]
fn auto_shot_fires_at_the_pull_and_lands_after_travel() {
    let logs = first_fight_log(production());
    assert!(
        player_line(&logs, "[0.00]", "Casting {OtherID: 4}").is_some(),
        "{logs}"
    );
    assert!(
        player_line(&logs, "[0.30]", "{OtherID: 4} Hit for").is_some()
            || player_line(&logs, "[0.30]", "{OtherID: 4} Crit for").is_some()
            || player_line(&logs, "[0.30]", "{OtherID: 4} Miss").is_some(),
        "{logs}"
    );
}

/// A ranged weapon out of range never starts its swing.
#[test]
fn a_ranged_weapon_out_of_range_never_shoots() {
    let mut value = production();
    value["melee"]["ranged"]["max_range"] = json!(10.0);
    let logs = first_fight_log(value);
    assert!(!logs.contains("{OtherID: 4}"), "{logs}");
}

/// Go hunter.go RegisterRangedSpell: Aimed Shot's two second cast divides by the ranged haste
/// multiplier, the quiver's 15%, unrounded.
#[test]
fn aimed_shot_casts_over_the_ranged_hasted_time() {
    let logs = first_fight_log(production());
    let line =
        player_line(&logs, "[-2.50]", "Casting {SpellID: 20904}").expect("prepull Aimed Shot");
    assert!(line.contains("Cast Time = 1.739130434s"), "{line}");
}

/// Go spell_resistances.go: a hawk's physical tick ignores armor.
#[test]
fn a_hawk_tick_ignores_armor() {
    let logs = first_fight_log(production());
    let line = logs
        .lines()
        .find(|line| line.contains("{SpellID: 1293527, Tag: 1} [DEBUG]"))
        .expect("a hawk tick");
    let stage = |name: &str| {
        let start = line.find(name).unwrap() + name.len();
        line[start..].split(',').next().unwrap().to_string()
    };
    assert_eq!(
        stage("AfterAttackerMods:"),
        stage("AfterResistances:"),
        "{line}"
    );
}

#[test]
fn an_unknown_auto_attack_type_is_rejected() {
    let mut value = production();
    let list = value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    list[1]["action"]["condition"]["and"]["vals"][0]["cmp"]["lhs"]["autoTimeToNext"]["autoType"] =
        json!("SomeAuto");
    let reasons = reasons(value);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("autoTimeToNext autoType")),
        "{reasons:?}"
    );
}

#[test]
fn a_magic_hit_sting_tick_is_rejected() {
    let mut value = production();
    for effect in value["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "serpent_sting" {
            effect["tick_outcome"] = json!("magic_hit");
        }
    }
    assert_eq!(reasons(value), ["Serpent Sting ticks with magic_hit"]);
}

#[test]
fn a_replaceable_main_hand_swing_in_range_is_rejected() {
    let mut value = production();
    value["unrepresented"] = json!(["main hand swings can be replaced"]);
    assert_eq!(
        reasons(value),
        ["unrepresented by the exporter: main hand swings can be replaced"]
    );
}
