//! Rules of 14 November 2026, under Forever's rules only (the Go reference keeps its own). Each
//! is from the live client rows wago.tools serves (`product=wowf`), a Wowhead Forever tooltip or
//! counts on combat logs; docs/mechanics.md has the evidence of each.
//!
//! - Enraged Idol (272428) adds 1 Rage to each of Enrage's ten ticks.
//! - Idol of Brutality (23198) cuts Maul and Swipe, not Primal Bite.
//! - Oilskin Leggings (9414) raise the Feral Combat skill by one.
//! - Redoubt rolls on every melee attack taken, an avoided one included.
//! - Two-Handed Weapon Specialization's Holy damage leaves Hammer of Wrath and Exorcism alone.
//! - Ripsaw, Bigger Rock on Bigger Stick and Sword of Dismemberment roll their measured rates.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const ENRAGED_IDOL: i64 = 272428;
const IDOL_OF_BRUTALITY: i64 = 23198;
const OILSKIN_LEGGINGS: i64 = 9414;
const RIPSAW: i64 = 9478;
const BONESLICER: i64 = 273819;
const BIGGER_ROCK_ON_BIGGER_STICK: i64 = 279480;
const SWORD_OF_DISMEMBERMENT: i64 = 287005;

const REDOUBT: i64 = 20128;
const JUDGEMENT: i64 = 20271;
const HAMMER_OF_WRATH: i64 = 24239;
const EXORCISM: i64 = 10314;

/// The live references' talents on the launch tree.
const FERAL_BEAR: &str = "-5503003212310212551-055";
const PROTECTION_PALADIN: &str = "05005-05302102213201531-5023";
const RETRIBUTION_PALADIN: &str = "050050002-503-05233301001301351";
const ARMS_WARRIOR: &str = "30303213132215201-55050000011-2";

/// The bear's legs and idol, the paladin's and the warrior's main hand in their requests.
const BEAR_LEGS: usize = 8;
const BEAR_IDOL: usize = 13;
const WARRIOR_MAIN_HAND: usize = 12;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// A production fixture's request with the talents it was captured with, which the reference's
/// trees read.
fn reference_request(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{name}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(200);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    request
}

/// The same request with talents of the launch tree, which Forever's rules read.
fn request(name: &str, talents: &str) -> Value {
    let mut request = reference_request(name);
    player(&mut request)["talentsString"] = json!(talents);
    request
}

/// A request with this item in this place of its equipment.
fn wearing(name: &str, talents: &str, slot: usize, item: i64) -> Value {
    let mut request = request(name, talents);
    player(&mut request)["equipment"]["items"][slot] = json!({ "id": item });
    request
}

fn bear(slot: usize, item: i64) -> Value {
    wearing("production-feral-bear-druid", FERAL_BEAR, slot, item)
}

/// The bear with the fixture's own talents, for the reference's rules.
fn reference_bear(slot: usize, item: i64) -> Value {
    let mut request = reference_request("production-feral-bear-druid");
    player(&mut request)["equipment"]["items"][slot] = json!({ "id": item });
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

/// A prepared input through the gate and run: the result.
fn simulate(value: &Value) -> Value {
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    simulate_prepared(&prepared).unwrap().result
}

fn dps(result: &Value) -> f64 {
    result["raidMetrics"]["dps"]["avg"].as_f64().unwrap()
}

fn effect<'a>(prepared: &'a Value, kind: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

fn effect_mut<'a>(prepared: &'a mut Value, kind: &str) -> &'a mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

/// The player's spell with this id and no tag.
fn spell(prepared: &Value, spell_id: i64) -> &Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| {
            let id = &spell["action_id"];
            id["spell_id"] == spell_id && id["tag"].as_i64().unwrap_or(0) == 0
        })
        .unwrap_or_else(|| panic!("no spell {spell_id}"))
}

/// The cost of the bear's spell of this class name, after its flat modifiers.
fn base_cost(prepared: &Value, class_spell: &str) -> i64 {
    let spell = prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["class_spell"] == class_spell && spell["cost"].is_object())
        .unwrap_or_else(|| panic!("no {class_spell}"));
    spell["cost"]["base_cost"].as_i64().unwrap() + spell["cost"]["flat_modifier"].as_i64().unwrap()
}

