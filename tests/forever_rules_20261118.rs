//! Forever's rules from public Forever combat logs of 10 to 17 November 2026 (client
//! 1.60.1.70938, after push 114115), on the live board references of 17 November:
//!
//! - Measured proc rates: Felstriker 0.5 a minute, Bashguuder 3.2, Rogue Armor Energize 0.8.
//! - Warrior's Resolve (Battlegear of Valor 5, Battlegear of Heroism 4): white swings and the
//!   Windfury Totem strike roll it, no melee ability does.
//! - Crusader's Wrath (Lightforge Armor 5, Soulforge Armor 4): 0.9 a minute on white swings, and
//!   on each cast that costs mana and is no melee or ranged ability, priced on its cast time.
//! - Flurry: the Windfury Totem strike spends no charge. Reckless Swings: a Heroic Strike or
//!   Cleave grants once a cast. Deep Wounds: a Thunder Clap crit starts no bleed.
//!
//! The Go reference keeps its own rules.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn request(spec: &str, iterations: u32) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/mage/prepared-v2/board-{spec}-20261117.request.json"
    ));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(iterations);
    request
}

fn prepare(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn run(prepared: &Value) -> Value {
    let prepared = serde_json::from_value(prepared.clone()).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["raidMetrics"]["parties"][0]["players"][0].clone()
}

fn effect<'a>(prepared: &'a Value, kind: &str, label: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["kind"] == kind
                && (effect["rng_label"] == label || effect["trigger_aura"] == label)
        })
        .unwrap_or_else(|| panic!("no {kind} effect for {label}"))
}

fn effect_mut<'a>(prepared: &'a mut Value, kind: &str) -> &'a mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

/// The action ids of the spells an effect's chances name, with each chance.
fn chances(prepared: &Value, effect: &Value, field: &str) -> Vec<(Value, f64)> {
    let spells = prepared["player"]["spells"].as_array().unwrap();
    effect[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let spell = &spells[entry["spell"].as_u64().unwrap() as usize];
            (
                spell["action_id"].clone(),
                entry["chance"].as_f64().unwrap(),
            )
        })
        .collect()
}

