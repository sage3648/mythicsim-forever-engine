//! Two item rules read on 13 November 2026 from the live Forever client rows (wago.tools,
//! `product=wowf`) beside the Wowhead Forever tooltips of that day, under Forever's rules only:
//!
//! - Ironfoe (11684): "Attacks have a chance to grant 2 extra attacks on your next swing. Attacks
//!   against Orcs are 2 times as likely to activate this effect. (Proc chance: 6%, 100ms
//!   cooldown)". The row's 6 is the chance against Orcs, so Fury of Forgewright rolls 3% on the
//!   targets the sims fight. The reference keeps its 6%.
//! - Soulforge Armor (ItemSet 516, the Tier 0.5 Paladin set): (2) +8 All Resistances, (4)
//!   Crusader's Wrath, 65 spell damage and healing for 10 sec on a melee autoattack, (6) +40
//!   Attack Power, (8) +200 Armor. Before, its pieces were plain items.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const IRONFOE: i64 = 11684;
const CRUSADERS_WRATH: i64 = 27499;

const HEAD: usize = 0;
const SHOULDER: usize = 2;
const CHEST: usize = 4;
const WRIST: usize = 5;
const HANDS: usize = 6;
const WAIST: usize = 7;
const LEGS: usize = 8;
const FEET: usize = 9;
const MAIN_HAND: usize = 14;

/// The Soulforge pieces in the order the tests add them.
const SOULFORGE: [(usize, i64); 8] = [
    (HEAD, 22091),
    (SHOULDER, 22093),
    (CHEST, 22089),
    (HANDS, 22090),
    (LEGS, 22092),
    (WAIST, 22086),
    (WRIST, 22088),
    (FEET, 22087),
];

/// The armor slots the Soulforge tests fill, emptied first so a difference is the items' alone.
const ARMOR_SLOTS: [usize; 8] = [HEAD, SHOULDER, CHEST, WRIST, HANDS, WAIST, LEGS, FEET];

fn read(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The Protection Paladin of `fixtures/forever-gaps`, who melees the target, in these armor
/// pieces alone.
fn paladin(items: &[(usize, i64)]) -> Value {
    let mut request = read("fixtures/forever-gaps/protection-paladin-quel-serrar.request.json");
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
    request
}

fn prepare(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"))
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap()
}

fn has_aura(prepared: &Value, label: &str) -> bool {
    prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .any(|aura| aura["label"] == label)
}

fn effect<'a>(prepared: &'a Value, kind: &str, trigger_aura: &str) -> Option<&'a Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind && effect["trigger_aura"] == trigger_aura)
}

/// What the bonus at `pieces` adds to a stat: the step from one piece fewer, less the last
/// piece's own stats.
fn bonus(pieces: usize, name: &str) -> f64 {
    let with = prepare(&paladin(&SOULFORGE[..pieces]), Rules::Forever);
    let without = prepare(&paladin(&SOULFORGE[..pieces - 1]), Rules::Forever);
    let last = prepare(&paladin(&SOULFORGE[pieces - 1..pieces]), Rules::Forever);
    let naked = prepare(&paladin(&[]), Rules::Forever);
    (stat(&with, name) - stat(&without, name)) - (stat(&last, name) - stat(&naked, name))
}

#[test]
fn ironfoe_rolls_three_percent_under_forevers_rules() {
    let mut request = paladin(&[]);
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"][MAIN_HAND] =
        json!({ "id": IRONFOE });
    let forever = prepare(&request, Rules::Forever);
    let fury = effect(&forever, "extra_attack_proc", "Fury of Forgewright")
        .expect("Fury of Forgewright");
    assert_eq!(fury["proc_chance"], 0.03);
    assert_eq!(fury["attacks"], 2);
}

#[test]
fn soulforge_armor_bonuses_apply_at_two_four_six_and_eight_pieces() {
    let three = prepare(&paladin(&SOULFORGE[..3]), Rules::Forever);
    assert!(has_aura(&three, "Soulforge Armor 2P"));
    assert!(!has_aura(&three, "Soulforge Armor 4P"));
    assert!(effect(&three, "stat_proc", "Soulforge Armor 4P").is_none());

    let four = prepare(&paladin(&SOULFORGE[..4]), Rules::Forever);
    assert!(has_aura(&four, "Soulforge Armor 4P"));
    let wrath = effect(&four, "stat_proc", "Soulforge Armor 4P").expect("Crusader's Wrath proc");
    assert_eq!(wrath["aura"], "Crusader's Wrath");
    assert!(!wrath["chances"].as_array().unwrap().is_empty());

    let eight = prepare(&paladin(&SOULFORGE), Rules::Forever);
    for pieces in [2, 4, 6, 8] {
        assert!(has_aura(&eight, &format!("Soulforge Armor {pieces}P")));
    }

    assert_eq!(bonus(2, "FireResistance"), 8.0);
    assert_eq!(bonus(6, "AttackPower"), 40.0);
    assert!(bonus(8, "Armor") >= 200.0, "{}", bonus(8, "Armor"));
}

/// In fights the four piece bonus gains Crusader's Wrath, about three times a minute.
#[test]
fn four_soulforge_pieces_gain_crusaders_wrath_in_a_fight() {
    let value = prepare(&paladin(&SOULFORGE[..4]), Rules::Forever);
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    let player = &result["raidMetrics"]["parties"][0]["players"][0];
    let wrath = player["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["id"]["spellId"].as_i64() == Some(CRUSADERS_WRATH))
        .expect("Crusader's Wrath metrics");
    let procs = wrath["procsAvg"].as_f64().unwrap_or(0.0);
    let uptime = wrath["uptimeSecondsAvg"].as_f64().unwrap_or(0.0);
    assert!(procs > 0.0 || uptime > 0.0, "{wrath}");
}
