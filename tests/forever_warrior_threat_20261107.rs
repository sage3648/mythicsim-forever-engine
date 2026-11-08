//! Warrior threat measurements at levels 20 to 24 (in game, 7 November 2026), under Forever's
//! rules. The Go reference keeps its own numbers, so every check also reads the reference.
//!
//! The rage of an avoided hit (Shield Specialization on a block, Master of Defense on a dodge or
//! parry) makes 5 threat a point in Battle and Defensive Stance alike, and Defiance does not
//! scale it; Cleave rank 1 adds 30 threat beyond its damage on one target.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/warrior_trees_20261104.rs"]
mod warrior_trees_20261104;

const BATTLE_STANCE: i64 = 2457;
const DEFENSIVE_STANCE: i64 = 71;
/// The energize spells of Shield Specialization and Master of Defense.
const SHIELD_SPECIALIZATION_RAGE: i64 = 1310318;
const MASTER_OF_DEFENSE_RAGE: i64 = 23602;

const CLEAVE_RANK_1: i64 = 845;
const CLEAVE_RANK_5: i64 = 20569;

fn recorded(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/mage/prepared-v2/production-{name}.request.json"
    ));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The production request with its talents in the client's order, which Forever's rules read.
fn production(name: &str) -> Value {
    let mut request = recorded(name);
    warrior_trees_20261104::forever_request(&mut request);
    request
}

/// The request as the rules read its talents: the recorded string is the reference's.
fn production_for(name: &str, rules: Rules) -> Value {
    match rules {
        Rules::Forever => production(name),
        Rules::GoReference => recorded(name),
    }
}

fn player_mut(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "warrior", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

/// A level 24 Fury warrior on the production request's gear: it knows Cleave rank 1, Hamstring
/// rank 1 and Shield Bash rank 1, and none of the second ranks. Its character sheet is the
/// numbers a level 24 warrior of the measurements reads.
fn levelling_warrior() -> Value {
    let mut request = production("warrior");
    let player = player_mut(&mut request);
    player["level"] = json!(24);
    player["talentsString"] = json!("");
    player["characterSheet"] = json!({
        "attributes": [59, 40, 55, 20, 25],
        "base": [46, 35, 43, 17, 22],
        "health": 800,
        "level": 24,
        "mana": 0,
        "meleeCrit": 5.0,
        "spellCrit": [0, 0, 0, 0, 0, 0],
    });
    request
}

/// The exported spell with this spell id and no tag.
fn spell(prepared: &Value, spell_id: i64) -> &Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| {
            spell["action_id"]["spell_id"].as_i64() == Some(spell_id)
                && spell["action_id"]["tag"].as_i64().unwrap_or(0) == 0
        })
        .unwrap_or_else(|| panic!("spell {spell_id} is not exported"))
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

/// The player's metrics of a run of the prepared request.
fn run(prepared: &Value) -> Value {
    let typed: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    check_prepared(&typed).unwrap_or_else(|error| panic!("{error:?}"));
    let result = simulate_prepared(&typed).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0].clone()
}

/// A rotation that casts only these (a stance first), so nothing else gains rage or deals damage.
fn rotation(stance: i64, casts: &[i64]) -> Value {
    json!({
        "type": "TypeAPL",
        "prepullActions": [{
            "action": {"castSpell": {"spellId": {"spellId": stance}}},
            "doAtValue": {"const": {"val": "-1s"}},
        }],
        "priorityList": casts
            .iter()
            .map(|id| json!({"action": {"castSpell": {"spellId": {"spellId": id}}}}))
            .collect::<Vec<_>>(),
    })
}

/// The rage the player's resource row with this spell id actually gained.
fn rage_gained(player: &Value, spell_id: i64) -> f64 {
    player["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["id"]["spellId"].as_i64() == Some(spell_id))
        .map(|row| row["actualGain"].as_f64().unwrap_or(0.0))
        .sum()
}

/// The threat the rage gain action booked, over all targets.
fn rage_threat(player: &Value) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["otherId"] == "OtherActionRageGain")
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["threat"].as_f64().unwrap_or(0.0))
        .sum()
}

/// A tank on Shield Specialization, Master of Defense and Defiance 3 that casts nothing but a
/// stance, so the only rage with threat is what its avoided hits give.
fn rage_on_avoid_threat_per_rage(stance: i64, rules: Rules) -> f64 {
    let mut request = production("protection-warrior");
    let player = player_mut(&mut request);
    player["talentsString"] = json!("--250033120310021251");
    player["rotation"] = rotation(stance, &[]);
    request["simOptions"]["iterations"] = json!(300);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    let metrics = run(&prepared(&request, rules));
    let gained = rage_gained(&metrics, SHIELD_SPECIALIZATION_RAGE)
        + rage_gained(&metrics, MASTER_OF_DEFENSE_RAGE);
    assert!(gained > 100.0, "{gained} rage from avoided hits");
    rage_threat(&metrics) / gained
}

/// Master of Defense's rage, like Shield Specialization's, makes 5 threat a point in Battle and
/// Defensive Stance alike, and Defiance does not scale it. The reference multiplies it by the
/// warrior's threat multiplier at the gain.
#[test]
fn rage_from_an_avoided_hit_makes_five_threat_in_any_stance() {
    for stance in [BATTLE_STANCE, DEFENSIVE_STANCE] {
        let forever = rage_on_avoid_threat_per_rage(stance, Rules::Forever);
        assert!((forever - 5.0).abs() < 1e-9, "stance {stance}: {forever}");
        let reference = rage_on_avoid_threat_per_rage(stance, Rules::GoReference);
        assert!(
            (reference - 5.0).abs() > 0.5,
            "stance {stance}: the reference makes {reference}"
        );
    }
}

fn rage_on_avoid_triggers(prepared: &Value) -> Vec<Value> {
    let effect = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "rage_on_avoid")
        .expect("a rage_on_avoid effect");
    effect["triggers"].as_array().unwrap().clone()
}

#[test]
fn only_forever_marks_the_avoid_rage_unscaled() {
    let mut request = production("protection-warrior");
    player_mut(&mut request)["talentsString"] = json!("--250033120310021251");
    let forever = rage_on_avoid_triggers(&prepared(&request, Rules::Forever));
    assert_eq!(forever.len(), 2);
    assert!(forever.iter().all(|t| t["unscaled_threat"] == json!(true)));
    let reference = rage_on_avoid_triggers(&prepared(&request, Rules::GoReference));
    assert_eq!(reference.len(), 2);
    assert!(reference.iter().all(|t| t.get("unscaled_threat").is_none()));
}

/// Cleave rank 1 adds 30 threat beyond its damage on one target (about 15 on each of two). The
/// per level figure of the other ranks would give 30.6 at its level 20, and rank 5 keeps it.
#[test]
fn cleave_rank_1_adds_thirty_threat() {
    let request = levelling_warrior();
    let forever = prepared(&request, Rules::Forever);
    assert_eq!(
        number(&spell(&forever, CLEAVE_RANK_1)["flat_threat_bonus"]),
        30.0
    );
    let reference = prepared(&request, Rules::GoReference);
    let flat = number(&spell(&reference, CLEAVE_RANK_1)["flat_threat_bonus"]);
    assert!((flat - 1.53 * 20.0).abs() < 1e-9, "{flat}");

    for rules in [Rules::Forever, Rules::GoReference] {
        let at_60 = production_for("warrior", rules);
        let flat = number(&spell(&prepared(&at_60, rules), CLEAVE_RANK_5)["flat_threat_bonus"]);
        assert!((flat - 1.53 * 60.0).abs() < 1e-9, "{flat}");
    }
}
