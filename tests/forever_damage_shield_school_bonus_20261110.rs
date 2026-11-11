//! A damage shield takes none of the target's flat school damage bonus under Forever's rules.
//! Levels 1 to 19 logs (client 1.60.1.70790) show 9 rank 1 Retribution Aura hits on beasts under
//! the paladin's own rank 2 Judgement of the Crusader, all 5 or 6: the flat 5 at a Dwarf's 5%
//! against beasts, with nothing from the judgement. The reference adds the judgement's Holy bonus
//! times the shield's spell power coefficient (0.06) to every hit.
//!
//! The request is the tank of `fixtures/forever-gaps/protection-paladin-retribution-aura`, whose
//! own Retribution Aura rank 2 (10298) strikes back at the target's landed swings (tag 2), with
//! and without the raid's Judgement of the Crusader on the target.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::Value;
use std::{fs, path::Path};

#[path = "support/paladin_113554.rs"]
mod paladin_113554;

const RETRIBUTION_AURA: i64 = 10298;
const SHIELD_TAG: i64 = 2;

fn request(crusader: bool) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/forever-gaps/protection-paladin-retribution-aura.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    paladin_113554::forever_request(&mut request);
    request["raid"]["debuffs"]["judgementOfTheCrusader"] = Value::Bool(crusader);
    request
}

fn prepared(crusader: bool, rules: Rules) -> Value {
    prepare_json_with(request(crusader).to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// Whether the shield's spell carries the flag.
fn shield_flagged(prepared: &Value) -> bool {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| {
            spell["action_id"]["spell_id"].as_i64() == Some(RETRIBUTION_AURA)
                && spell["action_id"]["tag"].as_i64() == Some(SHIELD_TAG)
        })
        .expect("the shield's spell")["flags"]
        .as_array()
        .unwrap()
        .contains(&Value::from("SpellFlagIgnoreTargetSchoolBonus"))
}

/// The shield's damage a hit, from the run's totals: a shield never crits and deals its fixed
/// amount, so every landed hit is the same.
fn damage_a_hit(prepared: Value) -> f64 {
    let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
    check_prepared(&typed).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&typed).unwrap().result;
    let player = &result["raidMetrics"]["parties"][0]["players"][0];
    let (mut hits, mut damage) = (0.0, 0.0);
    for action in player["actions"].as_array().unwrap() {
        let id = &action["id"];
        if id["spellId"].as_i64() != Some(RETRIBUTION_AURA)
            || id["tag"].as_i64().unwrap_or(0) != SHIELD_TAG
        {
            continue;
        }
        for target in action["targets"].as_array().unwrap() {
            hits += target["hits"].as_f64().unwrap_or(0.0);
            damage += target["damage"].as_f64().unwrap_or(0.0);
        }
    }
    assert!(hits > 0.0, "the shield strikes back");
    damage / hits
}

#[test]
fn retribution_aura_takes_nothing_from_judgement_of_the_crusader() {
    let with = prepared(true, Rules::Forever);
    assert!(shield_flagged(&with));
    let with = damage_a_hit(with);
    let without = damage_a_hit(prepared(false, Rules::Forever));
    assert!(
        (with - without).abs() < 1e-9,
        "with the judgement {with}, without {without}"
    );
}

/// The flag is what keeps the judgement off: the same shield without it gains the judgement's
/// Holy bonus at its 0.06 coefficient, as the reference's shields do. (The reference's rules
/// cannot run a tank's own Retribution Aura cast, so the control removes the flag instead.)
#[test]
fn without_the_flag_the_judgement_adds_to_the_shield() {
    let mut with = prepared(true, Rules::Forever);
    let mut without = prepared(false, Rules::Forever);
    for prepared in [&mut with, &mut without] {
        for spell in prepared["player"]["spells"].as_array_mut().unwrap() {
            if let Some(flags) = spell["flags"].as_array_mut() {
                flags.retain(|flag| flag != "SpellFlagIgnoreTargetSchoolBonus");
            }
        }
    }
    let (with, without) = (damage_a_hit(with), damage_a_hit(without));
    assert!(with > without + 1.0, "with {with}, without {without}");
}

/// The reference's shields carry no flag.
#[test]
fn the_reference_does_not_flag_its_shields() {
    let reference = prepared(true, Rules::GoReference);
    let flagged = reference["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .any(|spell| {
            spell["flags"]
                .as_array()
                .unwrap()
                .contains(&Value::from("SpellFlagIgnoreTargetSchoolBonus"))
        });
    assert!(!flagged);
}
