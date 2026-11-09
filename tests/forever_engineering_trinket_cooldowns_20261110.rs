//! The Engineering trinkets of the Undermine set have the cooldowns their Wowhead Forever tooltips
//! state (10 November 2026), the client rows' 300000 and 360000 ms: Adaptive Combat Assistant
//! (272437) "(5 Min Cooldown)", Weakness Analyzer (272438) and Serenity Field (272439) "(6 Min
//! Cooldown)". The reference rows give each 90 seconds. Their shared category cooldown with the
//! other trinkets stays the row's (15 or 20 seconds).

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const SECOND_NS: i64 = 1_000_000_000;

/// The holy paladin fixture wearing only the given trinket.
fn prepared(item: i64, rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/holy-paladin-healing.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(1);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!("");
    player["equipment"] = json!({ "items": [{ "id": item }] });
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// The trinket's use spell's own cooldown and shared cooldown, in seconds.
fn cooldowns(item: i64, rules: Rules) -> (i64, i64) {
    let prepared = prepared(item, rules);
    let spell = prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["item_id"].as_i64() == Some(item))
        .unwrap_or_else(|| panic!("no use spell for {item}"));
    let seconds = |cooldown: &Value| cooldown["duration_ns"].as_i64().unwrap() / SECOND_NS;
    (seconds(&spell["cd"]), seconds(&spell["shared_cd"]))
}

/// Serenity Field's use spell runs on the tooltip's 6 minutes and keeps the 15 seconds it shares
/// with the other trinkets.
#[test]
fn serenity_field_takes_its_tooltip_cooldown() {
    assert_eq!(cooldowns(272439, Rules::Forever), (360, 15));
    assert_eq!(cooldowns(272439, Rules::GoReference), (90, 15));
}

fn live_row(item: i64) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/forever-live-items.jsonl");
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|row| row["id"] == item)
        .unwrap_or_else(|| panic!("a live row for {item}"))
}

/// The three live rows: the tooltip's cooldown, the row's category cooldown. Adaptive Combat
/// Assistant's shield and Weakness Analyzer's crit register no use spell yet, so their rows are
/// pinned as the file holds them. The Assistant's row also takes the tooltip's 1.1% expertise
/// (11 rating) in place of the reference row's 20.
#[test]
fn the_live_rows_carry_the_tooltip_cooldowns() {
    for (item, cooldown, category) in [
        (272437, 300_000, 15_000),
        (272438, 360_000, 20_000),
        (272439, 360_000, 15_000),
    ] {
        let row = live_row(item);
        let on_use = &row["item_effects"][0]["on_use"];
        assert_eq!(on_use["cooldown_ms"], cooldown, "{item}");
        assert_eq!(on_use["category_cooldown_ms"], category, "{item}");
    }
    assert_eq!(
        live_row(272437)["scaling_options"]["0"]["stats"],
        json!({ "24": 11 })
    );
}
