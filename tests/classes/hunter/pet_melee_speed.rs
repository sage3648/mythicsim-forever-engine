//! Under Forever's rules a Hunter's pet swings at its owner's melee and attack speed on top of
//! its own. Forever combat logs: the pet swung every 1.965 s under the owner's 2% Wind Blessed,
//! 1.40 s under Rapid Fire's 40% and 1.08 s with its own Frenzy as well, and the owner's ranged
//! haste left it alone. Each change reached the pet at its next 5.25 s stat heartbeat and
//! rescaled the swing under way.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// Rapid Fire's melee and ranged haste.
const RAPID_FIRE: f64 = 1.4;
/// The pet's swing and the log's two decimals of a second.
const SWING: f64 = 2.0;
const TOLERANCE: f64 = 0.011;
const HEARTBEAT: f64 = 5.25;

fn read(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// A production Forever Hunter (Marksmanship and Survival talents, so no Frenzy) with a cat that
/// swings every 2.0 s, of the given race, whose rotation casts Rapid Fire at 10 s when asked.
fn request(race: &str, rapid_fire: bool, duration: u32, seed: u64) -> Value {
    let mut request = read("fixtures/forever-gaps/hunter-skysight.request.json");
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["race"] = race.into();
    let options = &mut player["hunter"]["options"]["classOptions"];
    options["petType"] = "Cat".into();
    options["petAttackSpeed"] = "Two".into();
    let mut actions = Vec::new();
    if rapid_fire {
        actions.push(json!({"action": {
            "castSpell": {"spellId": {"spellId": 3045}},
            "condition": {"cmp": {"lhs": {"currentTime": {}}, "op": "OpGe",
                "rhs": {"const": {"val": "10s"}}}},
        }}));
    }
    player["rotation"] = json!({"type": "TypeAPL", "priorityList": actions});
    request["encounter"]["duration"] = duration.into();
    request["encounter"]["durationVariation"] = 0.into();
    request["simOptions"]["randomSeed"] = seed.to_string().into();
    request["simOptions"]["debugFirstIteration"] = true.into();
    request["simOptions"]["iterations"] = 1.into();
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules).unwrap()
}

fn time(line: &str) -> f64 {
    line[1..line.find(']').unwrap()].parse().unwrap()
}

/// The first fight's log, and the times of the pet's melee swings in it.
fn swings(request: &Value) -> (String, Vec<f64>) {
    let typed: PreparedV2 = serde_json::from_value(prepared(request, Rules::Forever)).unwrap();
    check_prepared(&typed).expect("the gate passes");
    let report = simulate_prepared(&typed).unwrap();
    let logs = report.result["logs"].as_str().unwrap().to_string();
    let times = logs
        .lines()
        .filter(|line| line.contains("- Cat] [Target 1] {OtherID: 3, Tag: 1}"))
        .filter(|line| !line.contains("[DEBUG]"))
        .map(time)
        .collect();
    (logs, times)
}

/// The swings' intervals, each with the time its swing started.
fn intervals(times: &[f64]) -> Vec<(f64, f64)> {
    times.windows(2).map(|w| (w[0], w[1] - w[0])).collect()
}

/// The mean interval from the first swing to the last, to well under the log's resolution.
fn mean_interval(times: &[f64]) -> f64 {
    (times[times.len() - 1] - times[0]) / (times.len() - 1) as f64
}

/// Forever's rules mark the Hunter's pet as following its owner's speed; the reference's does
/// not, and its fixtures stay as they were.
#[test]
fn only_forever_rules_pass_the_owners_speed_to_the_pet() {
    let forever = prepared(&request("RaceNightElf", false, 60, 1), Rules::Forever);
    assert_eq!(forever["pets"][0]["inherits_melee_speed"], true);
    assert!(forever["unrepresented"].as_array().unwrap().is_empty());
    let reference = prepared(
        &read("fixtures/mage/prepared-v2/production-beast-mastery-hunter.request.json"),
        Rules::GoReference,
    );
    assert!(reference["pets"][0].get("inherits_melee_speed").is_none());
}

