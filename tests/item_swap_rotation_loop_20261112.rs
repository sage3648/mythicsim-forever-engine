//! A rotation whose item swap lines both hold at once. Five Quick Sims of custom Rogue rotations
//! failed on 10 November 2026 with `thread 'main' panicked at src/core/fight/rotation.rs:
//! infinite rotation loop`. Their inputs are private and were not read; the same player's public
//! rotations swap to Main on one condition and to Swap1 on another, and that shape reproduces
//! the panic once both conditions hold together. A swap takes no time of its own (it only
//! extends the GCD) and its readiness was "the other set is worn", so the rotation swapped back
//! and forth in one timestep until the loop guard panicked.
//!
//! The rotation now swaps once a timestep, under either rules, as it changes target once a
//! timestep: the second swap waits for the rotation's next look. Rotations that never ask for
//! two swaps at one moment are untouched.
//!
//! The requests are a Warrior's (a class whose rotation looks are cached) and a Retribution
//! Paladin's (looks not cached) with their swap lines replaced.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, prepared_refusals, simulate_prepared,
    Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn request(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The request with its own swap lines taken out and these put first.
fn with_swap_lines(mut request: Value, lines: &[Value]) -> Value {
    request["simOptions"]["iterations"] = json!(40);
    request["simOptions"]["debugFirstIteration"] = json!(true);
    let list = request["raid"]["parties"][0]["players"][0]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    list.retain(|entry| entry["action"].get("itemSwap").is_none());
    for (position, line) in lines.iter().enumerate() {
        list.insert(position, line.clone());
    }
    request
}

fn swap_to(set: &str) -> Value {
    json!({ "action": { "itemSwap": { "swapSet": set } } })
}

/// The first fight's log and the player's metrics.
fn run(request: &Value, rules: Rules) -> (String, Value) {
    let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"));
    let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
    assert_eq!(
        prepared_refusals(&typed)
            .into_iter()
            .map(|refusal| refusal.reason)
            .collect::<Vec<_>>(),
        Vec::<String>::new()
    );
    let result = simulate_prepared(&typed).unwrap().result;
    (
        result["logs"].as_str().unwrap().to_string(),
        result["raidMetrics"]["parties"][0]["players"][0].clone(),
    )
}

/// The timestamp and set of each swap the rotation made in the fight, in order.
fn swaps(logs: &str) -> Vec<(String, String)> {
    logs.lines()
        .filter_map(|line| {
            let (before, set) = line.split_once("Item Swap to set ")?;
            let stamp = before.split(']').next().unwrap().trim_start_matches('[');
            Some((stamp.to_string(), set.trim().to_string()))
        })
        .collect()
}

fn cases() -> Vec<(&'static str, Value, Rules)> {
    vec![
        (
            "warrior",
            request("fixtures/mage/prepared-v2/item-swap-dual-wield.request.json"),
            Rules::GoReference,
        ),
        (
            "paladin",
            request("fixtures/forever-gaps/retribution-paladin-swap-sword-of-zeal.request.json"),
            Rules::Forever,
        ),
    ]
}

/// Lines to Swap1 and to Main with no condition: the fight runs to its end, the rotation swaps
/// once a timestep and the sets alternate. Each swap restarts the swing and the GCD, so such a
/// rotation may land nothing; the fight only has to run.
#[test]
fn swap_lines_that_both_hold_swap_once_a_timestep() {
    for (name, base, rules) in cases() {
        for order in [["Swap1", "Main"], ["Main", "Swap1"]] {
            let request = with_swap_lines(base.clone(), &[swap_to(order[0]), swap_to(order[1])]);
            let (logs, _) = run(&request, rules);
            let swaps = swaps(&logs);
            assert!(swaps.len() > 10, "{name} {order:?}: {} swaps", swaps.len());
            for pair in swaps.windows(2) {
                assert_ne!(
                    pair[0].0, pair[1].0,
                    "{name} {order:?}: two swaps at one time"
                );
                assert_ne!(pair[0].1, pair[1].1, "{name} {order:?}: the sets alternate");
            }
            assert_eq!(
                swaps[0].1, "Swap1",
                "{name} {order:?}: Main is worn at the pull"
            );
        }
    }
}

/// The shape of the failed sims: a swap to Main on one condition and a swap to Swap1 on another
/// that can hold together, here the first seconds after a swing.
#[test]
fn swap_lines_whose_conditions_overlap_run_the_fight() {
    let after_a_swing = |window: &str| {
        json!({ "cmp": {
            "lhs": { "autoTimeSinceLast": {} },
            "op": "OpLt",
            "rhs": { "const": { "val": window } }
        } })
    };
    for (name, base, rules) in cases() {
        let mut to_main = swap_to("Main");
        to_main["action"]["condition"] = after_a_swing("1s");
        let mut to_swap = swap_to("Swap1");
        to_swap["action"]["condition"] = after_a_swing("0.5s");
        let (logs, player) = run(&with_swap_lines(base, &[to_main, to_swap]), rules);
        let swaps = swaps(&logs);
        assert!(swaps.len() > 2, "{name}: {} swaps", swaps.len());
        for pair in swaps.windows(2) {
            assert_ne!(pair[0].0, pair[1].0, "{name}: two swaps at one time");
        }
        assert!(player["dps"]["avg"].as_f64().unwrap() > 0.0, "{name}");
    }
}