fn is_auto(action: &Value) -> bool {
    action["other_id"] == "OtherActionAttack"
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// An aura's uptime in seconds an iteration.
fn uptime(player: &Value, spell_id: i64) -> f64 {
    player["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["id"]["spellId"].as_i64() == Some(spell_id))
        .and_then(|aura| aura["uptimeSecondsAvg"].as_f64())
        .unwrap_or(0.0)
}

#[test]
fn felstriker_rolls_half_a_proc_a_minute() {
    // The Combat Rogue reference wields Felstriker (1.7 sec) in the off hand.
    let prepared = prepare(&request("combat-rogue", 10));
    let felstriker = effect(&prepared, "stat_proc", "Felstriker");
    let rolled = chances(&prepared, felstriker, "chances");
    assert!(!rolled.is_empty());
    for (_, chance) in rolled {
        assert!(close(chance, 0.5 * 1.7 / 60.0), "{chance}");
    }
}

#[test]
fn bashguuder_rolls_three_point_two_a_minute() {
    // The Fury reference wields Bashguuder (1.8 sec).
    let prepared = prepare(&request("warrior", 10));
    let bashguuder = effect(&prepared, "armor_debuff_proc", "Bashguuder Proc");
    for entry in bashguuder["chances"].as_array().unwrap() {
        let chance = entry["chance"].as_f64().unwrap();
        assert!(close(chance, 3.2 * 1.8 / 60.0), "{chance}");
    }
}

#[test]
fn rogue_armor_energize_rolls_point_eight_a_minute() {
    // Main hand 2.4 sec, off hand Felstriker 1.7 sec.
    let prepared = prepare(&request("combat-rogue", 10));
    let energize = effect(&prepared, "energy_proc", "Rogue Armor Energize");
    let rolled = chances(&prepared, energize, "chances");
    assert!(rolled
        .iter()
        .any(|(action, chance)| action["tag"] == 1 && close(*chance, 0.8 * 2.4 / 60.0)));
    assert!(rolled
        .iter()
        .any(|(action, chance)| action["tag"] == 2 && close(*chance, 0.8 * 1.7 / 60.0)));
}

#[test]
fn warriors_resolve_hears_white_swings_and_the_totem_strike_only() {
    let prepared = prepare(&request("fury-protection-warrior", 10));
    let resolve = effect(&prepared, "health_rage_proc", "Warrior's Resolve");
    let rolled = chances(&prepared, resolve, "chances");
    assert!(
        rolled.iter().all(|(action, _)| is_auto(action)),
        "{rolled:?}"
    );
    // The main hand, the off hand and the Windfury Totem strike.
    let tags: Vec<i64> = rolled
        .iter()
        .map(|(action, _)| action["tag"].as_i64().unwrap())
        .collect();
    assert_eq!(tags, [1, 2, 25584]);
}

#[test]
fn crusaders_wrath_rolls_on_swings_and_mana_casts() {
    let request = request("protection-paladin", 300);
    let prepared = prepare(&request);
    let wrath = effect(
        &prepared,
        "stat_proc",
        "Crusader's Wrath Proc - Soulforge Armor",
    );
    let swings = chances(&prepared, wrath, "chances");
    assert!(!swings.is_empty());
    assert!(swings.iter().all(|(action, _)| is_auto(action)));
    let casts = chances(&prepared, wrath, "cast_chances");
    let cast = |id: i64| {
        casts
            .iter()
            .find(|(action, _)| action["spell_id"] == id)
            .map(|(_, chance)| *chance)
    };
    // Judgement and Consecration are instants; Holy Light rank 9 casts in 2.5 sec.
    assert!(close(cast(20271).unwrap(), 0.9 * 1.5 / 60.0));
    assert!(close(cast(20922).unwrap(), 0.9 * 1.5 / 60.0));
    assert!(close(cast(25292).unwrap(), 0.9 * 2.5 / 60.0));
    // Holy Strike is a melee ability, Hammer of Wrath a ranged one.
    for id in [10333_i64, 679, 24275, 24274, 24239] {
        assert!(cast(id).is_none(), "{id} rolls on cast");
    }

    // Without the casts the buff is up less.
    let with_casts = uptime(&run(&prepared), 27499);
    let mut swings_only = prepared.clone();
    swings_only["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .filter(|effect| effect["kind"] == "stat_proc")
        .for_each(|effect| effect["cast_chances"] = json!([]));
    let without = uptime(&run(&swings_only), 27499);
    assert!(with_casts > without + 1.0, "{with_casts} vs {without}");
}

#[test]
fn the_totem_strike_spends_no_flurry_charge() {
    let prepared = prepare(&request("warrior", 300));
    assert_eq!(
        effect(&prepared, "warrior_flurry", "Flurry - Trigger")["totem_strike_spends_none"],
        true
    );
    let spared = uptime(&run(&prepared), 12966);
    let mut spending = prepared.clone();
    effect_mut(&mut spending, "warrior_flurry")["totem_strike_spends_none"] = json!(false);
    let spent = uptime(&run(&spending), 12966);
    assert!(spared > spent, "{spared} vs {spent}");
}

#[test]
fn reckless_swings_grants_once_a_cleave() {
    // Two targets, so a Cleave hits twice at once.
    let mut request = request("warrior", 300);
    let target = request["encounter"]["targets"][0].clone();
    request["encounter"]["targets"]
        .as_array_mut()
        .unwrap()
        .push(target);
    let prepared = prepare(&request);
    assert_eq!(
        effect(&prepared, "reckless_swings", "Reckless Swings - Trigger")["strikes_spend_once"],
        true
    );
    let rage = |player: &Value| -> f64 {
        player["resources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["id"]["spellId"].as_i64() == Some(1329555))
            .filter_map(|row| row["gain"].as_f64())
            .sum()
    };
    let once = rage(&run(&prepared));
    let mut each = prepared.clone();
    effect_mut(&mut each, "reckless_swings")["strikes_spend_once"] = json!(false);
    let per_target = rage(&run(&each));
    assert!(once > 0.0);
    assert!(per_target > once, "{per_target} vs {once}");
}

#[test]
fn a_thunder_clap_crit_starts_no_deep_wounds() {
    let prepared = prepare(&request("warrior", 10));
    assert_eq!(
        effect(&prepared, "deep_wounds", "Deep Wounds - Trigger")["thunder_clap_starts_none"],
        true
    );
}
