//! The Warlock and Hunter class tuning live since 11 November 2026 (weekly reset), under
//! Forever's rules. Blizzard announced it on 6 November without numbers (the post "Class Tuning
//! Incoming" for 10 November: Affliction's dots should be worth casting beside Wrack); the
//! numbers are the live rows as Wowhead's Forever pages had them on the morning of 11 November.
//!
//! - Wrack (1316697): 60 Shadow damage a second at level 60 (25.2 at level 40, 1.75 a level;
//!   0.2002 of spell power a tick), was 86 with 0.286. Its 10% more damage from the warlock's
//!   other Shadow dots is gone. Instead, "While channeling, your Siphon Life, Corruption, and
//!   Bane of Agony effects on the target deal their periodic damage 100% faster."
//! - That line is a server-side script. The engine reads it as a dot under the channel ticking
//!   twice as often and lasting as long, so it deals twice the ticks while the channel is on
//!   it. It is provisional: the other reading, a dot that runs out in half the time with the
//!   ticks it had, costs Affliction about 14% on its best rotation, where Blizzard's post says
//!   the change comes "without affecting Affliction Warlocks' total damage output". Combat logs
//!   of a dot under the channel settle it.
//! - Summon Hawk and Arcane Shot cost 11% of base mana at every rank.
//! - The Imp's Firebolt ranks 1 to 4 deal more (`levels_demons_20261109.rs` holds the ranks).

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const WRACK: i64 = 1316697;
/// Corruption's top rank under Forever's rules, and the Go reference's.
const CORRUPTION: i64 = 11672;
const CORRUPTION_GO: i64 = 25311;

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2")
        .join(name);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The production Affliction board request with `lines` as its rotation and nothing before the
/// pull, in a fixed two minute fight.
fn affliction(lines: Vec<Value>) -> Value {
    let mut request = fixture("production-affliction-warlock-wrack.request.json");
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

fn cast_when_down(spell_id: i64) -> Value {
    json!({"castSpell": {"spellId": {"spellId": spell_id}},
        "condition": {"not": {"val": {"dotIsActive": {"spellId": {"spellId": spell_id}}}}}})
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

/// The casts of `spell_id` that landed and its ticks, an iteration, and the damage of a tick
/// that did not crit.
fn casts_ticks_and_tick(request: &Value, rules: Rules, spell_id: i64) -> (f64, f64, f64) {
    let prepared = serde_json::from_value(prepared(request, rules)).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    let iterations = request["simOptions"]["iterations"].as_f64().unwrap();
    let player = &report.result["raidMetrics"]["parties"][0]["players"][0];
    let action = player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"].as_i64() == Some(spell_id))
        .expect("the spell was cast");
    let target = &action["targets"][0];
    let range = &target["tickRange"];
    let ticks = target["ticks"].as_f64().unwrap() + target["critTicks"].as_f64().unwrap_or(0.0);
    (
        (target["casts"].as_f64().unwrap() - target["misses"].as_f64().unwrap_or(0.0)) / iterations,
        ticks / iterations,
        range["total"].as_f64().unwrap() / range["count"].as_f64().unwrap(),
    )
}

fn wrack_effect(prepared: &Value) -> Value {
    fn find(value: &Value) -> Option<Value> {
        match value {
            Value::Object(map) => {
                if map.get("kind").and_then(Value::as_str) == Some("wrack") {
                    return Some(value.clone());
                }
                map.values().find_map(find)
            }
            Value::Array(items) => items.iter().find_map(find),
            _ => None,
        }
    }
    find(prepared).expect("the build has Wrack")
}

/// The prepared Wrack: 60 a tick before spell power, no bonus on the dots' ticks, and the
/// three dots ticking twice as often. The Go reference keeps the frozen row and its 10%.
#[test]
fn wrack_prepares_the_live_row() {
    let request = affliction(vec![cast(WRACK)]);
    let forever = wrack_effect(&prepared(&request, Rules::Forever));
    assert_eq!(forever["tick_base"].as_f64(), Some(60.0));
    assert_eq!(forever["dot_bonus"].as_f64(), Some(1.0));
    assert_eq!(forever["dot_rate"].as_f64(), Some(2.0));
    let rated = forever["dot_spells"].as_array().unwrap().len();
    let go = wrack_effect(&prepared(&request, Rules::GoReference));
    assert_eq!(go["tick_base"].as_f64(), Some(36.0));
    assert!((go["dot_bonus"].as_f64().unwrap() - 1.1).abs() < 1e-9);
    assert!(go.get("dot_rate").is_none());
    // Siphon Life joins Corruption and Bane of Agony where the build has it.
    assert!(rated >= go["dot_spells"].as_array().unwrap().len());
}

/// Corruption under a Wrack channel ticks twice as often and lasts as long. Cast when it is
/// down with Wrack channelled otherwise, it is cast no more often than alone (less, as a
/// channel under way when it runs out is finished first), and a cast that lands deals close to
/// twelve ticks where it deals six alone, each as large as before.
#[test]
fn corruption_ticks_twice_as_often_under_the_channel() {
    let alone = affliction(vec![cast_when_down(CORRUPTION)]);
    let channelled = affliction(vec![cast_when_down(CORRUPTION), cast(WRACK)]);
    let (casts, ticks, tick) = casts_ticks_and_tick(&alone, Rules::Forever, CORRUPTION);
    let (fast_casts, fast_ticks, fast_tick) =
        casts_ticks_and_tick(&channelled, Rules::Forever, CORRUPTION);
    assert!((casts - 7.0).abs() < 0.5, "{casts} casts alone");
    assert!(fast_casts <= casts, "{fast_casts} casts against {casts}");
    // Alone each cast has its six ticks; only the last one of the fight loses some.
    let per_cast = ticks / casts;
    assert!(per_cast > 5.4 && per_cast <= 6.0, "{per_cast} ticks a cast");
    // Under the channel all but the cast's own 1.5 seconds tick at twice the rate.
    let per_cast = fast_ticks / fast_casts;
    assert!(
        per_cast > 10.0 && per_cast <= 12.0,
        "{per_cast} ticks a cast"
    );
    // The channel changes when a tick lands, not what it deals.
    assert!(
        (fast_tick / tick - 1.0).abs() < 1e-9,
        "{fast_tick} against {tick}"
    );
}

/// The Go reference has no faster ticks: with the channel a cast of Corruption still deals six.
#[test]
fn the_go_reference_keeps_corruption_at_six_ticks() {
    let alone = affliction(vec![cast_when_down(CORRUPTION_GO)]);
    let channelled = affliction(vec![cast_when_down(CORRUPTION_GO), cast(WRACK)]);
    let (casts, ticks, _) = casts_ticks_and_tick(&alone, Rules::GoReference, CORRUPTION_GO);
    let (fast_casts, fast_ticks, _) =
        casts_ticks_and_tick(&channelled, Rules::GoReference, CORRUPTION_GO);
    for (casts, ticks) in [(casts, ticks), (fast_casts, fast_ticks)] {
        let per_cast = ticks / casts;
        assert!(per_cast > 5.4 && per_cast <= 6.0, "{per_cast} ticks a cast");
    }
}
