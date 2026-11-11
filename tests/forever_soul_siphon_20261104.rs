//! Soul Siphon under Forever's rules (4 November 2026, text of 11 November 2026).
//!
//! The talent (17804) reads, since the class tuning of 11 November 2026, "Increases the damage
//! done or health drained by your Drain Life, Drain Soul, and Wrack spells by 12% per each of
//! your Corruption, Siphon Life, and Bane of Agony effects active on the target, up to a maximum
//! increase of 36%." Before it the text named "your other Affliction effects", which also counted
//! the warlock's own curse. It is a dummy aura, so the server counts, and Forever's damage over
//! time reads the caster's current modifiers at each tick, so each Wrack and Drain Life tick
//! counts the named effects up at that moment. The Go reference counts every registered
//! Affliction aura on the target, active or not, which is the full 36% on every tick.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const WRACK: i64 = 1316697;
const DRAIN_LIFE: i64 = 11700;
const CORRUPTION: i64 = 25311;
const BANE_OF_AGONY: i64 = 11713;
const CURSE_OF_THE_ELEMENTS: i64 = 1311680;

/// The production Affliction board request (Vrelk's Wrack build on Troll) with `lines` as its
/// rotation and nothing before the pull, in a fixed two minute fight.
fn affliction(lines: Vec<Value>) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-affliction-warlock-wrack.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["encounter"]["durationVariation"] = json!(0);
    request["simOptions"]["iterations"] = json!(20);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    request["raid"]["parties"][0]["players"][0]["rotation"] = json!({
        "type": "TypeAPL",
        "prepullActions": [],
        "priorityList": lines.into_iter().map(|line| json!({"action": line})).collect::<Vec<_>>(),
    });
    request
}

fn cast(spell_id: i64) -> Value {
    json!({"castSpell": {"spellId": {"spellId": spell_id}}})
}

fn cast_when(spell_id: i64, condition: Value) -> Value {
    json!({"castSpell": {"spellId": {"spellId": spell_id}}, "condition": condition})
}

/// Under 7 seconds left, so no dot falls off during a six second Wrack channel.
fn dot_short(spell_id: i64) -> Value {
    json!({"cmp": {"op": "OpLt",
        "lhs": {"dotRemainingTime": {"spellId": {"spellId": spell_id}}},
        "rhs": {"const": {"val": "7s"}}}})
}

fn dot_long(spell_id: i64) -> Value {
    json!({"not": {"val": dot_short(spell_id)}})
}

fn curse_down() -> Value {
    json!({"not": {"val": {"auraIsActive": {
        "auraId": {"spellId": CURSE_OF_THE_ELEMENTS}, "sourceUnit": {"type": "CurrentTarget"}}}}})
}

/// The average damage of a Wrack tick that did not crit.
fn wrack_tick(request: &Value, rules: Rules) -> f64 {
    tick(request, rules, WRACK)
}

/// The average damage of a tick of `spell_id` that did not crit.
fn tick(request: &Value, rules: Rules, spell_id: i64) -> f64 {
    let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"));
    let prepared = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    let player = &report.result["raidMetrics"]["parties"][0]["players"][0];
    let action = player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"].as_i64() == Some(spell_id))
        .expect("the spell was cast");
    let range = &action["targets"][0]["tickRange"];
    range["total"].as_f64().unwrap() / range["count"].as_f64().unwrap()
}

fn assert_ratio(value: f64, base: f64, expected: f64, tolerance: f64) {
    let ratio = value / base;
    assert!(
        (ratio - expected).abs() < tolerance,
        "{value} / {base} = {ratio}, expected {expected}"
    );
}

fn wrack_alone() -> Value {
    affliction(vec![cast(WRACK)])
}

fn wrack_on_two_dots() -> Value {
    let both_up = json!({"and": {"vals": [dot_long(CORRUPTION), dot_long(BANE_OF_AGONY)]}});
    affliction(vec![
        cast_when(CORRUPTION, dot_short(CORRUPTION)),
        cast_when(BANE_OF_AGONY, dot_short(BANE_OF_AGONY)),
        cast_when(WRACK, both_up),
    ])
}

/// Wrack channelled only while Corruption and Bane of Agony outlast it: 24% for the two dots
/// over Wrack with nothing else on the target, which gets no bonus. The Go reference gives both
/// the same 36%.
#[test]
fn each_active_dot_adds_a_step() {
    let alone = wrack_tick(&wrack_alone(), Rules::Forever);
    let two = wrack_tick(&wrack_on_two_dots(), Rules::Forever);
    assert_ratio(two, alone, 1.24, 1e-9);
    let alone = wrack_tick(&wrack_alone(), Rules::GoReference);
    let two = wrack_tick(&wrack_on_two_dots(), Rules::GoReference);
    assert_ratio(two, alone, 1.0, 0.01);
}

/// The warlock's own Curse of the Elements is not one of the three named effects: with both
/// dots the bonus stays 24%, on top of the curse's 10% more shadow damage taken. Before
/// 11 November 2026 it counted as a third effect.
#[test]
fn own_curse_does_not_count() {
    let all_up = json!({"and": {"vals": [
        dot_long(CORRUPTION),
        dot_long(BANE_OF_AGONY),
        {"auraIsActive": {"auraId": {"spellId": CURSE_OF_THE_ELEMENTS},
            "sourceUnit": {"type": "CurrentTarget"}}},
    ]}});
    let request = affliction(vec![
        cast_when(CURSE_OF_THE_ELEMENTS, curse_down()),
        cast_when(CORRUPTION, dot_short(CORRUPTION)),
        cast_when(BANE_OF_AGONY, dot_short(BANE_OF_AGONY)),
        cast_when(WRACK, all_up),
    ]);
    let alone = wrack_tick(&wrack_alone(), Rules::Forever);
    let three = wrack_tick(&request, Rules::Forever);
    assert_ratio(three, alone, 1.24 * 1.1, 1e-9);
}

/// Drain Life reads Soul Siphon the same way, in its 20 yard range.
#[test]
fn drain_life_counts_the_same_way() {
    let in_range = |mut request: Value| {
        request["raid"]["parties"][0]["players"][0]["distanceFromTarget"] = json!(15);
        request
    };
    let both_up = json!({"and": {"vals": [dot_long(CORRUPTION), dot_long(BANE_OF_AGONY)]}});
    let on_dots = in_range(affliction(vec![
        cast_when(CORRUPTION, dot_short(CORRUPTION)),
        cast_when(BANE_OF_AGONY, dot_short(BANE_OF_AGONY)),
        cast_when(DRAIN_LIFE, both_up),
    ]));
    let alone = in_range(affliction(vec![cast(DRAIN_LIFE)]));
    let alone = tick(&alone, Rules::Forever, DRAIN_LIFE);
    let two = tick(&on_dots, Rules::Forever, DRAIN_LIFE);
    assert_ratio(two, alone, 1.24, 1e-9);
}
