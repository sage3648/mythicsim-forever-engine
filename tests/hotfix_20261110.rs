//! Blizzard's WoW Forever hotfixes of 10 November 2026, the two lines the Enhancement Shaman
//! release (engine #276) and the class tuning of 11 November left, under Forever's rules only:
//!
//! - Beast Mastery: "Melee damage dealt by the Hawk pet increased by about 80%." The hawk's swing
//!   was read from level 30 logs taken before it, so its fixed part and its slope on the owner's
//!   ranged attack power both take 1.8. No client row states the hawk's damage.
//! - Restoration: "Nature's Swiftness now also makes Wild Growth instant." The talent's row still
//!   names Healing Touch and Regrowth alone; the reference keeps them.

use forever_engine::{check_prepared, prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The client's level 60 creature auto attack dps (`tests/classes/hunter/pet_models.rs`).
const CREATURE_DPS: f64 = 60.452_915_191_650_39;
/// Unleashed Fury 5/5, which every hawk log had.
const LOGGED_UNLEASHED_FURY: f64 = 1.15;

const NATURES_SWIFTNESS: i64 = 17116;
const WILD_GROWTH: i64 = 1238215;
/// The Restoration druid reference build's talents in hotfix push 113531's order, and in the
/// reference's.
const RESTORATION_DRUID: &str = "05302001--5053025153113051";
const RESTORATION_DRUID_REFERENCE: &str = "05002001--5050035153113051";

fn fixture(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn simulate(prepared: &Value) -> Value {
    let prepared = serde_json::from_value(prepared.clone()).unwrap();
    check_prepared(&prepared).unwrap();
    simulate_prepared(&prepared).unwrap().result
}

fn effect<'a>(prepared: &'a Value, kind: &str) -> Option<&'a Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
}

/// The hawk's swing under Forever's rules is 1.8 times what the pre-hotfix logs read, in its
/// fixed part and in its slope on ranged attack power; the reference's hawk has no guardian.
#[test]
fn the_hawk_swings_80_percent_harder() {
    let request = fixture("fixtures/prepull/hunter-turret-aimed-shot.request.json");
    let forever = prepared(&request, Rules::Forever);
    let guardian = &effect(&forever, "summon_hawk").unwrap()["guardian"];
    let average = guardian["swing_average"].as_f64().unwrap();
    let slope = guardian["attack_power_coefficient"].as_f64().unwrap();
    let logged_average = CREATURE_DPS * 0.0815 * 2.5 / LOGGED_UNLEASHED_FURY;
    let logged_slope = 0.0179 / LOGGED_UNLEASHED_FURY;
    assert!((average / logged_average - 1.8).abs() < 1e-12, "{average}");
    assert!((slope / logged_slope - 1.8).abs() < 1e-12, "{slope}");
    // Spread and crit share are untouched.
    assert!((guardian["swing_spread"].as_f64().unwrap() - 1.0 / 3.0).abs() < 1e-12);
    assert_eq!(guardian["crit_share"], 0.5);

    let reference = prepared(&request, Rules::GoReference);
    assert!(effect(&reference, "summon_hawk")
        .unwrap()
        .get("guardian")
        .is_none());
}

/// A Restoration druid's request casting Nature's Swiftness, then Wild Growth while it is up,
/// then Wild Growth again.
fn druid(talents: &str) -> Value {
    let mut request = fixture("fixtures/healers/restoration-druid.request.json");
    request["simOptions"]["iterations"] = json!(5);
    request["simOptions"]["debugFirstIteration"] = json!(true);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!(talents);
    let cast = |spell: i64, condition: Option<Value>| {
        // Nature's Swiftness is cast with no target, the heals on the druid.
        let mut line = json!({"action": {"castSpell": {"spellId": {"spellId": spell}}}});
        if spell != NATURES_SWIFTNESS {
            line["action"]["castSpell"]["target"] = json!({"type": "Self"});
        }
        if let Some(condition) = condition {
            line["action"]["condition"] = condition;
        }
        line
    };
    let swiftness_up = json!({"auraIsActive": {"auraId": {"spellId": NATURES_SWIFTNESS}}});
    player["rotation"] = json!({"type": "TypeAPL", "priorityList": [
        cast(WILD_GROWTH, Some(swiftness_up)),
        cast(NATURES_SWIFTNESS, None),
        cast(WILD_GROWTH, None),
    ]});
    request
}

/// The spellbook position of a spell's cast.
fn position(prepared: &Value, spell_id: i64) -> u64 {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .position(|spell| {
            spell["action_id"]["spell_id"] == spell_id && spell["action_id"].get("tag").is_none()
        })
        .unwrap_or_else(|| panic!("no spell {spell_id}")) as u64
}

/// Nature's Swiftness takes all of Wild Growth's cast time under Forever's rules: its instant
/// cast aura changes Wild Growth, and in the fights the Wild Growth after it casts at once and
/// spends it, the next one taking its full time.
#[test]
fn natures_swiftness_makes_wild_growth_instant() {
    let forever = prepared(&druid(RESTORATION_DRUID), Rules::Forever);
    let aura = effect(&forever, "instant_cast_aura").expect("Nature's Swiftness");
    assert_eq!(aura["spell_id"], NATURES_SWIFTNESS);
    let growth = position(&forever, WILD_GROWTH);
    let change = aura["changes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|change| change["spell"] == growth)
        .expect("a change on Wild Growth");
    assert_eq!(change["kind"], "cast_time_percent");
    assert_eq!(change["value"], -1.0);

    let result = simulate(&forever);
    let log = result["logs"].as_str().unwrap();
    let after_swiftness = log
        .split_once(&format!("Completed cast {{SpellID: {NATURES_SWIFTNESS}}}"))
        .expect("Nature's Swiftness was cast")
        .1;
    let casting: Vec<&str> = after_swiftness
        .lines()
        .filter(|line| {
            line.contains("Casting") && line.contains(&format!("{{SpellID: {WILD_GROWTH}}}"))
        })
        .take(2)
        .collect();
    assert_eq!(
        casting.len(),
        2,
        "two Wild Growths after Nature's Swiftness"
    );
    assert!(casting[0].contains("Cast Time = 0s"), "{}", casting[0]);
    assert!(!casting[1].contains("Cast Time = 0s"), "{}", casting[1]);
}

/// The reference's Nature's Swiftness leaves Wild Growth's cast time alone.
#[test]
fn the_reference_keeps_wild_growth_out_of_natures_swiftness() {
    let reference = prepared(&druid(RESTORATION_DRUID_REFERENCE), Rules::GoReference);
    if let Some(aura) = effect(&reference, "instant_cast_aura") {
        let growth = position(&reference, WILD_GROWTH);
        assert!(aura["changes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|change| change["spell"] != growth));
    }
}
