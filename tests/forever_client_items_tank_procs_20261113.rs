//! Forever client items and set bonuses that refused real players' Quick Sims and Top Gear runs
//! on 13 November 2026, all acting when the wearer is struck or blocks, from the client rows
//! wago.tools serves (`product=wowf`, build 1.60.1.70888) and Wowhead's Forever tooltips of that
//! day:
//!
//! - "set bonus Krol'dok Battlegear 4P is not prepared yet (item 287013)", about ten Protection
//!   Warriors of level 44 and 45 in a day wearing Helmet 275006, Pauldrons 287014, Gauntlets
//!   287012 and Legplates 287013. (4) Krol'dok Resolve (1324298, casting 1324296): "Chance on
//!   block to heal for 14 to 23." (5) Ogre Dominance (1324309, applying 1324308): "Taking damage
//!   has a high chance to increase your Stamina by 1 for 30 sec, stacking up to 20 times. (3s
//!   cooldown)"
//! - "item 274651 has an effect not prepared yet (mana when struck ...)": Zealot Plate, Proc Self
//!   Energize 88 (1294668, casting Zealotry 1294666): "When struck in combat, has a chance to
//!   energize you for 66 to 110 mana. This effect cannot occur more than once every 15 sec."
//! - Girdle of Extracted Answers (276742), Shield Spike (1308777): "Deals 20 to 30 Physical damage
//!   every time you block." SpellCooldowns 100881: 5000 ms.
//!
//! The owner's decision of 13 November 2026 reads the rows literally: a ProcChance of 100 is the
//! rate, gated by each row's own recovery or cooldown. No refused request was at hand, so the
//! tests dress fixture characters: the production Protection Warrior of
//! `fixtures/mage/prepared-v2` (talents read in the client's order) and the Protection Paladin of
//! `fixtures/forever-gaps`, who tank the target with a shield, and the DPS Warrior of
//! `fixtures/forever-gaps`, whom no target attacks.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/warrior_trees_20261104.rs"]
mod warrior_trees_20261104;

const KROLDOK_HELMET: i64 = 275006;
const KROLDOK_PAULDRONS: i64 = 287014;
const KROLDOK_CHEST: i64 = 275005;
const KROLDOK_GAUNTLETS: i64 = 287012;
const KROLDOK_LEGPLATES: i64 = 287013;
const ZEALOT_PLATE: i64 = 274651;
const GIRDLE_OF_EXTRACTED_ANSWERS: i64 = 276742;

const KROLDOK_RESOLVE_HEAL: i64 = 1324296;
const OGRE_DOMINANCE_BUFF: i64 = 1324308;
const ZEALOTRY: i64 = 1294666;
const SHIELD_SPIKE: i64 = 1308777;

const HEAD: usize = 0;
const SHOULDER: usize = 2;
const CHEST: usize = 4;
const HANDS: usize = 6;
const WAIST: usize = 7;
const LEGS: usize = 8;
const OFF_HAND: usize = 15;

/// The armor slots the tests fill, emptied first so a difference is the items' alone.
const ARMOR_SLOTS: [usize; 6] = [HEAD, SHOULDER, CHEST, HANDS, WAIST, LEGS];

