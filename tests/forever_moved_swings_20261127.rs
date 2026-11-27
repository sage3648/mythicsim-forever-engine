//! An auto attack keeps its swing time when the list of attacks is reordered under the pass
//! that swings them (`moved_swings_stay_due`, docs/mechanics.md), from a production Best Gear
//! run that stopped with "cannot wait negative time" (27 November 2026).
//!
//! The request is that run's gear set: a Subtlety Rogue with Dragon's Call (10847) in the main
//! hand and Distracting Dagger in the off hand. On the pinned seed the Emerald Dragon Whelp is
//! out, mid swing timer, when the Rogue's rotation answers an off hand swing with Vanish,
//! Premeditation and Hemorrhage. Vanish takes the Rogue's two attacks off the list and
//! Hemorrhage, breaking Stealth, puts them back, which moves the whelp's attack to a place the
//! pass has gone by. The Go reference then forgets when the whelp swings, so the whelp's
//! rotation, which waits for that swing, finds it in the past and panics.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The fight of the production run (seed 42, fight 777) that stopped it.
const SEED: i64 = 818;
const VANISH: &str = "Casting {SpellID: 1856}";
const ACID_SPIT: &str = "Casting {SpellID: 9591}";
const WHELP: &str = "Emerald Dragon Whelp]";
/// The whelp's main hand swing.
const WHELP_SWING: &str = "Casting {OtherID: 3, Tag: 1}";
const SWING_SECONDS: f64 = 2.0;
const SPIT_SECONDS: f64 = 3.0;

fn prepare(rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/forever-gaps/subtlety-rogue-dragons-call-vanish.request.json");
    prepare_json_with(&fs::read(path).unwrap(), "rust", rules).unwrap()
}

/// The log of one fight on the pinned seed.
fn fight_log(mut prepared: Value) -> String {
    prepared["sim"]["iterations"] = json!(1);
    prepared["sim"]["seed"] = json!(SEED);
    prepared["sim"]["debug_first_iteration"] = json!(true);
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

fn at(line: &str) -> f64 {
    line[1..line.find(']').unwrap()].parse().unwrap()
}

fn times(log: &str, unit: &str, what: &str) -> Vec<f64> {
    log.lines()
        .filter(|line| line.contains(unit) && line.contains(what))
        .map(at)
        .collect()
}

/// A request both rule sets prepare: the reference cannot read the Rogue's Forever talents.
fn prepare_warlock(rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-destruction-warlock.request.json");
    prepare_json_with(&fs::read(path).unwrap(), "rust", rules).unwrap()
}

#[test]
fn only_forever_preparation_writes_the_rule() {
    assert_eq!(
        prepare(Rules::Forever)["player"]["moved_swings_stay_due"],
        json!(true)
    );
    assert_eq!(
        prepare_warlock(Rules::Forever)["player"]["moved_swings_stay_due"],
        json!(true)
    );
    assert!(prepare_warlock(Rules::GoReference)["player"]
        .get("moved_swings_stay_due")
        .is_none());
}

#[test]
fn the_whelp_swings_on_time_when_vanish_restarts_the_rogues_attacks() {
    let log = fight_log(prepare(Rules::Forever));
    let swings = times(&log, WHELP, WHELP_SWING);
    let spits = times(&log, WHELP, ACID_SPIT);
    let vanishes = times(&log, "", VANISH);
    // The case itself: a Vanish between two of the whelp's swings. A change to the fight's
    // random rolls can move it off this seed; pin another fight that has it.
    let between = vanishes.iter().any(|&vanish| {
        swings
            .windows(2)
            .any(|pair| pair[0] < vanish && vanish < pair[1] && pair[1] - pair[0] < 6.0)
    });
    assert!(
        between,
        "no Vanish between two whelp swings: {vanishes:?} {swings:?}"
    );
    // The whelp swings every two seconds, or two seconds after an Acid Spit ends. A swing the
    // pass forgot landed with the Rogue's next swing instead, 0.22 seconds late on this seed.
    // The log keeps hundredths of a second.
    for pair in swings.windows(2) {
        let (previous, swing) = (pair[0], pair[1]);
        if swing - previous > 6.0 {
            // The next summon.
            continue;
        }
        let spit = spits
            .iter()
            .rev()
            .find(|&&spit| spit >= previous - 0.005 && spit < swing);
        let due = match spit {
            Some(&spit) => spit + SPIT_SECONDS + SWING_SECONDS,
            None => previous + SWING_SECONDS,
        };
        assert!(
            (swing - due).abs() < 0.015,
            "the whelp swung at {swing}, due at {due}: {swings:?} {spits:?}"
        );
    }
}
