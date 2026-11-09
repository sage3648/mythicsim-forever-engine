//! Bloodrage's threat under Forever's rules, from Forever combat logs: its instant 10 rage makes
//! 50 threat in Battle and Defensive Stance alike while its ticks take the stance, and rage
//! gained before the pull, as a prepull Bloodrage's, makes none.

use forever_engine::{contracts::prepared_v2::PreparedV2, prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const BATTLE_STANCE: i64 = 2457;
const BLOODRAGE: i64 = 2687;

/// The production Protection request with no talents, a twelve second fight against a target
/// that deals no damage, so the rage bar never fills, casting only Battle Stance and Bloodrage
/// (at `prepull_at` before the pull when it is given).
fn bloodrage_only(prepull_at: Option<&str>) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-protection-warrior.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["encounter"]["duration"] = json!(12);
    request["encounter"]["durationVariation"] = json!(0);
    request["encounter"]["targets"][0]["minBaseDamage"] = json!(0);
    request["simOptions"]["iterations"] = json!(20);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!("");
    let cast = |id: i64| json!({"castSpell": {"spellId": {"spellId": id}}});
    let mut prepull =
        vec![json!({"action": cast(BATTLE_STANCE), "doAtValue": {"const": {"val": "-20s"}}})];
    let mut list = Vec::new();
    match prepull_at {
        Some(at) => {
            prepull.push(json!({"action": cast(BLOODRAGE), "doAtValue": {"const": {"val": at}}}))
        }
        None => list.push(json!({"action": cast(BLOODRAGE)})),
    }
    player["rotation"] =
        json!({"type": "TypeAPL", "prepullActions": prepull, "priorityList": list});
    request
}

/// The rage Bloodrage gained and actually gained, and the threat the rage gain action booked,
/// over the run.
fn rage_and_threat(rules: Rules, prepull_at: Option<&str>) -> (f64, f64, f64) {
    let request = bloodrage_only(prepull_at);
    let prepared = prepare_json_with(request.to_string().as_bytes(), "warrior", rules)
        .unwrap_or_else(|err| panic!("{err:?}"));
    let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let result = forever_engine::simulate_prepared(&typed).unwrap().result;
    let player = &result["raidMetrics"]["parties"][0]["players"][0];
    let row = |key: &str| -> f64 {
        player["resources"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["id"]["spellId"].as_i64() == Some(BLOODRAGE))
            .map(|row| row[key].as_f64().unwrap_or(0.0))
            .sum()
    };
    let threat = player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["otherId"] == "OtherActionRageGain")
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["threat"].as_f64().unwrap_or(0.0))
        .sum();
    (row("gain"), row("actualGain"), threat)
}

/// One Bloodrage at the pull in Battle Stance: under Forever's rules the instant 10 rage makes
/// 50 threat and only the ticks take the stance's multiplier, which the reference applies to
/// all of it. The instant rage comes at the pull, with the bar near empty, so all of it is
/// gained.
#[test]
fn bloodrages_instant_rage_makes_unscaled_threat() {
    let iterations = 20.0;
    let (_, actual, reference) = rage_and_threat(Rules::GoReference, None);
    // The reference: 5 threat a rage times the stance, on every point.
    let multiplier = reference / (5.0 * actual);
    assert!((multiplier - 1.0).abs() > 0.1, "{multiplier}");
    let (gain, actual, forever) = rage_and_threat(Rules::Forever, None);
    assert!(gain == 20.0 * iterations, "{gain} rage");
    let instant = 10.0 * iterations;
    let want = 5.0 * instant + 5.0 * multiplier * (actual - instant);
    assert!((forever - want).abs() < 1e-6, "{forever} against {want}");
}

/// Bloodrage cast fifteen seconds before the pull gains all its rage before it: no threat
/// under Forever's rules, where the reference books it all.
#[test]
fn rage_before_the_pull_makes_no_threat() {
    let (gain, _, forever) = rage_and_threat(Rules::Forever, Some("-15s"));
    assert!(gain > 10.0, "{gain}");
    assert_eq!(forever, 0.0);
    let (_, _, reference) = rage_and_threat(Rules::GoReference, Some("-15s"));
    assert!(reference > 0.0);
}