fn read(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn dress(request: &mut Value, items: &[(usize, i64)]) {
    request["simOptions"]["iterations"] = json!(50);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.resize(17, json!({}));
    for slot in ARMOR_SLOTS {
        equipment[slot] = json!({});
    }
    for &(slot, id) in items {
        equipment[slot] = json!({ "id": id });
    }
}

/// The production Protection Warrior, who tanks the target with a shield (index 13 of its list,
/// which preparation places by type) and has no mana bar.
fn tank_warrior(items: &[(usize, i64)]) -> Value {
    let mut request = read(
        "fixtures/mage/prepared-v2/protection-warrior-blood-craze-ephemeral-power.request.json",
    );
    warrior_trees_20261104::forever_request(&mut request);
    dress(&mut request, items);
    request
}

/// The Protection Paladin, who tanks the target with a shield and has a mana bar.
fn tank_paladin(items: &[(usize, i64)]) -> Value {
    let mut request = read("fixtures/forever-gaps/protection-paladin-quel-serrar.request.json");
    dress(&mut request, items);
    request
}

/// The DPS Warrior of the Pummel request, whom no target attacks.
fn dps_warrior(items: &[(usize, i64)]) -> Value {
    let mut request = read("fixtures/forever-gaps/dps-warrior-pummel.request.json");
    request["raid"]["buffs"] = json!({});
    request["raid"]["parties"][0]["buffs"] = json!({});
    request["raid"]["parties"][0]["players"][0]["buffs"] = json!({});
    dress(&mut request, items);
    request
}

fn prepare(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// Prepared, through the gate and run: the prepared input and the player's metrics.
fn run(request: &Value) -> (Value, Value) {
    let value = prepare(request);
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    let player = result["raidMetrics"]["parties"][0]["players"][0].clone();
    assert!(player["dps"]["avg"].as_f64().unwrap() > 0.0);
    // The blocks the target's swings met, over every iteration.
    let blocks: f64 = result["encounterMetrics"]["targets"][0]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["blocks"].as_f64().unwrap_or(0.0))
        .sum();
    let mut player = player;
    player["blocksTaken"] = json!(blocks);
    (value, player)
}

fn refusal(request: &Value, rules: Rules) -> String {
    match prepare_json_with(request.to_string().as_bytes(), "rust", rules) {
        Err(PrepareError::Refused(refusal)) => refusal.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn effects<'a>(prepared: &'a Value, kind: &str) -> Vec<&'a Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == kind)
        .collect()
}

/// The effect of a kind a trigger aura owns.
fn effect_of<'a>(prepared: &'a Value, kind: &str, aura: &str) -> &'a Value {
    effects(prepared, kind)
        .into_iter()
        .find(|effect| effect["trigger_aura"] == aura)
        .unwrap_or_else(|| panic!("no {kind} of {aura}"))
}

fn inert(prepared: &Value, aura: &str) -> bool {
    effects(prepared, "inert_listener")
        .iter()
        .any(|effect| effect["aura"] == aura)
}

/// The metrics of the player's action with this spell id, summed over every tag and target.
fn total(player: &Value, id: i64, field: &str) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"].as_i64() == Some(id))
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target[field].as_f64().unwrap_or(0.0))
        .sum()
}

fn aura_metrics(player: &Value, id: i64) -> Option<&Value> {
    player["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["id"]["spellId"].as_i64() == Some(id))
}

fn has_aura(prepared: &Value, label: &str) -> bool {
    prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .any(|aura| aura["label"] == label)
}

fn four_kroldok() -> Vec<(usize, i64)> {
    vec![
        (HEAD, KROLDOK_HELMET),
        (SHOULDER, KROLDOK_PAULDRONS),
        (CHEST, KROLDOK_CHEST),
        (HANDS, KROLDOK_GAUNTLETS),
    ]
}

fn five_kroldok() -> Vec<(usize, i64)> {
    let mut items = four_kroldok();
    items.push((LEGS, KROLDOK_LEGPLATES));
    items
}

// ---------------------------------------------------------------------------------------------
// Krol'dok Battlegear.
// ---------------------------------------------------------------------------------------------

/// (4) Every block heals the wearer once: a heal proc that hears the blocked swings of the target,
/// at a chance of 1, for a roll of 18 with a variance of 0.5.
#[test]
fn four_kroldok_pieces_heal_on_every_block() {
    let request = tank_warrior(&four_kroldok());
    let (prepared, player) = run(&request);
    assert!(has_aura(&prepared, "Krol'dok Battlegear 4P"));
    assert!(!has_aura(&prepared, "Krol'dok Battlegear 5P"));
    let heal = effect_of(&prepared, "spell_data_heal_proc", "Krol'dok Battlegear 4P");
    assert_eq!(heal["struck"], true);
    assert_eq!(heal["outcome"], json!(["Block"]));
    assert_eq!(heal["proc_chance"], 1.0);
    assert_eq!(heal["average"], 18.0);
    assert_eq!(heal["variance"], 0.5);
    // No proc recovery: the trigger row states none.
    let trigger = prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == "Krol'dok Battlegear 4P")
        .unwrap();
    assert!(trigger["icd"].is_null(), "{trigger}");

    let blocks = player["blocksTaken"].as_f64().unwrap();
    assert!(blocks > 0.0);
    let heals = total(&player, KROLDOK_RESOLVE_HEAL, "hits")
        + total(&player, KROLDOK_RESOLVE_HEAL, "crits");
    assert_eq!(heals, blocks, "one heal for every block");
    // A roll of 13.5 to 22.5 through the spell's multipliers, crits included.
    let average = total(&player, KROLDOK_RESOLVE_HEAL, "healing") / heals;
    assert!((13.0..=32.0).contains(&average), "{average}");

    // Without the set the warrior heals nothing.
    let (_, plain) = run(&tank_warrior(&[]));
    assert_eq!(total(&plain, KROLDOK_RESOLVE_HEAL, "healing"), 0.0);
}