fn first_fight_log(prepared: &Value, seed: u64) -> String {
    let mut value = prepared.clone();
    value["sim"]["iterations"] = json!(1);
    value["sim"]["debug_first_iteration"] = json!(true);
    value["sim"]["seed"] = json!(seed);
    let prepared = serde_json::from_value(value).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

// ---------------------------------------------------------------------------------------------
// Enraged Idol, Idol of Brutality, Oilskin Leggings.
// ---------------------------------------------------------------------------------------------

/// Row 1291060 adds 10 tenths of Rage to Enrage's periodic effect: 3 Rage a tick for ten ticks
/// with the idol, 2 without it and under the reference's rules. The instant Rage stays.
#[test]
fn the_enraged_idol_adds_a_rage_to_each_enrage_tick() {
    let with = prepared(&bear(BEAR_IDOL, ENRAGED_IDOL), Rules::Forever);
    let without = prepared(&bear(BEAR_IDOL, IDOL_OF_BRUTALITY), Rules::Forever);
    let reference = prepared(&reference_bear(BEAR_IDOL, ENRAGED_IDOL), Rules::GoReference);

    assert_eq!(effect(&with, "enrage")["rage_per_tick"], 3.0);
    assert_eq!(effect(&with, "enrage")["ticks"], 10);
    assert_eq!(effect(&without, "enrage")["rage_per_tick"], 2.0);
    assert_eq!(effect(&reference, "enrage")["rage_per_tick"], 2.0);
    assert_eq!(
        effect(&with, "enrage")["instant_rage"],
        effect(&without, "enrage")["instant_rage"]
    );

    // The extra Rage is spent: the bear with the idol does more than the one with no idol.
    let mut bare = bear(BEAR_IDOL, ENRAGED_IDOL);
    player(&mut bare)["equipment"]["items"][BEAR_IDOL] = json!({});
    let bare = prepared(&bare, Rules::Forever);
    assert_eq!(effect(&bare, "enrage")["rage_per_tick"], 2.0);
    assert!(dps(&simulate(&with)) > dps(&simulate(&bare)));
}

/// Row 28855's class mask is Maul's and Swipe's, so Primal Bite keeps its 20 Rage under
/// Forever's rules; the reference cuts all three.
#[test]
fn the_idol_of_brutality_cuts_maul_and_swipe_alone() {
    let mut bare = bear(BEAR_IDOL, IDOL_OF_BRUTALITY);
    player(&mut bare)["equipment"]["items"][BEAR_IDOL] = json!({});
    let bare = prepared(&bare, Rules::Forever);
    let forever = prepared(&bear(BEAR_IDOL, IDOL_OF_BRUTALITY), Rules::Forever);
    let reference = prepared(
        &reference_bear(BEAR_IDOL, IDOL_OF_BRUTALITY),
        Rules::GoReference,
    );
    let mut reference_bare = reference_bear(BEAR_IDOL, IDOL_OF_BRUTALITY);
    player(&mut reference_bare)["equipment"]["items"][BEAR_IDOL] = json!({});
    let reference_bare = prepared(&reference_bare, Rules::GoReference);

    for cut in ["maul", "swipe"] {
        assert_eq!(base_cost(&forever, cut), base_cost(&bare, cut) - 2, "{cut}");
        assert_eq!(
            base_cost(&reference, cut),
            base_cost(&reference_bare, cut) - 2,
            "{cut}"
        );
    }
    assert_eq!(
        base_cost(&forever, "primal_bite"),
        base_cost(&bare, "primal_bite")
    );
    assert_eq!(
        base_cost(&reference, "primal_bite"),
        base_cost(&reference_bare, "primal_bite") - 2
    );
}

/// The leggings carry Spiderchitin Wrap's equip spell (1320726): one point of the skill the
/// bear's attacks take.
#[test]
fn oilskin_leggings_raise_a_druids_feral_combat_skill() {
    let skill = |prepared: &Value| {
        prepared["melee"]["main_hand_skill"]["skill"]
            .as_f64()
            .unwrap_or(0.0)
    };
    let with = prepared(&bear(BEAR_LEGS, OILSKIN_LEGGINGS), Rules::Forever);
    let mut without = bear(BEAR_LEGS, OILSKIN_LEGGINGS);
    player(&mut without)["equipment"]["items"][BEAR_LEGS] = json!({});
    let without = prepared(&without, Rules::Forever);

    assert_eq!(skill(&without), 0.0);
    assert_eq!(skill(&with), 1.0);
    simulate(&with);
}

// ---------------------------------------------------------------------------------------------
// Redoubt.
// ---------------------------------------------------------------------------------------------

/// The swings of the target that Redoubt follows in a fight's log: (avoided, damaging).
fn redoubt_after_swings(logs: &str) -> (u32, u32) {
    let gained = format!("Aura gained: {{SpellID: {REDOUBT}}}");
    let lines: Vec<&str> = logs.lines().collect();
    let (mut avoided, mut damaging) = (0, 0);
    for (index, line) in lines.iter().enumerate() {
        if !line.contains(&gained) {
            continue;
        }
        let time = &line[..line.find(']').unwrap() + 1];
        // The swing that rolled it is the target's nearest result line before it at the same
        // moment.
        let swing_start = format!("{time} [Target ");
        let Some(swing) = lines[..index]
            .iter()
            .rev()
            .take_while(|earlier| earlier.starts_with(time))
            .find(|earlier| {
                earlier.starts_with(&swing_start) && earlier.contains("(SpellSchool: ")
            })
        else {
            continue;
        };
        if swing.contains(" Miss ") || swing.contains(" Dodge ") || swing.contains(" Parry ") {
            avoided += 1;
        } else {
            damaging += 1;
        }
    }
    (avoided, damaging)
}

/// Under Forever's rules the effect says every attack rolls, and Redoubt then follows swings the
/// paladin dodged, parried or was missed by; without the flag only a swing that landed and dealt
/// damage raises it.
#[test]
fn redoubt_rolls_on_every_melee_attack_taken() {
    let request = request("production-protection-paladin", PROTECTION_PALADIN);
    let forever = prepared(&request, Rules::Forever);
    let reference = prepared(
        &reference_request("production-protection-paladin"),
        Rules::GoReference,
    );
    assert_eq!(effect(&forever, "redoubt")["any_attack"], true);
    assert!(effect(&reference, "redoubt").get("any_attack").is_none());
    assert_eq!(effect(&forever, "redoubt")["proc_chance"], 0.1);

    let mut landed_only = forever.clone();
    effect_mut(&mut landed_only, "redoubt")
        .as_object_mut()
        .unwrap()
        .remove("any_attack");

    let (mut avoided, mut damaging, mut old_avoided, mut old_damaging) = (0, 0, 0, 0);
    for seed in 1..=40 {
        let (a, d) = redoubt_after_swings(&first_fight_log(&forever, seed));
        avoided += a;
        damaging += d;
        let (a, d) = redoubt_after_swings(&first_fight_log(&landed_only, seed));
        old_avoided += a;
        old_damaging += d;
    }
    assert!(avoided > 0, "Redoubt after an avoided swing");
    assert!(damaging > 0, "Redoubt after a damaging swing");
    assert_eq!(old_avoided, 0, "the old trigger needs a landed hit");
    assert!(old_damaging > 0);

    simulate(&forever);
}

// ---------------------------------------------------------------------------------------------
// Two-Handed Weapon Specialization.
// ---------------------------------------------------------------------------------------------

/// With 3 points and a two-hander the Holy damage of Judgement is 6% up, and Hammer of Wrath
/// and Exorcism are not; the reference's rules give the talent no Holy damage at all.
#[test]
fn two_handed_weapon_specialization_leaves_hammer_of_wrath_and_exorcism() {
    let request = request("production-retribution-paladin", RETRIBUTION_PALADIN);
    let forever = prepared(&request, Rules::Forever);
    let multiplier =
        |prepared: &Value, id: i64| spell(prepared, id)["damage_multiplier"].as_f64().unwrap();

    assert!((multiplier(&forever, JUDGEMENT) - 1.06).abs() < 1e-9);
    assert_eq!(multiplier(&forever, HAMMER_OF_WRATH), 1.0);
    assert_eq!(multiplier(&forever, EXORCISM), 1.0);
    simulate(&forever);
}

// ---------------------------------------------------------------------------------------------
// Measured chance on hit rates.
// ---------------------------------------------------------------------------------------------

/// The chance a swing of the warrior's main hand rolls the weapon's chance on hit.
fn swing_chance(item: i64, name: &str) -> f64 {
    let prepared = prepared(
        &wearing(
            "production-arms-warrior",
            ARMS_WARRIOR,
            WARRIOR_MAIN_HAND,
            item,
        ),
        Rules::Forever,
    );
    let proc = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "spell_data_damage_proc" && effect["trigger_aura"] == name)
        .unwrap_or_else(|| panic!("{name}'s chance on hit"));
    let chances: Vec<f64> = proc["chances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|chance| chance["chance"].as_f64().unwrap())
        .collect();
    assert!(chances.iter().all(|chance| *chance == chances[0]), "{name}");
    simulate(&prepared);
    chances[0]
}

/// Procs a minute times the weapon's speed over 60: Ripsaw 2 at 2.7, Bigger Rock on Bigger
/// Stick 2 at 3.4, Sword of Dismemberment 0.6 at 3.7. Boneslicer, which shares Ripsaw's Wound,
/// keeps the assumed 1.08 at 2.0.
#[test]
fn measured_chance_on_hit_weapons_roll_their_own_rate() {
    for (item, name, ppm, speed) in [
        (RIPSAW, "Ripsaw", 2.0, 2.7),
        (
            BIGGER_ROCK_ON_BIGGER_STICK,
            "Bigger Rock on Bigger Stick",
            2.0,
            3.4,
        ),
        (SWORD_OF_DISMEMBERMENT, "Sword of Dismemberment", 0.6, 3.7),
        (BONESLICER, "Boneslicer", 1.08, 2.0),
    ] {
        let chance = swing_chance(item, name);
        let expected = ppm * speed / 60.0;
        assert!(
            (chance - expected).abs() < 1e-6,
            "{name}: {chance}, expected {expected}"
        );
    }
}
