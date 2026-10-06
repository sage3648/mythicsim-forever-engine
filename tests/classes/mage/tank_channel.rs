//! A tanking Mage's Arcane Missiles with a cast time, which Go's "Pushback trigger" pushes
//! back by a quarter of the cast time per landed hit. No spell Go registers is both a
//! channel and a hardcast, so the accepted requests name a player the exporter gives one
//! cast time for (`tools/oracle-v2/synthetic.go`); the Go goldens of both cases are compared
//! with the rest of the fixture family in `prepared_v2.rs`.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, simulate_prepared, PreparedError,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const MISSILES: &str = "{SpellID: 25345}";

fn accepted(case: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn parse(value: Value) -> PreparedV2 {
    serde_json::from_value(value).unwrap()
}

fn reasons(value: Value) -> Vec<String> {
    match check_prepared(&parse(value)) {
        Err(PreparedError::Unsupported(reasons)) => reasons,
        other => panic!("expected unsupported, got {other:?}"),
    }
}

fn effect<'a>(value: &'a mut Value, kind: &str) -> &'a mut Value {
    value["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap()
}

fn first_fight_log(mut value: Value) -> String {
    value["sim"]["iterations"] = json!(1);
    let report = simulate_prepared(&parse(value)).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

/// The lines of the log that carry `needle`, each with its time in seconds.
fn lines_with<'a>(log: &'a str, needle: &str) -> Vec<(f64, &'a str)> {
    log.lines()
        .filter(|line| line.contains(needle))
        .map(|line| (line[1..line.find(']').unwrap()].parse().unwrap(), line))
        .collect()
}

fn times(log: &str, needle: &str) -> Vec<f64> {
    lines_with(log, needle)
        .into_iter()
        .map(|(time, _)| time)
        .collect()
}

const CHANNEL_REFUSAL: &str =
    "rotation reaches spell 25345, a channel with a cast time while the target swings at the player";

/// The channel's pushback needs the trigger the exporter reads, the reduced avoidance rolls a
/// hardcast holds and a chance of one or none: every other chance still has no Go comparison.
#[test]
fn a_tanking_channel_with_a_cast_time_needs_the_trigger_and_a_certain_chance() {
    let value = accepted("tank-mage-channel-pushback");
    assert_eq!(check_prepared(&parse(value.clone())), Ok(()));
    // The channel is pushed back whether or not its row carries the pushback flag.
    let missiles = value["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["spell_id"] == 25345)
        .unwrap();
    assert_eq!(missiles["default_cast"]["cast_time_ns"], 3_000_000_000_u64);
    let flags = missiles["flags"].as_array().unwrap();
    assert!(flags.contains(&json!("SpellFlagChanneled")));
    assert!(!flags.contains(&json!("SpellFlagPushback")));

    let mut untriggered = value.clone();
    untriggered["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "pushback_trigger");
    assert!(reasons(untriggered).contains(&CHANNEL_REFUSAL.into()));

    let mut rolled = value.clone();
    effect(&mut rolled, "pushback_trigger")["chance"] = json!(0.65);
    assert_eq!(
        reasons(rolled),
        [
            "rotation reaches spell 25345, a channel with a cast time the target's swings push \
          back with a chance that needs a roll"
        ]
    );

    let mut unrolled = value;
    unrolled["enemy"]
        .as_object_mut()
        .unwrap()
        .remove("reduced_avoidance_rolls");
    assert!(reasons(unrolled).contains(&CHANNEL_REFUSAL.into()));
}

/// A chance the spell's resist cancels never rolls, so the cast is never pushed back and
/// Arcane Missiles completes at the end of its cast time.
#[test]
fn a_chance_the_resist_cancels_leaves_the_channel_alone() {
    let mut value = accepted("tank-mage-channel-pushback-after-cast");
    effect(&mut value, "pushback_trigger")["chance"] = json!(0.0);
    assert_eq!(check_prepared(&parse(value.clone())), Ok(()));
    let log = first_fight_log(value);
    assert!(!log.contains("pushed back"));
    // The first cast starts at 0.00 and its three seconds run in full.
    assert_eq!(times(&log, &format!("Completed cast {MISSILES}"))[0], 3.0);
}

/// Each hit takes a quarter of the cast time off the end of the cast, never moving it before
/// the moment the handler runs, and logs the quarter, as Go does. The first cast of seed 74 is
/// hit at 0.03 and 2.03: the first hit moves the end from 3.00 to 2.25, and the second would
/// move it to 1.50, which is already past, so the cast completes as the handler runs.
#[test]
fn each_hit_takes_a_quarter_of_the_cast_time_and_never_more_than_is_left() {
    let log = first_fight_log(accepted("tank-mage-channel-pushback-after-cast"));
    let pushbacks = lines_with(&log, "pushed back");
    assert_eq!(
        &pushbacks[..2],
        [
            (
                0.04,
                "[0.04] [synthetic-channel-cast-mage (#1)] {SpellID: 25345} pushed back 750ms \
                 while channeling"
            ),
            (
                2.04,
                "[2.04] [synthetic-channel-cast-mage (#1)] {SpellID: 25345} pushed back 750ms \
                 while channeling"
            ),
        ]
    );
    let completed = times(&log, &format!("Completed cast {MISSILES}"));
    assert_eq!(completed[0], 2.04);
    // A cast hit once and by the swing two seconds later: 15.04 to 18.04 loses 0.75 seconds
    // to the first hit at 16.03 and is over before the next swing lands.
    assert!(pushbacks.iter().any(|(time, _)| *time == 16.04));
    assert!(completed.contains(&17.29));
}

/// Since the fork's patch 89 a hit in the batch window before the cast completes leaves the
/// cast alone: the swing at 10.03 lands before the cast that began at 7.04 completes at 10.04,
/// and the channel completes once at its full time.
#[test]
fn a_hit_in_the_last_batch_window_leaves_the_channel_alone() {
    let log = first_fight_log(accepted("tank-mage-channel-pushback-after-cast"));
    assert!(times(&log, "{OtherID: 3, Tag: 1} Hit for").contains(&10.03));
    assert!(!times(&log, "pushed back").contains(&10.04));
    let completed = times(&log, &format!("Completed cast {MISSILES}"));
    assert_eq!(completed.iter().filter(|time| **time == 10.04).count(), 1);
}
