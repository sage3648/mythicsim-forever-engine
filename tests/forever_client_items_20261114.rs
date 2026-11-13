//! Sul'thraze the Lasher (9372), a reference two-hand sword whose chance on hit the reference's
//! list leaves out ("deals damage and debuffs the target"), so a sim wearing it had no effect.
//! Wowhead's Forever tooltip: "Chance on hit: Lash the enemy with the rage of Sul'thraze, reducing
//! their attack power by 180 for 15 sec and dealing 95 to 223 Shadow damage. After 5 sec, the
//! target suffers an additional 159 Shadow damage. This additional Shadow damage deals 2 times as
//! much damage to Trolls."
//!
//! The client rows wago.tools serves (`product=wowf`, read on 13 November 2026) are the store's:
//! ItemEffect 98585 is a chance on hit (trigger type 2) of Rage of Sul'thraze (1294430) with no
//! rate and no cooldown. Its SpellEffect 1315781 is a School Damage of 159 with Variance 0.8,
//! Shadow, magic defense; 1315782 and 1315783 lower the target's melee and ranged attack power by
//! 180 for 15 sec; 1315784 triggers Shadow Lash (1294431) on the target; 1315810 is a Dummy of 2,
//! the doubling against Trolls. Shadow Lash's SpellEffect 1315785 is a periodic damage aura of 159
//! every 5000 ms for 5 sec; the row always hits (Attributes_3 0x40000) and its tick can crit
//! (Attributes_8 0x200).
//!
//! The tests dress fixture warriors: the Fury Warrior of `fixtures/forever-gaps/dps-warrior-pummel`
//! and the Protection Warrior of `fixtures/forever-gaps/protection-warrior-thunder-clap`, whom the
//! target attacks, each with the sword in place of Ironfoe and no off hand.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/warrior_trees_20261104.rs"]
mod warrior_trees_20261104;

const SULTHRAZE: i64 = 9372;
const RAGE_OF_SULTHRAZE: i64 = 1294430;
const NAME: &str = "Sul'thraze the Lasher";

/// The fixtures' main hand.
const IRONFOE: i64 = 11684;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// A fixture's request (a path under `fixtures/`) with the sword in place of Ironfoe, the off hand
/// taken out and no consumables.
fn wielding(fixture: &str, off_hand: i64, iterations: u32) -> Value {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/{fixture}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(iterations);
    let player = player(&mut request);
    player["consumables"] = json!({});
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.retain(|item| item["id"].as_i64() != Some(off_hand));
    let main_hand = equipment
        .iter_mut()
        .find(|item| item["id"].as_i64() == Some(IRONFOE))
        .expect("the fixture wields Ironfoe");
    *main_hand = json!({ "id": SULTHRAZE });
    request
}

/// The Fury Warrior, in Forever's talent trees.
fn warrior(iterations: u32) -> Value {
    wielding("forever-gaps/dps-warrior-pummel", 13368, iterations)
}

/// The Protection Warrior, whom the target attacks, with its talents read in Forever's trees.
fn tank() -> Value {
    let mut request = wielding("forever-gaps/protection-warrior-thunder-clap", 18756, 20);
    warrior_trees_20261104::forever_request(&mut request);
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// A prepared input through the gate and run: the player's metrics.
fn simulate(value: &Value) -> Value {
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    let player = result["raidMetrics"]["parties"][0]["players"][0].clone();
    assert!(player["dps"]["avg"].as_f64().unwrap() > 0.0);
    player
}

/// The player's actions with this spell id and tag.
fn targets(player: &Value, id: i64, tag: i64) -> Vec<Value> {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| {
            action["id"]["spellId"].as_i64() == Some(id)
                && action["id"]["tag"].as_i64().unwrap_or(0) == tag
        })
        .flat_map(|action| action["targets"].as_array().unwrap().clone())
        .collect()
}

fn sum(targets: &[Value], field: &str) -> f64 {
    targets
        .iter()
        .map(|target| target[field].as_f64().unwrap_or(0.0))
        .sum()
}

fn proc(prepared: &Value) -> Option<&Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "spell_data_damage_proc" && effect["trigger_aura"] == NAME)
}

/// The sword prepares, passes the gate and runs in the warrior's hands, and its lash deals damage.
#[test]
fn the_sword_prepares_and_runs() {
    let player = simulate(&prepared(&warrior(20), Rules::Forever));
    let lash = targets(&player, RAGE_OF_SULTHRAZE, 0);
    assert!(sum(&lash, "damage") > 0.0);
}

