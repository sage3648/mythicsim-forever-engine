//! Three refusals and failures from prod on 24 November 2026.
//!
//! - A strict sequence's wait step (Go `APLActionWait` inside `APLActionStrictSequence`) was
//!   refused ("strictSequence action wait is unsupported"), failing a Warrior's custom rotation.
//!   Go runs it as a step: the sequence moves past it at once and the wait takes control on top
//!   of the sequence until its time has passed.
//! - A rotation casting a passive proc (Seal of Righteousness's hit, 25736) ran 1,000 casts in a
//!   timestep and panicked ("infinite rotation loop"), failing a Paladin's whole Droptimizer.
//!   The gate now refuses it by name.
//! - A Hunter can train Thrown, so the worker now accepts a thrown weapon in a Hunter's ranged
//!   slot. Under Forever's rules nothing that needs a bow, gun or crossbow is pressed with it:
//!   not Auto Shot, the shots, the stings or Volley.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, prepared_refusals, simulate_prepared,
    Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const AFFLICTION: &str =
    "fixtures/mage/prepared-v2/production-affliction-warlock-wrack.request.json";
const RETRIBUTION: &str =
    "fixtures/paladin/retribution-paladin-unsealed-wrath-command-echoes.request.json";
const MARKSMANSHIP: &str = "fixtures/mage/prepared-v2/production-marksmanship-hunter.request.json";
const SHADOW_BOLT: i64 = 25307;
/// Seal of Righteousness's hit, a passive proc the rotation cannot press.
const SEAL_OF_RIGHTEOUSNESS_HIT: i64 = 25736;
/// Gutcutter Hatchet, a thrown weapon a level 48 Survival Hunter wore on 24 November 2026.
const GUTCUTTER_HATCHET: i64 = 274643;

