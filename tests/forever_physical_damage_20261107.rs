//! Physical damage measurements at levels 20 to 24 (in game, 7 November 2026), under Forever's
//! rules, which move every class that blocks. The Go reference keeps its own numbers, so every
//! check also reads the reference.
//!
//! A block takes off a whole number, the sheet's block value.

use forever_engine::{prepare_json_with, Rules};
use serde_json::Value;
use std::{fs, path::Path};

#[path = "support/warrior_trees_20261104.rs"]
mod warrior_trees_20261104;

fn recorded(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/mage/prepared-v2/production-{name}.request.json"
    ));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The production request with its talents in the client's order, which Forever's rules read.
fn production(name: &str) -> Value {
    let mut request = recorded(name);
    warrior_trees_20261104::forever_request(&mut request);
    request
}

/// The request as the rules read its talents: the recorded string is the reference's.
fn production_for(name: &str, rules: Rules) -> Value {
    match rules {
        Rules::Forever => production(name),
        Rules::GoReference => recorded(name),
    }
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "warrior", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

/// A block takes off the whole number below the block value, in both the target's swing at a
/// tank and Shield Slam's bonus; the reference takes off the fraction.
/// (`a_block_takes_off_a_whole_number` in `src/prepare/enemy/tests.rs` pins 7 for a 6 shield at
/// 59 Strength.)
#[test]
fn a_blocked_hit_loses_a_whole_number() {
    let reductions = |rules| -> Vec<f64> {
        let prepared = prepared(&production_for("protection-warrior", rules), rules);
        let enemy = &prepared["enemy"];
        ["rolls", "reduced_avoidance_rolls"]
            .into_iter()
            .flat_map(|key| enemy[key].as_array().unwrap().clone())
            .map(|rolls| number(&rolls["block_reduction"]))
            .collect()
    };
    let forever = reductions(Rules::Forever);
    assert!(!forever.is_empty());
    assert!(
        forever.iter().all(|reduction| reduction.fract() == 0.0),
        "{forever:?}"
    );
    assert!(
        forever.iter().any(|reduction| *reduction > 20.0),
        "{forever:?}"
    );
    let reference = reductions(Rules::GoReference);
    assert!(
        reference.iter().any(|reduction| reduction.fract() != 0.0),
        "{reference:?}"
    );
}