/// Skyborne's Wind Blessed (2% attack speed) reaches the pet from the pull: it swings every
/// 2.0 / 1.02 = 1.961 s, and a Night Elf's pet every 2.0 s.
#[test]
fn wind_blessed_speeds_the_pet_from_the_pull() {
    let (_, skyborne) = swings(&request("RaceWindshaperSkyborne", false, 120, 1));
    let want = SWING / 1.02;
    assert!(
        (mean_interval(&skyborne) - want).abs() < 0.001,
        "the Skyborne's pet swings every {:.4} s, want {want:.4}",
        mean_interval(&skyborne)
    );
    let (_, night_elf) = swings(&request("RaceNightElf", false, 120, 1));
    assert!(
        (mean_interval(&night_elf) - SWING).abs() < 0.001,
        "the Night Elf's pet swings every {:.4} s",
        mean_interval(&night_elf)
    );
}

/// Rapid Fire at 10 s for 15 s: the pet swings every 2.0 s before it, every 2.0 / 1.4 s from
/// the first heartbeat after it starts and every 2.0 s again from the first heartbeat after it
/// ends. The heartbeat's offset is random, so over ten seeds some swings that start after Rapid
/// Fire's gain or fade still run at the old speed.
#[test]
fn rapid_fire_reaches_the_pet_at_its_next_heartbeat() {
    let (gain, fade) = (10.0, 25.0);
    let hasted = SWING / RAPID_FIRE;
    let (mut late_gain, mut late_fade) = (0, 0);
    for seed in 1..=10u64 {
        let (logs, times) = swings(&request("RaceNightElf", true, 50, seed));
        let aura = |what: &str| {
            logs.lines()
                .find(|line| {
                    line.contains("[Player (#1)] Aura")
                        && line.contains(what)
                        && line.contains("3045")
                })
                .map(time)
        };
        assert_eq!(aura("gained"), Some(gain), "seed {seed}");
        assert_eq!(aura("faded"), Some(fade), "seed {seed}");
        let mut first_after_gain = true;
        let mut first_after_fade = true;
        for (start, interval) in intervals(&times) {
            let end = start + interval;
            let near = |want: f64| (interval - want).abs() < TOLERANCE;
            if end <= gain || start >= fade + HEARTBEAT {
                assert!(near(SWING), "seed {seed}: {interval:.2} s from {start}");
            } else if start >= gain + HEARTBEAT && end <= fade {
                assert!(near(hasted), "seed {seed}: {interval:.2} s from {start}");
            } else {
                // A swing the change reached part way through runs between the two speeds.
                assert!(
                    interval > hasted - TOLERANCE && interval < SWING + TOLERANCE,
                    "seed {seed}: {interval:.2} s from {start}"
                );
            }
            if start >= gain && first_after_gain {
                first_after_gain = false;
                late_gain += usize::from(near(SWING));
            }
            if start >= fade && first_after_fade {
                first_after_fade = false;
                late_fade += usize::from(near(hasted));
            }
        }
    }
    assert!(
        late_gain > 0,
        "Rapid Fire reached the pet at once in every fight"
    );
    assert!(
        late_fade > 0,
        "Rapid Fire left the pet at once in every fight"
    );
}

/// Three Battle Squawk stacks (5% melee speed each) reach the pet from the pull, and their
/// expiry at four minutes leaves it at its next heartbeat.
#[test]
fn battle_squawk_reaches_the_pet_and_its_expiry_follows() {
    let mut request = request("RaceNightElf", false, 300, 1);
    request["raid"]["parties"][0]["battleSquawks"] = 3.into();
    let (_, times) = swings(&request);
    let squawked = SWING / 1.05f64.powi(3);
    let before: Vec<f64> = times.iter().copied().filter(|t| *t <= 240.0).collect();
    let after: Vec<f64> = times
        .iter()
        .copied()
        .filter(|t| *t >= 240.0 + HEARTBEAT)
        .collect();
    assert!(
        (mean_interval(&before) - squawked).abs() < 0.001,
        "with the stacks up the pet swings every {:.4} s, want {squawked:.4}",
        mean_interval(&before)
    );
    assert!(
        (mean_interval(&after) - SWING).abs() < 0.001,
        "after the stacks the pet swings every {:.4} s",
        mean_interval(&after)
    );
}
