//! The fight side of four Tier 1 5 piece bonuses (ItemSets 2100, 2098 and 2105, client
//! 1.60.1.70425). No request reaches five pieces (an item goes to the slot its type names), so the
//! unit tests of `src/prepare/client_set_effects/tests.rs` read what the export writes with the
//! pieces counted beside the equipment, and these tests set those fields in a prepared export and
//! run the fight:
//!
//! - Demonheart Raiment: Life Tap's mana is 20% more with the health it spends unchanged
//!   (`mana_bonus`).
//! - Manaflare Regalia: for Frostfire Bolt alone, ten points more chance to trigger Missile Barrage
//!   and Fingers of Frost (`frostfire_bolt_chance_bonus`) and ten more crit while Combustion is up
//!   (`frostfire_bolt_crit`). A bonus of 1 (or 100) makes each roll certain, so a count is exact.
//! - Raiments of Conviction: a Devouring Plague cooldown below zero is no cooldown.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const LIFE_TAP: i64 = 11689;
const DEVOURING_PLAGUE: i64 = 19280;
const COMBUSTION: i64 = 11129;
const FROSTBOLT: i64 = 25304;
const FROSTFIRE_BOLT: i64 = 1237313;
const MISSILE_BARRAGE: i64 = 44404;
const FINGERS_OF_FROST: i64 = 400669;
/// Missile Barrage, Combustion and Fingers of Frost in the client's order (Arcane's seventh
/// talent, Fire's seventeenth and Frost's sixteenth).
const MAGE_TALENTS: &str = "0000002-00000000000000001-0000000000000002";

