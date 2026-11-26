//! Enchant Weapon - Revelation under Forever's rules since Blizzard's hotfixes of 25 November
//! 2026: "The Revelation weapon enchant no longer rolls its proc chance for every target struck by
//! an area of effect spell. It now rolls once per wave." and "The Revelation enchant no longer
//! affects the entire duration of channeled spells." A spell's hits at one moment roll once, and
//! a channel's tick spends the buff where the channel's end used to. The Go reference keeps both.

use forever_engine::contracts::prepared_v2::PreparedV2;
use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The Mage's Revelation buff.
const REVELATION: i64 = 1248808;

/// The Arcane board's AoE request of 26 November 2026 (Arcane Explosion and Flamestrike on five
/// targets) with Revelation on its main hand, on `targets` of its targets.
fn prepared(targets: usize, rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/board-arcane-mage-aoe-revelation-20261126.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["encounter"]["targets"]
        .as_array_mut()
        .unwrap()
        .truncate(targets);
    request["simOptions"]["iterations"] = json!(300);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn revelation_effect(prepared: &mut Value) -> &mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "revelation")
        .expect("the Revelation effect")
}

/// The buff's gains a fight and its seconds up.
fn buff(prepared: &Value) -> (f64, f64) {
    let prepared: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    let result = forever_engine::simulate_prepared(&prepared).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["id"]["spellId"] == REVELATION)
        .map(|aura| {
            (
                aura["procsAvg"].as_f64().unwrap(),
                aura["uptimeSecondsAvg"].as_f64().unwrap(),
            )
        })
        .expect("the Revelation buff's row")
}

/// Forever's rules prepare `per_wave`; the reference does not.
#[test]
fn only_forevers_rules_roll_once_a_wave() {
    let mut forever = prepared(5, Rules::Forever);
    assert_eq!(revelation_effect(&mut forever)["per_wave"], true);
}

/// On five targets Arcane Explosion's wave rolls once under Forever's rules, where a roll a
/// target gains the buff far more often.
#[test]
fn an_area_wave_rolls_once() {
    let per_wave = prepared(5, Rules::Forever);
    let mut per_target = per_wave.clone();
    revelation_effect(&mut per_target)["per_wave"] = json!(false);
    let (wave_gains, _) = buff(&per_wave);
    let (target_gains, _) = buff(&per_target);
    assert!(wave_gains > 0.0);
    assert!(
        target_gains > 2.0 * wave_gains,
        "{target_gains} against {wave_gains}"
    );
}

/// On one target there is one hit a wave: rolling a wave or a target is the same fight but for
/// the channel rule, Arcane Missiles spending the buff on its first missile, so the buff is up
/// for less of the fight.
#[test]
fn a_channel_spends_the_buff_on_its_tick() {
    let per_wave = prepared(1, Rules::Forever);
    let mut old = per_wave.clone();
    revelation_effect(&mut old)["per_wave"] = json!(false);
    let (_, new_uptime) = buff(&per_wave);
    let (_, old_uptime) = buff(&old);
    assert!(new_uptime < old_uptime, "{new_uptime} against {old_uptime}");
}
