//! Forever's rule for a cast begun before the pull (`prepull_casts_finish_on_pull`): one that
//! haste or an early press would finish before 0:00 finishes on 0:00, as SimulationCraft executes
//! its precast at the start of combat and a player times a precast to land as the fight starts.
//! The Go reference finishes it early and drops its damage, which lands before 0:00
//! (docs/prepull-damage.md). The fixtures are the live board requests of 4 November 2026
//! (fixtures/prepull).

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::Value;
use std::{fs, path::Path};

const STARFIRE: &str = "{SpellID: 9876}";
const AIMED_SHOT: &str = "{SpellID: 20904}";

fn request(name: &str) -> Value {
    let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/prepull"))
        .join(format!("{name}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = 1.into();
    request
}

/// One fight's log lines and the player's spell metrics, prepared under Forever's rules with
/// the precast rule on or, as the reference has it, off.
fn fight(request: &Value, rule: bool) -> (Vec<String>, Value) {
    let mut prepared =
        prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever).unwrap();
    assert_eq!(prepared["player"]["prepull_casts_finish_on_pull"], true);
    prepared["player"]["prepull_casts_finish_on_pull"] = rule.into();
    let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&typed).unwrap();
    let logs = report.result["logs"].as_str().unwrap();
    let lines = logs.lines().map(str::to_string).collect();
    let player = report.result["raidMetrics"]["parties"][0]["players"][0].clone();
    (lines, player)
}

fn time(line: &str) -> f64 {
    line[1..line.find(']').unwrap()].parse().unwrap()
}

/// The time of the first line that names `id` and contains `event`.
fn first(lines: &[String], id: &str, event: &str) -> f64 {
    let line = lines
        .iter()
        .find(|line| line.contains(id) && line.contains(event))
        .unwrap_or_else(|| panic!("no {event} of {id}"));
    time(line)
}

/// Every hit and crit of `id` the log shows, with its time.
fn hits(lines: &[String], id: &str) -> Vec<(f64, f64)> {
    lines
        .iter()
        .filter(|line| line.contains(id))
        .filter_map(|line| {
            let at = line.find(" for ")?;
            let rest = &line[at + 5..];
            if !(line.contains("} Hit for ") || line.contains("} Crit for ")) {
                return None;
            }
            let damage = rest[..rest.find(' ')?].parse().ok()?;
            Some((time(line), damage))
        })
        .collect()
}

/// The damage the metrics count for the spell with this ID, over all its targets.
fn counted(player: &Value, spell_id: u64) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"].as_u64() == Some(spell_id))
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["damage"].as_f64().unwrap_or(0.0))
        .sum()
}

/// The Balance board's Skyborne Moonkin pressing Starfire at -3.0 s: 2% haste finishes the
/// 3 s cast at -0.06 s. Under Forever's rule it finishes on the pull and its damage counts;
/// without it, as in the reference, the hit is dropped.
#[test]
fn a_hasted_skyborne_starfire_lands_on_the_pull() {
    let request = request("balance-druid-skyborne-starfire-3s");

    let (lines, player) = fight(&request, true);
    assert_eq!(first(&lines, STARFIRE, "Casting"), -3.0);
    assert_eq!(first(&lines, STARFIRE, "Completed cast"), 0.0);
    let hits_forever = hits(&lines, STARFIRE);
    assert_eq!(hits_forever[0].0, 0.0);
    let logged: f64 = hits_forever.iter().map(|(_, damage)| damage).sum();
    assert!((counted(&player, 9876) - logged).abs() < 1e-6 * logged);

    let (lines, player) = fight(&request, false);
    assert_eq!(first(&lines, STARFIRE, "Completed cast"), -0.06);
    let hits_go = hits(&lines, STARFIRE);
    let (at, precast) = hits_go[0];
    assert_eq!(at, -0.06);
    assert!(precast > 0.0);
    let logged: f64 = hits_go.iter().map(|(_, damage)| damage).sum();
    // The reference counts every Starfire but the one that landed before the pull.
    assert!((counted(&player, 9876) - (logged - precast)).abs() < 1e-6 * logged);
}

/// The live preset presses Starfire at -2.9 s, which finishes after the pull either way: the
/// rule leaves it alone.
#[test]
fn a_precast_finishing_after_the_pull_is_unchanged() {
    let mut request = request("balance-druid-skyborne-starfire-3s");
    for action in request["raid"]["parties"][0]["players"][0]["rotation"]["prepullActions"]
        .as_array_mut()
        .unwrap()
    {
        if action["action"]["castSpell"]["spellId"]["spellId"] == 25298 {
            action["doAtValue"]["const"]["val"] = "-2.9s".into();
        }
    }
    let (with_rule, _) = fight(&request, true);
    let (without, _) = fight(&request, false);
    assert_eq!(first(&with_rule, STARFIRE, "Completed cast"), 0.04);
    assert_eq!(with_rule, without);
}

/// The Hunter turret board's Aimed Shot at -2.5 s: ranged haste makes its 2 s cast 1.74 s, so
/// the reference finishes it at -0.76 s and its arrow lands at -0.46 s, uncounted. Under
/// Forever's rule the cast finishes its 0.3 s flight before the pull, so the arrow lands on the
/// pull, as a player times it in game (25 November 2026; until then it finished on 0:00 and
/// landed 0.3 s in, SimulationCraft's precast).
#[test]
fn the_hunter_turret_aimed_shot_lands_on_the_pull() {
    let request = request("hunter-turret-aimed-shot");

    let (lines, player) = fight(&request, true);
    assert_eq!(first(&lines, AIMED_SHOT, "Completed cast"), -0.3);
    let hits_forever = hits(&lines, AIMED_SHOT);
    assert_eq!(hits_forever[0].0, 0.0);
    let logged: f64 = hits_forever.iter().map(|(_, damage)| damage).sum();
    assert!((counted(&player, 20904) - logged).abs() < 1e-6 * logged);

    let (lines, player) = fight(&request, false);
    assert_eq!(first(&lines, AIMED_SHOT, "Completed cast"), -0.76);
    let hits_go = hits(&lines, AIMED_SHOT);
    let (at, precast) = hits_go[0];
    assert_eq!(at, -0.46);
    let logged: f64 = hits_go.iter().map(|(_, damage)| damage).sum();
    assert!((counted(&player, 20904) - (logged - precast)).abs() < 1e-6 * logged);
}

/// Forever preparation writes the rule; the reference's rules leave it out.
#[test]
fn only_forever_preparation_finishes_precasts_on_the_pull() {
    let request = request("hunter-turret-aimed-shot").to_string();
    let forever = prepare_json_with(request.as_bytes(), "rust", Rules::Forever).unwrap();
    let go = prepare_json_with(request.as_bytes(), "rust", Rules::GoReference).unwrap();
    assert_eq!(
        forever["player"]["prepull_casts_finish_on_pull"],
        Value::Bool(true)
    );
    assert!(go["player"].get("prepull_casts_finish_on_pull").is_none());
}
