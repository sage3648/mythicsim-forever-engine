//! The healer spells MythicSim's healer sims cast past each class's heals (README, Rust-only
//! mechanics): Penance's heal for a Discipline priest, Tranquility and Nature's Swiftness for a
//! Restoration druid, and Nature's Swiftness and Water Shield for a Restoration shaman. Each test
//! puts one of `fixtures/healers`' requests on a rotation that casts the spell, then checks the
//! export, that the request passes the gate, and what the spell did in the fights.

use forever_engine::{check_prepared, prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// A healer fixture's request on these talents, casting `spells` (client spell IDs) in order,
/// each on the player but the self buffs.
fn request(name: &str, talents: &str, spells: &[(i64, Option<Value>)]) -> Value {
    let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/healers"))
        .join(format!("{name}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!(talents);
    let list: Vec<Value> = spells
        .iter()
        .map(|(id, condition)| {
            let mut cast = json!({"spellId": {"spellId": id}});
            if !SELF_BUFFS.contains(id) {
                cast["target"] = json!({"type": "Self"});
            }
            let mut action = json!({"castSpell": cast});
            if let Some(condition) = condition {
                action["condition"] = condition.clone();
            }
            json!({ "action": action })
        })
        .collect();
    player["rotation"] = json!({"type": "TypeAPL", "priorityList": list});
    request["simOptions"]["iterations"] = json!(20);
    request
}

/// Nature's Swiftness (Druid, Shaman) and Water Shield: cast with no target.
const SELF_BUFFS: &[i64] = &[16188, 17116, 408510];

const DISCIPLINE: &str = "005205031332011531-03505003";
const RESTORATION_DRUID: &str = "05002001--5050035153113051";
const RESTORATION_SHAMAN: &str = "-055-5531500015513151";

fn active(spell: i64) -> Option<Value> {
    Some(json!({"auraIsActive": {"auraId": {"spellId": spell}}}))
}

fn inactive(spell: i64) -> Option<Value> {
    Some(json!({"not": {"val": {"auraIsActive": {"auraId": {"spellId": spell}}}}}))
}

fn prepared(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever).unwrap()
}

fn effects<'a>(prepared: &'a Value, kind: &str) -> Vec<&'a Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == kind)
        .collect()
}

