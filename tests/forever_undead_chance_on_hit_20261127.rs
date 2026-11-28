//! Eater of the Dead (10805) and X'caliboar (10758): reference weapons whose Forever chance on
//! hit deals 3 times its damage to Undead.
//!
//! Wowhead's Forever tooltips (27 November 2026): Eater of the Dead, "Chance on hit: Steals 21
//! life from the target enemy. Deals 3 times as much damage to Undead."; X'caliboar, "Chance on
//! hit: Unleash a mighty swing, dealing 33 to 51 Holy damage. Deals 3 times as much damage to
//! Undead and Swine." The client rows wago.tools serves (`product=wowf`, build 1.60.1.71139)
//! carry the 3 as each spell's second effect: Corpse Eater (1299869) SpellEffect 1324280 and Holy
//! Cleaver (1293787) SpellEffect 1314769, a dummy with base points 3, as Lightcrest Hammer's
//! Smite carries its 2 (`forever_client_items_20261113b`). The client has no Swine creature type.
//!
//! The tests dress the Fury Warrior of `fixtures/forever-gaps/dps-warrior-pummel` with the weapon
//! in place of Ironfoe and no off hand.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const EATER_OF_THE_DEAD: i64 = 10805;
const CORPSE_EATER: i64 = 1299869;
const XCALIBOAR: i64 = 10758;
const HOLY_CLEAVER: i64 = 1293787;

const IRONFOE: i64 = 11684;
const OFF_HAND: i64 = 13368;

/// The Fury Warrior with this weapon in place of Ironfoe, the off hand taken out, no consumables,
/// against targets of this creature type.
fn wielding(weapon: i64, mob_type: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/forever-gaps/dps-warrior-pummel.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(200);
    for target in request["encounter"]["targets"].as_array_mut().unwrap() {
        target["mobType"] = json!(mob_type);
    }
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["consumables"] = json!({});
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.retain(|item| item["id"].as_i64() != Some(OFF_HAND));
    let main_hand = equipment
        .iter_mut()
        .find(|item| item["id"].as_i64() == Some(IRONFOE))
        .expect("the fixture wields Ironfoe");
    *main_hand = json!({ "id": weapon });
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// The prepared spell with this id.
fn spell(prepared: &Value, id: i64) -> Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["spell_id"].as_i64() == Some(id))
        .unwrap_or_else(|| panic!("spell {id} is prepared"))
        .clone()
}

/// The damage the player dealt with this spell in a run through the gate.
fn damage(value: &Value, id: i64) -> f64 {
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"].as_i64() == Some(id))
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["damage"].as_f64().unwrap_or(0.0))
        .sum()
}

/// Against Undead each chance on hit carries a damage multiplier of 3, against a Humanoid 1, and
/// nothing else about the spell differs.
#[test]
fn the_chance_on_hit_is_tripled_against_undead() {
    for (weapon, id) in [(EATER_OF_THE_DEAD, CORPSE_EATER), (XCALIBOAR, HOLY_CLEAVER)] {
        let undead = prepared(&wielding(weapon, "MobTypeUndead"), Rules::Forever);
        let humanoid = prepared(&wielding(weapon, "MobTypeHumanoid"), Rules::Forever);
        assert_eq!(spell(&undead, id)["damage_multiplier"], 3.0, "{weapon}");
        assert_eq!(spell(&humanoid, id)["damage_multiplier"], 1.0, "{weapon}");
        let mut same = spell(&undead, id);
        same["damage_multiplier"] = json!(1.0);
        assert_eq!(same, spell(&humanoid, id), "{weapon}");
    }
}

/// In the fight the chance on hit lands, and deals more to an Undead target than to a Humanoid.
#[test]
fn the_chance_on_hit_deals_more_to_undead() {
    for (weapon, id) in [(EATER_OF_THE_DEAD, CORPSE_EATER), (XCALIBOAR, HOLY_CLEAVER)] {
        let undead = damage(
            &prepared(&wielding(weapon, "MobTypeUndead"), Rules::Forever),
            id,
        );
        let humanoid = damage(
            &prepared(&wielding(weapon, "MobTypeHumanoid"), Rules::Forever),
            id,
        );
        assert!(humanoid > 0.0, "{weapon}");
        assert!(
            undead > 2.0 * humanoid,
            "{weapon}: {undead} against {humanoid}"
        );
    }
}

/// The reference's rules read no creature type: where they prepare the chance on hit at all, its
/// multiplier stays 1 against Undead.
#[test]
fn the_reference_rules_do_not_triple_it() {
    for (weapon, id) in [(EATER_OF_THE_DEAD, CORPSE_EATER), (XCALIBOAR, HOLY_CLEAVER)] {
        let undead = prepared(&wielding(weapon, "MobTypeUndead"), Rules::GoReference);
        for spell in undead["player"]["spells"].as_array().unwrap() {
            if spell["action_id"]["spell_id"].as_i64() == Some(id) {
                assert_eq!(spell["damage_multiplier"], 1.0, "{weapon}");
            }
        }
    }
}
