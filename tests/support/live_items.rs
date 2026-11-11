//! Items with a row in `data/forever-live-items.jsonl` carry other stats under Forever's rules
//! than under the reference's. Tests that sim one request under both rules and compare the rolls
//! take those items off first, so both rule sets wear the same stats.

#![allow(dead_code)]

use serde_json::{json, Value};
use std::{fs, path::Path};

/// The ids of the live item rows.
pub fn live_item_ids() -> Vec<i64> {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/forever-live-items.jsonl"))
        .unwrap()
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line).unwrap()["id"]
                .as_i64()
                .unwrap()
        })
        .collect()
}

/// Empties every equipment slot of `player` that holds an item with a live row.
pub fn take_off_live_items(player: &mut Value) {
    let live = live_item_ids();
    if let Some(items) = player["equipment"]["items"].as_array_mut() {
        for item in items {
            if item["id"].as_i64().is_some_and(|id| live.contains(&id)) {
                *item = json!({});
            }
        }
    }
}