fn heal(prepared: &Value, spell_id: i64) -> &Value {
    effects(prepared, "heals")[0]["heals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|heal| heal["spell_id"] == spell_id)
        .unwrap_or_else(|| panic!("no heal for spell {spell_id}"))
}

/// The prepared request passes the gate; the fights' result.
fn simulate(prepared: &Value) -> Value {
    let prepared = serde_json::from_value(prepared.clone()).unwrap();
    check_prepared(&prepared).unwrap();
    simulate_prepared(&prepared).unwrap().result
}

/// What a spell did in every fight together, summed over its targets.
fn sum(result: &Value, spell: i64, key: &str) -> f64 {
    result["raidMetrics"]["parties"][0]["players"][0]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"] == spell)
        .flat_map(|action| action["targets"].as_array().unwrap().iter())
        .map(|target| target[key].as_f64().unwrap_or(0.0))
        .sum()
}

/// The first fight's log line after the first cast of `spell`.
fn cast_after(result: &Value, spell: i64) -> String {
    let log = result["logs"].as_str().unwrap();
    let marker = format!("Completed cast {{SpellID: {spell}}}");
    let (_, rest) = log
        .split_once(&marker)
        .unwrap_or_else(|| panic!("the first fight never casts {spell}"));
    rest.lines()
        .find(|line| line.contains("Casting"))
        .unwrap()
        .to_string()
}

/// A Discipline priest's Penance (1316995) heals: a bolt at once and one a second for two
/// more, each of the bolt row's healing, and no damage spell under it.
#[test]
fn penance_heals_for_a_discipline_priest() {
    let request = request("holy-priest", DISCIPLINE, &[(1316995, None), (10917, None)]);
    let prepared = prepared(&request);
    let penance = heal(&prepared, 1316995);
    assert!(penance["direct"]["average"].as_f64().unwrap() > 0.0);
    assert_eq!(penance["direct"]["average"], penance["tick"]);
    assert!(effects(&prepared, "penance").is_empty());
    let result = simulate(&prepared);
    assert!(sum(&result, 1316995, "casts") > 0.0);
    let ticks = sum(&result, 1316995, "ticks") + sum(&result, 1316995, "critTicks");
    let hits = sum(&result, 1316995, "hits") + sum(&result, 1316995, "crits");
    // Two ticks a cast, less those of a channel the fight's end cuts.
    assert!(
        ticks <= 2.0 * hits && ticks >= 2.0 * (hits - 20.0),
        "{ticks} ticks, {hits} casts"
    );
    assert!(sum(&result, 1316995, "healing") > 0.0);
}

/// Tranquility (9863) heals the party of five each 2 sec over its 10 sec channel, which a
/// 5 minute cooldown follows.
#[test]
fn tranquility_heals_the_party_over_its_channel() {
    let request = request(
        "restoration-druid",
        RESTORATION_DRUID,
        &[(9863, None), (25297, None)],
    );
    let prepared = prepared(&request);
    let tranquility = heal(&prepared, 9863);
    assert_eq!(tranquility["targets"], 5);
    assert_eq!(tranquility["tick"], 285.0);
    assert!(tranquility.get("direct").is_none());
    let result = simulate(&prepared);
    // One channel a fight: 3 minutes hold one cooldown.
    assert_eq!(sum(&result, 9863, "casts"), 20.0);
    let ticks = sum(&result, 9863, "ticks") + sum(&result, 9863, "critTicks");
    assert_eq!(ticks, 20.0 * 5.0 * 5.0, "five ticks on five players");
    assert!(sum(&result, 9863, "healing") > 0.0);
}

/// Nature's Swiftness makes the next heal with a cast time instant, and that cast spends it:
/// the Druid's (17116) on Healing Touch, the Shaman's (16188) on Healing Wave.
#[test]
fn natures_swiftness_makes_the_next_heal_instant() {
    for (name, talents, swiftness, heal) in [
        ("restoration-druid", RESTORATION_DRUID, 17116, 25297),
        ("restoration-shaman", RESTORATION_SHAMAN, 16188, 25357),
    ] {
        let request = request(
            name,
            talents,
            &[(heal, active(swiftness)), (swiftness, None), (heal, None)],
        );
        let prepared = prepared(&request);
        let aura = effects(&prepared, "instant_cast_aura");
        assert_eq!(aura.len(), 1, "{name}");
        assert_eq!(aura[0]["spell_id"], swiftness);
        let result = simulate(&prepared);
        let next = cast_after(&result, swiftness);
        assert!(
            next.contains(&format!("{{SpellID: {heal}}}")) && next.contains("Cast Time = 0s"),
            "{name}: {next}"
        );
        // Spent: the cast after it takes its full time again.
        let after = cast_after(&result, heal);
        assert!(!after.contains("Cast Time = 0s"), "{name}: {after}");
    }
}

/// Water Shield (408510): three globes, each spent by a direct heal that crits, at most once
/// per 3.5 sec, for 2% of the maximum mana.
#[test]
fn water_shield_globes_restore_mana_on_crit_heals() {
    let request = request(
        "restoration-shaman",
        RESTORATION_SHAMAN,
        &[(408510, inactive(408510)), (10623, None)],
    );
    let prepared = prepared(&request);
    // The class effect alone: a healer export carries no second globes effect that could bind
    // beside it and double the mana.
    assert!(effects(&prepared, "mana_globes").is_empty());
    let globes = effects(&prepared, "water_shield");
    assert_eq!(globes.len(), 1);
    assert_eq!(globes[0]["mana_percent"], 2.0);
    assert_eq!(globes[0]["globes"], 3);
    assert_eq!(globes[0]["icd_ns"], 3_500_000_000_i64);
    let result = simulate(&prepared);
    let mana = result["raidMetrics"]["parties"][0]["players"][0]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["id"]["spellId"] == 408511)
        .expect("the globes' mana metrics");
    let (events, gain) = (
        mana["events"].as_f64().unwrap(),
        mana["gain"].as_f64().unwrap(),
    );
    assert!(events > 0.0);
    // At most one globe each 3.5 sec of the 3 minute fights.
    assert!(events <= 20.0 * (180.0 / 3.5 + 1.0), "{events} globes");
    assert!(sum(&result, 408510, "casts") >= events / 3.0);
    assert!(gain / events > 100.0, "{} mana a globe", gain / events);
}
