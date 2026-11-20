//! Forever sims refused on the evening of 19 November 2026 (UTC):
//!
//! - "an item swap changes /player/auras/6/displaced_by, ...; /player/auras/20/active;
//!   /player/auras/20/blocked_at_reset, which the runtime does not follow", two Best Gear runs of
//!   a Human Engineer Retribution Paladin (Shockadin talents) at 22:15 and 22:17. The default
//!   Manual Crowd Pummeler opener's swap set held Runeblade of Baron Rivendare (13505), whose
//!   Unholy Aura (8% movement speed) bids in the passive movement speed category at the priority
//!   of the talent Pursuit of Justice (8%). At the swap the Runeblade's aura takes the category
//!   over, as the runtime's tie does (the newcomer wins a tie that the holder does not outlast),
//!   and the speed stays 1.08. The coverage check refused any exclusive state change of an aura
//!   the swap does not switch.
//! - "rotation item 6: move has no range; prepull actions outside the rotation are unsupported",
//!   a Hunter Quick Sim whose custom rotation held a move with no range. The rotation failed to
//!   parse, and the coverage check, given no rotation, also called its prepull actions
//!   unsupported.
//!
//! The Paladin's request is the refused set rebuilt from the player's export and the sim's
//! settings with the application's own request builder (100 iterations).

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, prepared_coverage,
    simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const RUNEBLADE_OF_BARON_RIVENDARE: i64 = 13505;
const MAIN_HAND: usize = 14;

const PALADIN: &str =
    "fixtures/forever-gaps/retribution-paladin-swap-runeblade-of-baron-rivendare.request.json";
const HUNTER: &str = "fixtures/forever-gaps/hunter-skysight.request.json";

fn request(fixture: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn prepared(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

fn coverage(value: &Value) -> Vec<String> {
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    prepared_coverage(&prepared)
}

fn aura_index(prepared: &Value, label: &str) -> usize {
    prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .position(|aura| aura["label"] == label)
        .unwrap_or_else(|| panic!("no aura {label}"))
}

/// The item swap effect.
fn item_swap(prepared: &mut Value) -> &mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "item_swap")
        .expect("an item swap")
}

fn movement_index(prepared: &Value) -> usize {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .position(|effect| effect["kind"] == "player_movement")
        .expect("a player movement effect")
}

fn swapped_paths(prepared: &mut Value) -> Vec<String> {
    item_swap(prepared)["swapped"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| change["path"].as_str().unwrap().to_string())
        .collect()
}

/// The refused set: Unholy Aura takes the movement speed category over from Pursuit of Justice
/// at the swap, at the same priority and the same speed. It passes the gate and runs.
#[test]
fn a_swapped_speed_aura_tying_pursuit_of_justice_passes_the_gate() {
    let request = request(PALADIN);
    let player = &request["raid"]["parties"][0]["players"][0];
    assert_eq!(
        player["itemSwap"]["items"][MAIN_HAND]["id"],
        RUNEBLADE_OF_BARON_RIVENDARE
    );
    let mut value = prepared(&request);
    let unholy = aura_index(&value, "Unholy Aura");
    let pursuit = aura_index(&value, "Pursuit of Justice");
    for label in [unholy, pursuit] {
        let bids = &value["player"]["auras"][label]["exclusive_memberships"];
        assert_eq!(bids[0]["category"], "PassiveMovementSpeed");
        assert_eq!(bids[0]["priority"], 0.08);
    }
    let movement = movement_index(&value);
    assert_eq!(value["effects"][movement]["speed_multiplier"], 1.08);
    let paths = swapped_paths(&mut value);
    for path in [
        format!("/player/auras/{unholy}/displaced_by"),
        format!("/player/auras/{pursuit}/active"),
        format!("/player/auras/{pursuit}/blocked_at_reset"),
    ] {
        assert!(paths.contains(&path), "{path} in {paths:?}");
    }
    assert!(!paths.contains(&format!("/effects/{movement}/speed_multiplier")));
    assert_eq!(coverage(&value), Vec::<String>::new());
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    let player = &result["raidMetrics"]["parties"][0]["players"][0];
    assert!(player["dps"]["avg"].as_f64().unwrap() > 0.0);
}

/// The tie is accepted only as stated: Pursuit of Justice bidding at another priority, or a swap
/// that moves the speed, is still refused.
#[test]
fn the_tie_is_accepted_only_at_one_priority_and_one_speed() {
    let base = prepared(&request(PALADIN));
    let pursuit = aura_index(&base, "Pursuit of Justice");
    let refused = |reasons: Vec<String>| {
        assert!(
            reasons
                .iter()
                .any(|reason| reason
                    .contains(&format!("/player/auras/{pursuit}/blocked_at_reset"))),
            "{reasons:?}"
        );
    };

    let mut other_priority = base.clone();
    other_priority["player"]["auras"][pursuit]["exclusive_memberships"][0]["priority"] =
        json!(0.07);
    refused(coverage(&other_priority));

    let mut faster = base.clone();
    let movement = movement_index(&faster);
    item_swap(&mut faster)["swapped"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "path": format!("/effects/{movement}/speed_multiplier"), "value": 1.16 }));
    refused(coverage(&faster));

    // A member of the category outside the movement effect's speed auras.
    let mut outside = base.clone();
    outside["effects"][movement]["speed_auras"] = json!(["Unholy Aura"]);
    refused(coverage(&outside));
}

/// A rotation that fails to parse is refused for its own reason alone: its prepull actions are
/// not called unsupported.
#[test]
fn a_rotation_that_fails_to_parse_does_not_refuse_its_prepull_actions() {
    let mut request = request(HUNTER);
    let rotation = &mut request["raid"]["parties"][0]["players"][0]["rotation"];
    assert!(!rotation["prepullActions"].as_array().unwrap().is_empty());
    let list = rotation["priorityList"].as_array_mut().unwrap();
    let at = list.len();
    list.push(json!({ "action": { "move": {} } }));
    let reasons = coverage(&prepared(&request));
    assert!(
        reasons.contains(&format!("rotation item {}: move has no range", at + 1)),
        "{reasons:?}"
    );
    assert!(
        reasons
            .iter()
            .all(|reason| !reason.contains("prepull actions outside the rotation")),
        "{reasons:?}"
    );
}
