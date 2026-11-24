//! A precast lands on the pull under Forever's rules (25 November 2026).
//!
//! In game a caster times a precast so its damage lands as the fight starts, and a Warlock sends
//! the Imp in so its first Firebolt does too. Forever's rule held every early precast to finish
//! on 0:00 (SimulationCraft's precast), so a missile landed its travel time into the fight, and
//! pets only started casting at 0:00. Now a missile may leave up to its travel time before the
//! pull and lands on it, and a pet's first cast with a cast time begun on the pull is its
//! precast: it lands on 0:00 with its cast and GCD behind it. Damage before 0:00 still never
//! counts, and the Go reference's timing is unchanged.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const AFFLICTION: &str =
    "fixtures/mage/prepared-v2/production-affliction-warlock-wrack.request.json";
const IMP_WARLOCK: &str =
    "fixtures/mage/prepared-v2/affliction-warlock-dot-base-duration.request.json";

fn request(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn log(mut request: Value, rules: Rules) -> Vec<String> {
    request["encounter"]["duration"] = json!(30);
    request["encounter"]["durationVariation"] = json!(0);
    request["simOptions"]["iterations"] = json!(1);
    request["simOptions"]["debugFirstIteration"] = json!(true);
    request["simOptions"]["randomSeed"] = json!("7");
    let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"));
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["logs"]
        .as_str()
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn time(line: &str) -> f64 {
    line[1..line.find(']').unwrap()].parse().unwrap()
}

fn first(lines: &[String], needles: &[&str]) -> Option<f64> {
    lines
        .iter()
        .find(|line| needles.iter().all(|needle| line.contains(needle)))
        .map(|line| time(line))
}

/// A Shadow Bolt pressed well before the pull: under Forever's rule it leaves its travel time
/// early and lands on 0:00; the reference finishes it early and drops its damage.
#[test]
fn a_precast_missile_leaves_early_and_lands_on_the_pull() {
    let mut request = request(AFFLICTION);
    request["raid"]["parties"][0]["players"][0]["rotation"]["prepullActions"] = json!([{
        "action": {"castSpell": {"spellId": {"spellId": 25307}}},
        "doAtValue": {"const": {"val": "-6s"}},
    }]);
    let forever = log(request.clone(), Rules::Forever);
    let completed = first(&forever, &["] Completed cast {SpellID: 11661"]).unwrap();
    let landed = first(&forever, &["{SpellID: 11661}", " damage "]).unwrap();
    assert!(
        completed < 0.0,
        "the bolt leaves before the pull: {completed}"
    );
    assert_eq!(landed, 0.0, "and lands on it");

    let reference = log(request, Rules::GoReference);
    let landed = first(&reference, &["{SpellID: 25307}", " damage "]).unwrap();
    assert!(
        landed < 0.0,
        "the reference lands it before the pull: {landed}"
    );
}

/// The Imp's first Firebolt lands on the pull with its cast behind it, so the demon's loop
/// moves on to the next after its usual short wait; under the reference the first finishes one
/// cast time into the fight and every later cast comes that much later.
#[test]
fn the_imp_precasts_its_first_firebolt() {
    let imp_casts = |lines: &[String], event: &str| -> Vec<f64> {
        lines
            .iter()
            .filter(|line| line.contains(" - Imp] ") && line.contains(event))
            .map(|line| time(line))
            .collect()
    };
    let forever = log(request(IMP_WARLOCK), Rules::Forever);
    let reference = log(request(IMP_WARLOCK), Rules::GoReference);
    let (completed, started) = (
        imp_casts(&forever, "Completed cast"),
        imp_casts(&forever, "] Casting "),
    );
    let (completed_ref, started_ref) = (
        imp_casts(&reference, "Completed cast"),
        imp_casts(&reference, "] Casting "),
    );
    assert_eq!(completed[0], 0.0, "{completed:?}");
    assert!(
        first(&forever, &[" - Imp] ", " damage "]) == Some(0.0),
        "the first Firebolt lands on the pull"
    );
    assert_eq!(completed_ref[0], 2.0, "{completed_ref:?}");
    // The second cast starts one 2 s cast earlier than under the reference.
    assert!(
        (started_ref[1] - started[1] - 2.0).abs() < 1e-9,
        "{started:?} {started_ref:?}"
    );
}
