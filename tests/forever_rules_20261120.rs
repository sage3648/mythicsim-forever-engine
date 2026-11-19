//! Forever's rules from public Forever combat logs on forever.warcraftlogs.com of 18 to 20
//! November 2026 (client 1.60.1.70959), the live client rows and Wowhead's Forever tooltips:
//!
//! - A weapon's debuff row of the melee defense type is a strike that rolls the hit table:
//!   Puncture Armor (17315, Bashguuder and Rivenspike) logs a missed, dodged, parried or immune
//!   attempt as a damage event of no amount. 19 dungeon reports of 18 to 20 November: 2,873
//!   attempts (1,602 applications, 896 stacks, 260 refreshes with no stack beside them and 115
//!   failures), of which 2 missed, 76 were dodged, 34 parried and 3 met an immune target, 4.0%.
//!   Armor Shatter (16928) is a magic row and lands with the proc.
//! - Pendulum of Doom, Diabolic Skiver and Gut Ripper deal 50%, 50% and 100% more below 25%
//!   health, as their rows' second and third dummies and their tooltips state.
//! - Flame Wrath (11809) rolls 1.5 times a minute: 88 procs in 1,089 isolated landed main hand
//!   white swings of 8 wearers in 8 reports, 8.1% a swing on its 3.3 s speed.
//!
//! The Go reference keeps its own rules.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const PUNCTURE_ARMOR: i64 = 17315;
const GUT_RIPPER: i64 = 2164;
const WOUND: i64 = 18107;

/// The live Fury board request of 20 November 2026: Bashguuder in the off hand.
fn board(iterations: u32) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/board-warrior-20261120.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(iterations);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn run(prepared: &Value) -> Value {
    let prepared = serde_json::from_value(prepared.clone()).unwrap();
    simulate_prepared(&prepared).unwrap().result
}

fn spell_position(prepared: &Value, spell_id: i64) -> Option<usize> {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .position(|spell| spell["action_id"]["spell_id"] == spell_id)
}

fn armor_debuff(prepared: &Value) -> &Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "armor_debuff_proc")
        .expect("the weapon's armor debuff proc")
}

/// The player's metrics of one spell, summed over its targets.
fn metric(result: &Value, spell_id: i64, field: &str) -> f64 {
    result["raidMetrics"]["parties"][0]["players"][0]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"] == spell_id)
        .flat_map(|action| action["targets"].as_array().unwrap().iter())
        .map(|target| target[field].as_f64().unwrap_or(0.0))
        .sum()
}

/// Under Forever's rules Puncture Armor carries its strike, the client's row of the melee
/// defense type; the Go reference lands it with the proc.
#[test]
fn puncture_armor_is_a_strike() {
    let request = board(1);
    let forever = prepared(&request, Rules::Forever);
    let strike = spell_position(&forever, PUNCTURE_ARMOR).expect("the strike is registered");
    assert_eq!(armor_debuff(&forever)["strike_spell"], json!(strike));

    let go = prepared(&request, Rules::GoReference);
    assert!(armor_debuff(&go).get("strike_spell").is_none());
    assert_eq!(spell_position(&go, PUNCTURE_ARMOR), None);
}

/// The strike from behind a level 63 target is dodged, never parried, and a dodged one puts
/// nothing on the target: the attempts that land are the hits the strike counts.
#[test]
fn puncture_armor_strikes_are_dodged() {
    let result = run(&prepared(&board(200), Rules::Forever));
    let hits = metric(&result, PUNCTURE_ARMOR, "hits");
    let dodges = metric(&result, PUNCTURE_ARMOR, "dodges");
    let parries = metric(&result, PUNCTURE_ARMOR, "parries");
    assert!(hits > 1000.0, "{hits} landed strikes");
    assert_eq!(parries, 0.0);
    let failed = dodges / (hits + dodges);
    assert!(
        (0.03..0.10).contains(&failed),
        "{failed} of the strikes dodged"
    );
    assert_eq!(metric(&result, PUNCTURE_ARMOR, "damage"), 0.0);
}

/// Gut Ripper's Wound deals twice its damage below 25% health: the prepared spell carries the
/// row's 25 and 100, and the bonus raises the hits of the fight's last quarter.
#[test]
fn gut_ripper_doubles_below_a_quarter() {
    let mut request = board(300);
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"][12] =
        json!({"id": GUT_RIPPER});
    let forever = prepared(&request, Rules::Forever);
    let position = spell_position(&forever, WOUND).expect("the Wound is registered");
    assert_eq!(
        forever["player"]["spells"][position]["execute_damage_bonus"],
        json!([25.0, 2.0])
    );
    let go = prepared(&request, Rules::GoReference);
    let go_position = spell_position(&go, WOUND).expect("the Wound is registered");
    assert!(go["player"]["spells"][go_position]
        .get("execute_damage_bonus")
        .is_none());

    let with = run(&forever);
    let mut plain = forever.clone();
    plain["player"]["spells"][position]
        .as_object_mut()
        .unwrap()
        .remove("execute_damage_bonus");
    let without = run(&plain);
    let per_hit = |result: &Value| {
        metric(result, WOUND, "damage")
            / (metric(result, WOUND, "hits") + metric(result, WOUND, "crits"))
    };
    let ratio = per_hit(&with) / per_hit(&without);
    assert!((1.1..1.5).contains(&ratio), "{ratio}");
}

/// Flame Wrath rolls 1.5 times a minute of its 3.3 s speed under Forever's rules, 8.25% a hit;
/// the Go reference keeps its 1.0.
#[test]
fn flame_wrath_rolls_its_measured_rate() {
    let mut request = board(1);
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"][12] = json!({"id": 11809});
    let chance = |rules: Rules| {
        let prepared = prepared(&request, rules);
        let effect = prepared["effects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|effect| {
                effect["kind"] == "spell_data_damage_proc"
                    && effect["average"].as_f64() == Some(154.0)
            })
            .expect("Flame Wrath's proc")
            .clone();
        effect["chances"][0]["chance"].as_f64().unwrap()
    };
    assert!((chance(Rules::Forever) - 1.5 * 3.3 / 60.0).abs() < 1e-9);
    assert!((chance(Rules::GoReference) - 3.3 / 60.0).abs() < 1e-9);
}