fn fixture(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// `request` in a fixed fight of `seconds` that casts `spells` in order of priority.
fn with_rotation(mut request: Value, seconds: i64, iterations: i64, spells: &[i64]) -> Value {
    request["encounter"]["duration"] = json!(seconds);
    request["encounter"]["durationVariation"] = json!(0);
    request["simOptions"]["iterations"] = json!(iterations);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    request["raid"]["parties"][0]["players"][0]["rotation"] = json!({
        "type": "TypeAPL",
        "prepullActions": [],
        "priorityList": spells
            .iter()
            .map(|id| json!({"action": {"castSpell": {"spellId": {"spellId": id}}}}))
            .collect::<Vec<_>>(),
    });
    request
}

fn prepared(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn simulate(prepared: Value) -> Value {
    let prepared = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["raidMetrics"]["parties"][0]["players"][0].clone()
}

fn effect_mut<'a>(prepared: &'a mut Value, kind: &str) -> &'a mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

fn action(player: &Value, spell_id: i64) -> &Value {
    &player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"].as_i64() == Some(spell_id))
        .unwrap_or_else(|| panic!("spell {spell_id} was not cast"))["targets"][0]
}

fn procs(player: &Value, spell_id: i64) -> f64 {
    player["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["id"]["spellId"].as_i64() == Some(spell_id))
        .and_then(|aura| aura["procsAvg"].as_f64())
        .unwrap_or(0.0)
}

/// The mana Life Tap generated and the health the fight lost, summed over its casts.
fn life_tap_flows(player: &Value) -> (f64, f64) {
    let resources = player["resources"].as_array().unwrap();
    let gain = |matches: &dyn Fn(&Value) -> bool| {
        resources
            .iter()
            .filter(|resource| matches(resource))
            .map(|resource| resource["gain"].as_f64().unwrap())
            .sum::<f64>()
    };
    (
        gain(&|r| r["type"] == "ResourceTypeMana" && r["id"]["spellId"] == LIFE_TAP),
        -gain(&|r| {
            r["type"] == "ResourceTypeHealth" && r["id"]["otherId"] == "OtherActionDamageTaken"
        }),
    )
}

#[test]
fn life_tap_gives_twenty_percent_more_mana_for_the_same_health() {
    let request = with_rotation(
        fixture("fixtures/mage/prepared-v2/production-affliction-warlock-wrack.request.json"),
        3,
        5,
        &[LIFE_TAP],
    );
    let base = prepared(&request);
    assert!(effect_mut(&mut base.clone(), "life_tap")
        .get("mana_bonus")
        .is_none());
    let (mana, health) = life_tap_flows(&simulate(base.clone()));
    assert!(mana > 0.0);
    // Improved Life Tap's share scales both sides alike.
    assert!((mana / health - 1.0).abs() < 1e-9, "{mana} {health}");

    let mut boosted = base;
    effect_mut(&mut boosted, "life_tap")["mana_bonus"] = json!(0.2);
    let (boosted_mana, boosted_health) = life_tap_flows(&simulate(boosted));
    assert!(
        (boosted_health - health).abs() < 1e-6,
        "{boosted_health} {health}"
    );
    assert!((boosted_mana / boosted_health - 1.2).abs() < 1e-9);
    assert!((boosted_mana / mana - 1.2).abs() < 1e-9);
}

fn mage(spells: &[i64], seconds: i64, iterations: i64) -> Value {
    let mut request = with_rotation(
        fixture("fixtures/mage/prepared-v2/frost-reference.request.json"),
        seconds,
        iterations,
        spells,
    );
    request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(MAGE_TALENTS);
    prepared(&request)
}

/// With no other chance, a Frostfire Bolt rolls Missile Barrage and Fingers of Frost at the
/// set's ten points alone, and a Frostbolt at none.
#[test]
fn the_bonus_chances_are_for_frostfire_bolt_only() {
    let certain = |spells: &[i64]| {
        let mut prepared = mage(spells, 10, 10);
        let barrage = effect_mut(&mut prepared, "missile_barrage");
        barrage["arcane_blast_chance"] = json!(0.0);
        barrage["bolt_chance"] = json!(0.0);
        barrage["frostfire_bolt_chance_bonus"] = json!(1.0);
        let fingers = effect_mut(&mut prepared, "fingers_of_frost");
        fingers["proc_chance"] = json!(0.0);
        fingers["frostfire_bolt_chance_bonus"] = json!(1.0);
        let player = simulate(prepared);
        (
            procs(&player, MISSILE_BARRAGE),
            procs(&player, FINGERS_OF_FROST),
        )
    };
    let (barrage, fingers) = certain(&[FROSTFIRE_BOLT]);
    assert!(barrage > 0.0 && fingers > 0.0, "{barrage} {fingers}");
    assert_eq!(certain(&[FROSTBOLT]), (0.0, 0.0));
}

/// While Combustion is up Frostfire Bolt gains the crit: at 100 every hit under the buff crits,
/// and without it they do not all.
#[test]
fn frostfire_bolt_gains_crit_while_combustion_is_up() {
    let run = |bonus: Option<f64>| {
        let mut prepared = mage(&[COMBUSTION, FROSTFIRE_BOLT], 9, 20);
        if let Some(bonus) = bonus {
            effect_mut(&mut prepared, "combustion")["frostfire_bolt_crit"] = json!(bonus);
        }
        let player = simulate(prepared);
        let target = action(&player, FROSTFIRE_BOLT);
        let count = |range: &str| target[range]["count"].as_i64().unwrap_or(0);
        (count("critRange"), count("hitRange"))
    };
    let (crits, hits) = run(Some(100.0));
    assert!(crits > 0);
    assert_eq!(hits, 0, "every hit under Combustion crits");
    let (_, plain_hits) = run(None);
    assert!(plain_hits > 0);
}

/// A Devouring Plague cooldown below zero, as an unfloored 60 sec off a 24 sec one would give, is
/// no cooldown: the casts are those of a cooldown of none.
#[test]
fn a_negative_devouring_plague_cooldown_is_none() {
    let request = with_rotation(
        fixture("fixtures/forever-gaps/shadow-priest-chastise.request.json"),
        12,
        3,
        &[DEVOURING_PLAGUE],
    );
    let base = prepared(&request);
    let casts = |duration_ns: Option<i64>| {
        let mut prepared = base.clone();
        if let Some(duration) = duration_ns {
            for spell in prepared["player"]["spells"].as_array_mut().unwrap() {
                if spell["class_spell"] == "devouring_plague" {
                    spell["cd"]["duration_ns"] = json!(duration);
                }
            }
        }
        action(&simulate(prepared), DEVOURING_PLAGUE)["casts"]
            .as_i64()
            .unwrap()
    };
    let with_cooldown = casts(None);
    let none = casts(Some(0));
    assert!(with_cooldown < none, "{with_cooldown} {none}");
    assert_eq!(casts(Some(-36_000_000_000)), none);
}