/// (5) Each landed hit taken that deals damage, at most once in 3 sec, adds a stack of +1 Stamina
/// for 30 sec, up to 20.
#[test]
fn five_kroldok_pieces_stack_stamina_when_struck() {
    let (prepared, player) = run(&tank_warrior(&five_kroldok()));
    assert!(has_aura(&prepared, "Krol'dok Battlegear 5P"));
    // The four piece bonus is worn too.
    effect_of(&prepared, "spell_data_heal_proc", "Krol'dok Battlegear 4P");

    let proc = effect_of(&prepared, "spell_data_stat_proc", "Krol'dok Battlegear 5P");
    assert_eq!(proc["aura"], "Ogre Dominance");
    assert_eq!(proc["struck"], true);
    assert_eq!(proc["add_stack"], true);
    assert_eq!(proc["proc_chance"], 1.0);
    assert_eq!(proc["landed_only"], true);
    assert_eq!(proc["require_damage"], true);

    let auras = prepared["player"]["auras"].as_array().unwrap();
    let trigger = auras
        .iter()
        .find(|aura| aura["label"] == "Krol'dok Battlegear 5P")
        .unwrap();
    assert_eq!(
        trigger["icd"]["duration_ns"], 3_000_000_000_i64,
        "{trigger}"
    );
    let buff = auras
        .iter()
        .find(|aura| aura["label"] == "Ogre Dominance")
        .expect("the stacking buff");
    assert_eq!(buff["max_stacks"], 20);
    assert_eq!(buff["duration_ns"], 30_000_000_000_i64);

    // The stacks are a stat aura of five bits whose combinations carry the Stamina and the Health
    // it moves.
    let stat_auras = effects(&prepared, "stat_auras")[0];
    let position = stat_auras["auras"]
        .as_array()
        .unwrap()
        .iter()
        .position(|label| label == "Ogre Dominance")
        .expect("a stat aura");
    assert_eq!(stat_auras["stacks"][position], 20);
    assert!(stat_auras["changed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|stat| stat == "Stamina"));
    let health: Vec<f64> = stat_auras["combos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|combo| combo["Health"].as_f64().unwrap())
        .collect();
    let (low, high) = health.iter().fold((f64::MAX, f64::MIN), |(low, high), &h| {
        (low.min(h), high.max(h))
    });
    assert!(high - low >= 20.0, "{low} to {high}");

    // Over 120 seconds of swings the buff goes up and refreshes, at most once in 3 sec.
    let metrics = aura_metrics(&player, OGRE_DOMINANCE_BUFF).expect("the buff's metrics");
    let procs = metrics["procsAvg"].as_f64().unwrap();
    assert!(procs > 5.0 && procs <= 41.0, "{procs}");
    assert!(metrics["uptimeSecondsAvg"].as_f64().unwrap() > 60.0);

    // Four pieces stack nothing.
    let (four, _) = run(&tank_warrior(&four_kroldok()));
    assert!(!has_aura(&four, "Ogre Dominance"));
}

/// Worn by a character no target attacks, both listeners are inert and nothing stacks.
#[test]
fn ogre_dominance_is_no_stat_aura_off_the_tank() {
    let (prepared, _) = run(&dps_warrior(&five_kroldok()));
    assert!(inert(&prepared, "Krol'dok Battlegear 4P"));
    assert!(inert(&prepared, "Krol'dok Battlegear 5P"));
    assert!(effects(&prepared, "spell_data_heal_proc").is_empty());
    assert!(effects(&prepared, "spell_data_stat_proc").is_empty());
    let in_stat_auras = effects(&prepared, "stat_auras")
        .iter()
        .any(|effect| effect["auras"].to_string().contains("Ogre Dominance"));
    assert!(!in_stat_auras);
}

// ---------------------------------------------------------------------------------------------
// Zealot Plate.
// ---------------------------------------------------------------------------------------------

/// The gains of mana a metrics list holds for an action with this spell id: (count, total).
fn mana_gains(player: &Value, id: i64) -> (f64, f64) {
    let mut count = 0.0;
    let mut total = 0.0;
    for resource in player["resources"].as_array().unwrap() {
        if resource["id"]["spellId"].as_i64() == Some(id) {
            count += resource["events"].as_f64().unwrap_or(0.0);
            total += resource["gain"].as_f64().unwrap_or(0.0);
        }
    }
    (count, total)
}

/// A tank with a mana bar who wears Zealot Plate gains 66 to 110 mana from a landed hit taken, at
/// most once in 15 sec.
#[test]
fn zealot_plate_restores_mana_to_a_struck_tank_with_mana() {
    let request = tank_paladin(&[(CHEST, ZEALOT_PLATE)]);
    let (prepared, player) = run(&request);
    let proc = effect_of(&prepared, "mana_proc", "Zealot Plate");
    assert_eq!(proc["struck"], true);
    assert_eq!(proc["mana"], 88.0);
    assert_eq!(proc["variance"], 0.5);
    assert_eq!(proc["metrics_action_id"]["spell_id"], ZEALOTRY);
    let trigger = prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == "Zealot Plate")
        .unwrap();
    assert_eq!(
        trigger["icd"]["duration_ns"], 15_000_000_000_i64,
        "{trigger}"
    );

    let iterations = request["simOptions"]["iterations"].as_f64().unwrap();
    let (count, mana) = mana_gains(&player, ZEALOTRY);
    // The fight lasts 120 sec: at most nine gains an iteration behind the 15 sec recovery, and
    // the target swings far more often than that, so nearly every window is used.
    assert!(
        count > iterations * 6.0 && count <= iterations * 9.0,
        "{count}"
    );
    let average = mana / count;
    assert!((66.0..=110.0).contains(&average), "{average}");
    assert!((80.0..=96.0).contains(&average), "{average}");

    // Without the plate the paladin gains none of it.
    let (_, plain) = run(&tank_paladin(&[]));
    assert_eq!(mana_gains(&plain, ZEALOTRY), (0.0, 0.0));
}

/// A tank with no mana bar gains nothing, and a character no target attacks is never struck: the
/// plate is a plain item on either.
#[test]
fn zealot_plate_is_plain_where_nothing_is_gained() {
    let (prepared, player) = run(&tank_warrior(&[(CHEST, ZEALOT_PLATE)]));
    assert!(!has_aura(&prepared, "Zealot Plate"));
    assert!(effects(&prepared, "mana_proc").is_empty());
    assert_eq!(mana_gains(&player, ZEALOTRY), (0.0, 0.0));

    let (prepared, _) = run(&dps_warrior(&[(CHEST, ZEALOT_PLATE)]));
    assert!(!has_aura(&prepared, "Zealot Plate"));

    let mut untanked = tank_paladin(&[(CHEST, ZEALOT_PLATE)]);
    untanked["raid"].as_object_mut().unwrap().remove("tanks");
    let (prepared, player) = run(&untanked);
    assert!(inert(&prepared, "Zealot Plate"));
    assert_eq!(mana_gains(&player, ZEALOTRY), (0.0, 0.0));
}

// ---------------------------------------------------------------------------------------------
// Girdle of Extracted Answers.
// ---------------------------------------------------------------------------------------------

/// A tank with a shield deals 20 to 30 Physical damage to the attacker on a block, at most once
/// in 5 sec.
#[test]
fn the_girdle_deals_its_damage_on_a_block() {
    let request = tank_warrior(&[(WAIST, GIRDLE_OF_EXTRACTED_ANSWERS)]);
    let (prepared, player) = run(&request);
    let proc = effect_of(
        &prepared,
        "spell_data_damage_proc",
        "Girdle of Extracted Answers",
    );
    assert_eq!(proc["struck"], true);
    assert_eq!(proc["taken_outcome"], json!(["Block"]));
    assert_eq!(proc["outcome"], "always_hit");
    assert_eq!(proc["can_crit"], false);
    assert_eq!(proc["average"], 25.0);
    assert!((proc["variance"].as_f64().unwrap() - 0.4).abs() < 1e-6);
    let trigger = prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == "Girdle of Extracted Answers")
        .unwrap();
    assert_eq!(
        trigger["icd"]["duration_ns"], 5_000_000_000_i64,
        "{trigger}"
    );

    let iterations = request["simOptions"]["iterations"].as_f64().unwrap();
    let hits = total(&player, SHIELD_SPIKE, "hits");
    assert_eq!(
        total(&player, SHIELD_SPIKE, "misses"),
        0.0,
        "it never misses"
    );
    assert_eq!(total(&player, SHIELD_SPIKE, "crits"), 0.0, "it cannot crit");
    // A block comes every few swings, a few times a fight, and the 5 sec cooldown swallows those
    // that follow another within it: at most one for every block, and never more than 24 a fight.
    let blocks = player["blocksTaken"].as_f64().unwrap();
    assert!(
        hits > blocks * 0.5 && hits < blocks,
        "{hits} of {blocks} blocks"
    );
    assert!(hits <= iterations * 24.0);
    let average = total(&player, SHIELD_SPIKE, "damage") / hits;
    assert!((14.0..=34.0).contains(&average), "{average}");

    // Without the girdle the warrior deals none of it.
    let (_, plain) = run(&tank_warrior(&[]));
    assert_eq!(total(&plain, SHIELD_SPIKE, "damage"), 0.0);
}

/// A tank with no shield never blocks, and a character no target attacks is never struck.
#[test]
fn the_girdle_is_plain_where_nothing_blocks() {
    let mut no_shield = tank_paladin(&[(WAIST, GIRDLE_OF_EXTRACTED_ANSWERS)]);
    no_shield["raid"]["parties"][0]["players"][0]["equipment"]["items"][OFF_HAND] = json!({});
    let (_, player) = run(&no_shield);
    assert_eq!(total(&player, SHIELD_SPIKE, "damage"), 0.0);

    let (prepared, _) = run(&dps_warrior(&[(WAIST, GIRDLE_OF_EXTRACTED_ANSWERS)]));
    assert!(inert(&prepared, "Girdle of Extracted Answers"));
    assert!(effects(&prepared, "spell_data_damage_proc").is_empty());
}

// ---------------------------------------------------------------------------------------------
// The reference's rules.
// ---------------------------------------------------------------------------------------------

/// The reference's rules still refuse every one of these items as unknown.
#[test]
fn the_reference_rules_still_refuse_the_items() {
    for (slot, id) in [
        (CHEST, ZEALOT_PLATE),
        (WAIST, GIRDLE_OF_EXTRACTED_ANSWERS),
        (CHEST, KROLDOK_CHEST),
    ] {
        let reason = refusal(&tank_warrior(&[(slot, id)]), Rules::GoReference);
        assert!(
            reason.contains(&format!("no item with id {id}")),
            "{reason}"
        );
    }
}
