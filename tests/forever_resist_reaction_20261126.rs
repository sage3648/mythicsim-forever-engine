//! A failed dot cast waits a reaction time before its recast under Forever's rules
//! (`resist_waits_for_reaction`, docs/mechanics.md), from MythicSim's Discord (WizardXeze, 20
//! November 2026: a resisted spell was queued again back to back). The player sees an Immolate
//! miss only when its dot fails to appear, so the next Immolate starts no sooner than the
//! reaction time after the miss lands. The Go reference recasts it the instant the miss lands.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// Immolate's ranks: Forever's client and the reference prepare different top ranks.
const IMMOLATE: [i64; 8] = [348, 707, 1094, 2941, 11665, 11667, 11668, 25309];
const REACTION_MS: i64 = 200;

fn request() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-destruction-warlock.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["raid"]["parties"][0]["players"][0]["reactionTimeMs"] = json!(REACTION_MS);
    request["encounter"]["duration"] = json!(300);
    request["encounter"]["durationVariation"] = json!(0);
    request
}

fn prepare(rules: Rules) -> Value {
    prepare_json_with(request().to_string().as_bytes(), "rust", rules).unwrap()
}

/// The first fight's log of a one fight run on a seed.
fn first_fight_log(mut prepared: Value, seed: i64) -> String {
    prepared["sim"]["iterations"] = json!(1);
    prepared["sim"]["seed"] = json!(seed);
    prepared["sim"]["debug_first_iteration"] = json!(true);
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

fn at(line: &str) -> f64 {
    line[1..line.find(']').unwrap()].parse().unwrap()
}

/// The times Immolate's cast missed, and the times each Immolate cast began.
fn misses_and_casts(log: &str) -> (Vec<f64>, Vec<f64>) {
    let ids: Vec<String> = IMMOLATE
        .iter()
        .map(|id| format!("{{SpellID: {id}}}"))
        .collect();
    let misses = log
        .lines()
        .filter(|line| ids.iter().any(|id| line.contains(&format!("{id} Miss"))))
        .map(at)
        .collect();
    let casts = log
        .lines()
        .filter(|line| ids.iter().any(|id| line.contains(&format!("Casting {id}"))))
        .map(at)
        .collect();
    (misses, casts)
}

/// The gap from each miss to the next Immolate cast that began at or after it.
fn recast_gaps(log: &str) -> Vec<f64> {
    let (misses, casts) = misses_and_casts(log);
    misses
        .iter()
        .filter_map(|&miss| {
            casts
                .iter()
                .find(|&&cast| cast >= miss)
                .map(|&cast| cast - miss)
        })
        .collect()
}

#[test]
fn only_forever_preparation_writes_the_rule() {
    assert_eq!(
        prepare(Rules::Forever)["player"]["resist_waits_for_reaction"],
        json!(true)
    );
    assert!(prepare(Rules::GoReference)["player"]
        .get("resist_waits_for_reaction")
        .is_none());
}

#[test]
fn a_missed_immolate_is_recast_after_the_reaction_time() {
    let (forever, go) = (prepare(Rules::Forever), prepare(Rules::GoReference));
    let reaction = REACTION_MS as f64 / 1000.0;
    let (mut forever_gaps, mut go_gaps) = (Vec::new(), Vec::new());
    for seed in 1..=12 {
        forever_gaps.extend(recast_gaps(&first_fight_log(forever.clone(), seed)));
        go_gaps.extend(recast_gaps(&first_fight_log(go.clone(), seed)));
    }
    assert!(
        !forever_gaps.is_empty(),
        "no Immolate missed in twelve fights"
    );
    // The log keeps hundredths of a second.
    assert!(
        forever_gaps.iter().all(|&gap| gap >= reaction - 0.005),
        "an Immolate recast inside the reaction time: {forever_gaps:?}"
    );
    // The reference recasts at once, which is what the rule changes.
    assert!(
        go_gaps.iter().any(|&gap| gap < 0.005),
        "the reference never recast a missed Immolate at once: {go_gaps:?}"
    );
}
