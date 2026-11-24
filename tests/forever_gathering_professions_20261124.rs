//! Skinning, Mining and Herbalism carry no combat passive under Forever's rules (24 November 2026).
//!
//! The client (wago.tools `product=wowf`, 1.60.1.71037) gives the gathering skill lines their
//! rank spells, gathering, smelting, tracking and camp features, and no damage or health aura.
//! The engine had given Skinning 5% damage to Beasts and Dragonkin and Mining 5% health, read off
//! a pre-beta demo slide; those stay under the Go reference rules only.
//!
//! An Arms Warrior Quick Sim request built by the application (Engineering,
//! `fixtures/forever-gaps/arms-warrior-revenge`, its boss made a Beast or Dragonkin) is prepared
//! with each gathering profession as the second profession and without one. Under Forever's rules
//! the prepared fight is the same apart from the profession list and the request hash. The Go
//! reference rules still apply both passives, checked on the Fury Warrior golden request.

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/warrior_trees_20261104.rs"]
mod warrior_trees_20261104;

fn warrior(file: &str, profession: Option<&str>, mob_type: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["encounter"]["targets"][0]["mobType"] = json!(mob_type);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    assert_eq!(player["profession1"], "Engineering");
    if let Some(profession) = profession {
        player["profession2"] = json!(profession);
    }
    request
}

fn arms(profession: Option<&str>, mob_type: &str) -> Value {
    let mut request = warrior(
        "fixtures/forever-gaps/arms-warrior-revenge.request.json",
        profession,
        mob_type,
    );
    // The recorded talents in the order Forever's rules read (as `tests/forever_gaps.rs` does).
    warrior_trees_20261104::forever_request(&mut request);
    request
}

fn fury(profession: Option<&str>, mob_type: &str) -> Value {
    warrior(
        "tests/classes/warrior/prepare/fury.request.json",
        profession,
        mob_type,
    )
}

fn prepare(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// The prepared input without what names the request: its hash and the profession list.
fn fight(mut prepared: Value) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(map) => {
                map.remove("request_sha256");
                map.remove("professions");
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut prepared);
    prepared
}

#[test]
fn gathering_professions_change_nothing_under_forever_rules() {
    for mob_type in ["MobTypeBeast", "MobTypeDragonkin"] {
        let without = fight(prepare(&arms(None, mob_type), Rules::Forever));
        for profession in ["Skinning", "Mining", "Herbalism"] {
            let with = fight(prepare(&arms(Some(profession), mob_type), Rules::Forever));
            assert!(
                with == without,
                "{profession} against {mob_type} changed the prepared fight"
            );
        }
    }
}

#[test]
fn the_go_reference_rules_keep_skinning_and_mining() {
    let without = fight(prepare(&fury(None, "MobTypeBeast"), Rules::GoReference));
    for profession in ["Skinning", "Mining"] {
        let with = fight(prepare(
            &fury(Some(profession), "MobTypeBeast"),
            Rules::GoReference,
        ));
        assert!(
            with != without,
            "{profession} should still apply under the Go reference rules"
        );
    }
}
