//! Rotation conditions and casts that name a unit: what the gate accepts and refuses when a
//! `castSpell` names a target, and what the units in dot and aura values mean. Their Go result
//! and first-fight log goldens are compared with every other supported case in
//! `mage/prepared_v2.rs`; the variants compared with the pinned Go engine are in
//! validation/2026-10-06-rotation-target-fields.

use super::refusal_codes;
use serde_json::{json, Value};
use std::{fs, path::Path};

fn fixture(case: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The case with these items first in its rotation, in a fight of `targets` targets.
fn with_items(case: &str, targets: u32, items: Vec<Value>) -> Value {
    let mut value = fixture(case);
    // The exporter writes the count only for a fight against several targets.
    if targets > 1 {
        value["encounter"]["target_count"] = json!(targets);
    }
    let list = value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    list.splice(0..0, items);
    value
}

fn cast_at(spell: i32, target: Value) -> Value {
    json!({"action": {"castSpell": {"spellId": {"spellId": spell}, "target": target}}})
}

/// Why the gate refuses the input; empty when it supports it.
fn refused(value: Value) -> Vec<(&'static str, String)> {
    refusal_codes(value)
}

fn assert_supported(value: Value) {
    assert_eq!(refused(value), []);
}

const SECOND: fn() -> Value = || json!({"type": "Target", "index": 1});

#[test]
fn a_warlock_casts_a_curse_and_a_dot_at_other_targets() {
    let items = vec![
        cast_at(1311680, SECOND()),
        cast_at(25311, json!({"type": "NextTarget"})),
        cast_at(11713, json!({"type": "PreviousTarget"})),
    ];
    assert_supported(with_items("production-affliction-warlock", 3, items));
}

/// A target past the fight's is no unit, so Go drops the action and nothing reaches it.
#[test]
fn a_cast_at_a_target_the_fight_lacks_is_dropped() {
    let items = vec![cast_at(25311, json!({"type": "Target", "index": 4}))];
    assert_supported(with_items(
        "production-affliction-warlock",
        3,
        items.clone(),
    ));
    assert_supported(with_items(
        "production-marksmanship-hunter",
        1,
        vec![cast_at(25295, SECOND())],
    ));
}

#[test]
fn a_class_not_checked_against_casts_at_other_targets_is_refused() {
    let refusals = refusal_codes(with_items(
        "production-marksmanship-hunter",
        3,
        vec![cast_at(25295, SECOND())],
    ));
    assert!(
        refusals.contains(&(
            "several_targets_unsupported",
            "castSpell on a target past the first is not supported for ClassHunter yet".into()
        )),
        "{refusals:?}"
    );
}

#[test]
fn demonic_brand_stays_on_the_first_target() {
    let refusals = refusal_codes(with_items(
        "production-demonology-warlock",
        3,
        vec![cast_at(25307, SECOND())],
    ));
    assert!(
        refusals.contains(&(
            "several_targets_unsupported",
            "Demonic Brand lands on the first target only".into()
        )),
        "{refusals:?}"
    );
}

/// A channel with a dot on its target keeps the dot aura on the first target; Evocation's
/// dot is the mage's own.
#[test]
fn a_channel_with_a_target_dot_is_cast_at_the_first_target_only() {
    let refusals = refusal_codes(with_items(
        "production-fire",
        3,
        vec![cast_at(25345, SECOND())],
    ));
    assert!(
        refusals.iter().any(|(code, reason)| *code == "several_targets_unsupported"
            && reason.ends_with("castSpell of spell 25345 on target 2: a channel keeps its dot on the first target")),
        "{refusals:?}"
    );
    assert_supported(with_items(
        "production-fire",
        3,
        vec![cast_at(12051, SECOND()), cast_at(25306, SECOND())],
    ));
}

#[test]
fn a_cast_on_the_player_is_refused() {
    let refusals = refusal_codes(with_items(
        "production-affliction-warlock",
        1,
        vec![cast_at(11689, json!({"type": "Self"}))],
    ));
    assert!(
        refusals
            .iter()
            .any(|(code, reason)| *code == "several_targets_unsupported"
                && reason.ends_with("castSpell of spell 11689 on the player is unsupported")),
        "{refusals:?}"
    );
}

#[test]
fn units_outside_scope_are_refused_with_the_rotation_code() {
    for (item, reason) in [
        (
            cast_at(25311, json!({"type": "AllTargets"})),
            r#"rotation item 1: castSpell target {"type":"AllTargets"} is unsupported"#,
        ),
        (
            json!({"action": {"castSpell": {"spellId": {"spellId": 25307}},
                "condition": {"dotIsActive": {"spellId": {"spellId": 25311},
                    "targetUnit": {"type": "Pet", "owner": {"type": "Self"}}}}}}),
            r#"rotation item 1: dotIsActive targetUnit {"owner":{"type":"Self"},"type":"Pet"} is unsupported"#,
        ),
        (
            json!({"action": {"castSpell": {"spellId": {"spellId": 25307}},
                "condition": {"auraIsActive": {"auraId": {"spellId": 1311680},
                    "sourceUnit": {"type": "Player", "index": 2}}}}}),
            r#"rotation item 1: auraIsActive sourceUnit {"index":2,"type":"Player"} is unsupported"#,
        ),
    ] {
        let refusals = refusal_codes(with_items("production-affliction-warlock", 3, vec![item]));
        assert!(
            refusals.contains(&("rotation_unsupported", reason.into())),
            "{refusals:?}"
        );
    }
}

/// The three forms the shadow sims refused with no target involved.
#[test]
fn a_dot_without_a_spell_an_ordered_boolean_and_an_inactive_aura_are_supported() {
    let conditions = [
        json!({"dotIsActive": {}}),
        json!({"cmp": {"op": "OpGt", "lhs": {"dotIsActive": {"spellId": {"spellId": 25311}}},
            "rhs": {"auraIsActive": {"auraId": {"spellId": 1311680}}}}}),
        json!({"auraIsInactive": {"auraId": {"spellId": 1311680},
            "sourceUnit": {"type": "CurrentTarget"}}}),
        json!({"auraIsInactive": {"auraId": {"spellId": 17941}}}),
    ];
    for condition in conditions {
        let item = json!({"action": {"castSpell": {"spellId": {"spellId": 25307}},
            "condition": condition}});
        assert_supported(with_items("production-affliction-warlock", 1, vec![item]));
    }
}
