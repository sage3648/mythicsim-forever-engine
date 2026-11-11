//! One process prepares characters of different levels (the test suite, a WebAssembly page,
//! any batch): the class code's tables kept for the process must not hold one character's ranks.
//! A shaman's damage rows were built from the highest Earth Shock, Flame Shock and Frost Shock
//! ranks the first character prepared knew, so a level 22 shaman prepared after a level 60 one
//! had no row for its Earth Shock rank 3 (8045) and its fight panicked on a spell with no
//! behavior (9 November 2026). The mage's and warlock's tables were built the same way.
//!
//! This file is its own test process, so the level 60 character is the first one prepared.

use forever_engine::contracts::prepared_v2::PreparedV2;
use forever_engine::{prepare_json_with, prepared_refusals, simulate_prepared, Rules};
use serde_json::Value;
use std::{fs, path::Path};

fn request(name: &str) -> Value {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/levels/{name}.request.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn simulate(request: &Value) -> Value {
    let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("prepares: {err}"));
    let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
    assert_eq!(prepared_refusals(&typed), vec![]);
    simulate_prepared(&typed).unwrap().result
}

/// The same build at level 60 (no level, sheet or spellbook) and at its own level 22, then 30.
#[test]
fn characters_of_other_levels_share_a_process() {
    for name in [
        "enhancement-shaman-L22-flametongue",
        "frost-mage-L20",
        "affliction-warlock-L30",
    ] {
        let levelled = request(name);
        let mut level_60 = levelled.clone();
        let player = level_60["raid"]["parties"][0]["players"][0]
            .as_object_mut()
            .unwrap();
        for field in ["level", "characterSheet", "knownSpells"] {
            player.remove(field);
        }
        for target in level_60["encounter"]["targets"].as_array_mut().unwrap() {
            target["level"] = 63.into();
        }
        level_60["simOptions"]["iterations"] = 5.into();
        let mut levelled = levelled;
        levelled["simOptions"]["iterations"] = 5.into();
        for request in [&level_60, &levelled] {
            let result = simulate(request);
            let dps = result["raidMetrics"]["parties"][0]["players"][0]["dps"]["avg"]
                .as_f64()
                .unwrap_or(0.0);
            assert!(dps > 0.0, "{name}: {dps}");
        }
    }
}