/// The lash is the client row's: a Shadow hit of 159 with the row's variance on the magic table,
/// with no coefficient, then Shadow Lash's 159 a tick on the dot it carries, which rolls the magic
/// crit alone, landing with the hit. No rate is stated: 4.68% a swing at 2.60.
#[test]
fn the_lash_is_the_rows_hit_and_shadow_lash() {
    let prepared = prepared(&warrior(20), Rules::Forever);
    let proc = proc(&prepared).expect("Sul'thraze's chance on hit");
    assert_eq!(proc["landed_only"], true);
    assert_eq!(proc["average"], 159.0);
    assert!((proc["variance"].as_f64().unwrap() - 0.8).abs() < 1e-6);
    assert_eq!(proc["can_crit"], true);
    assert!(proc.get("outcome").is_none());
    assert_eq!(
        proc["periodic"],
        json!({ "tick_base": 159.0, "tick_outcome": "tick_magic_crit", "with_direct": true })
    );
    let chances = proc["chances"].as_array().unwrap();
    assert!(!chances.is_empty());
    for chance in chances {
        let chance = chance["chance"].as_f64().unwrap();
        assert!((chance - 0.018 * 2.6).abs() < 1e-9, "{chance}");
    }
    let position = proc["spell"].as_u64().unwrap() as usize;
    let spell = &prepared["player"]["spells"][position];
    assert_eq!(spell["action_id"]["spell_id"], RAGE_OF_SULTHRAZE);
    assert_eq!(spell["school"], 32);
    assert_eq!(spell["defense_type"], "DefenseTypeMagic");
    assert_eq!(spell["bonus_coefficient"], 0.0);
    assert_eq!(spell["damage_multiplier"], 1.0);
    assert!(!spell["dot"].is_null(), "the spell carries Shadow Lash");
}

/// In the fight the hit that does not crit rolls 95.4 to 222.6, and Shadow Lash ticks once a
/// landed hit for the row's 159 when it does not crit, 5 sec later. Against no resistance and no
/// debuffs.
#[test]
fn the_lash_hits_and_shadow_lash_ticks() {
    let mut request = warrior(200);
    request["raid"]["debuffs"] = json!({});
    let player = simulate(&prepared(&request, Rules::Forever));
    let hit = targets(&player, RAGE_OF_SULTHRAZE, 0);
    let landed = sum(&hit, "hits") + sum(&hit, "crits");
    assert!(landed > 0.0);
    for target in &hit {
        if target["hits"].as_f64().unwrap_or(0.0) > 0.0 {
            assert!(target["hitRange"]["min"].as_f64().unwrap() >= 95.0);
            assert!(target["hitRange"]["max"].as_f64().unwrap() <= 223.0);
        }
    }
    let ticks: Vec<Value> = player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"].as_i64() == Some(RAGE_OF_SULTHRAZE))
        .flat_map(|action| action["targets"].as_array().unwrap().clone())
        .filter(|target| target["ticks"].as_f64().unwrap_or(0.0) > 0.0)
        .collect();
    for target in &ticks {
        assert_eq!(target["tickRange"]["min"], 159.0);
        assert_eq!(target["tickRange"]["max"], 159.0);
    }
    let ticked = sum(&ticks, "ticks") + sum(&ticks, "critTicks");
    assert!(ticked > 0.0, "Shadow Lash ticks");
    // One tick for each landed hit, but for a lash that refreshes Shadow Lash within 5 sec or
    // lands within 5 sec of the fight's end.
    assert!(ticked <= landed);
    assert!(
        ticked > 0.5 * landed,
        "{ticked} ticks of {landed} landed hits"
    );
}

/// The Protection Warrior, whom the target attacks, wears it too: it prepares and runs, with the
/// target's swings at their own attack power (the reduction is not modelled).
#[test]
fn a_tank_wears_the_sword() {
    let prepared = prepared(&tank(), Rules::Forever);
    assert!(proc(&prepared).is_some());
    let player = simulate(&prepared);
    assert!(sum(&targets(&player, RAGE_OF_SULTHRAZE, 0), "damage") > 0.0);
}

/// Under the reference's rules the sword is still worn with no effect: it prepares with no lash
/// in the spellbook.
#[test]
fn the_reference_rules_wear_the_sword_without_its_lash() {
    let prepared = prepared(&warrior(20), Rules::GoReference);
    assert!(proc(&prepared).is_none());
    assert!(prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .all(|spell| spell["action_id"]["spell_id"] != RAGE_OF_SULTHRAZE));
}