fn request(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

fn prepared(request: &Value, rules: Rules) -> PreparedV2 {
    let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"));
    serde_json::from_value(prepared).unwrap()
}

fn one_fight(mut request: Value, seconds: u32) -> Value {
    request["encounter"]["duration"] = json!(seconds);
    request["encounter"]["durationVariation"] = json!(0);
    request["simOptions"]["iterations"] = json!(1);
    request["simOptions"]["debugFirstIteration"] = json!(true);
    request["simOptions"]["randomSeed"] = json!("7");
    request
}

fn bolt() -> Value {
    json!({"castSpell": {"spellId": {"spellId": SHADOW_BOLT}}})
}

/// The first iteration's log of a 30 s fight whose whole rotation is one strict sequence.
fn strict_log(actions: Value, rules: Rules) -> String {
    let mut request = request(AFFLICTION);
    player(&mut request)["rotation"] = json!({
        "type": "TypeAPL",
        "priorityList": [{"action": {"strictSequence": {"actions": actions}}}],
    });
    let request = one_fight(request, 30);
    let report = simulate_prepared(&prepared(&request, rules)).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

fn time(line: &str) -> f64 {
    line[1..line.find(']').unwrap()].parse().unwrap()
}

/// When the player starts casting Shadow Bolt (11661 under Forever's rules, 25307 under the Go
/// reference's).
fn bolt_starts(log: &str) -> Vec<f64> {
    log.lines()
        .filter(|line| {
            line.contains("] Casting {SpellID: 11661}")
                || line.contains("] Casting {SpellID: 25307}")
        })
        .map(time)
        .collect()
}

#[test]
fn a_strict_sequence_wait_step_holds_the_sequence_for_its_duration() {
    for rules in [Rules::Forever, Rules::GoReference] {
        let plain = bolt_starts(&strict_log(json!([bolt(), bolt()]), rules));
        let waited = bolt_starts(&strict_log(
            json!([bolt(), {"wait": {"duration": {"const": {"val": "5s"}}}}, bolt()]),
            rules,
        ));
        assert!(
            plain.len() >= 2 && waited.len() >= 2,
            "{plain:?} {waited:?}"
        );
        assert_eq!(plain[0], waited[0], "the first bolt is the same");
        let cast = plain[1] - plain[0];
        let gap = waited[1] - waited[0];
        // Go's strict sequence moves past a ready wait at once and the wait runs from the first
        // bolt's start, so the second bolt starts 5 s after it.
        assert!(
            cast < 4.0,
            "without the wait the bolts follow each other: {cast}"
        );
        assert!(
            (gap - 5.0).abs() < 0.01,
            "the wait holds the sequence 5 s: gap {gap}"
        );
    }
}

#[test]
fn a_strict_sequence_ending_in_a_wait_runs_again_after_it() {
    let waited = bolt_starts(&strict_log(
        json!([bolt(), {"wait": {"duration": {"const": {"val": "6s"}}}}]),
        Rules::Forever,
    ));
    assert!(waited.len() >= 3, "{waited:?}");
    for pair in waited.windows(2) {
        assert!((pair[1] - pair[0] - 6.0).abs() < 0.01, "{waited:?}");
    }
}

#[test]
fn a_strict_sequence_with_a_wait_without_a_duration_is_no_action() {
    // Go's newActionWait is nil without a value, and a strict sequence with a nil step is nil.
    let starts = bolt_starts(&strict_log(
        json!([bolt(), {"wait": {}}, bolt()]),
        Rules::Forever,
    ));
    assert!(starts.is_empty(), "{starts:?}");
}

#[test]
fn a_cast_of_a_passive_proc_is_refused_by_name() {
    let mut request = request(RETRIBUTION);
    let list = player(&mut request)["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    list.insert(
        0,
        json!({"action": {"castSpell": {"spellId": {"spellId": SEAL_OF_RIGHTEOUSNESS_HIT}}}}),
    );
    let refusals = prepared_refusals(&prepared(&request, Rules::Forever));
    assert!(
        refusals
            .iter()
            .any(|refusal| refusal.code == "rotation_unsupported"
                && refusal.reason.contains("spell 25736 is a passive effect")),
        "{:?}",
        refusals
            .iter()
            .map(|refusal| &refusal.reason)
            .collect::<Vec<_>>()
    );
}

fn with_thrown(mut request: Value) -> Value {
    let items = player(&mut request)["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    // The ranged slot is the last of the 17.
    while items.len() < 17 {
        items.push(json!({}));
    }
    items[16] = json!({"id": GUTCUTTER_HATCHET});
    request
}

fn has_flag(prepared: &PreparedV2, class_spell: &str, flag: &str) -> bool {
    prepared
        .player
        .spells
        .iter()
        .filter(|spell| spell.class_spell.as_deref() == Some(class_spell))
        .all(|spell| spell.has_flag(flag))
}

#[test]
fn a_hunter_with_a_thrown_weapon_presses_nothing_that_needs_a_bow() {
    let thrown = with_thrown(request(MARKSMANSHIP));
    let forever = prepared(&thrown, Rules::Forever);
    assert!(
        !forever.melee.auto_swing_ranged,
        "no Auto Shot from a thrown weapon"
    );
    for shot in ["aimed_shot", "arcane_shot", "multi_shot", "serpent_sting"] {
        assert!(has_flag(&forever, shot, "SpellFlagUnlearned"), "{shot}");
        assert!(!has_flag(&forever, shot, "SpellFlagAPL"), "{shot}");
    }
    // The fight runs on the hunter's melee and traps alone, and the weapon's stats still count.
    let report = simulate_prepared(&forever).unwrap();
    let actions = report.result["raidMetrics"]["parties"][0]["players"][0]["actions"]
        .as_array()
        .unwrap();
    let shots: f64 = actions
        .iter()
        .filter(|action| {
            action["id"]["otherId"] == "OtherActionShoot"
                || [20900, 25295, 25294, 14283, 2643, 13551]
                    .iter()
                    .any(|id| action["id"]["spellId"] == *id)
        })
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["damage"].as_f64().unwrap_or(0.0))
        .sum();
    assert_eq!(shots, 0.0);
    // A bow keeps every shot.
    let bow = prepared(&request(MARKSMANSHIP), Rules::Forever);
    assert!(bow.melee.auto_swing_ranged);
    assert!(has_flag(&bow, "aimed_shot", "SpellFlagAPL"));
}
