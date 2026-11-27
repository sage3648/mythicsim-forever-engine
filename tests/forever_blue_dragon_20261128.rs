//! Darkmoon Card: Blue Dragon (19288) under Forever's rules rolls the chance the live game states:
//! "Equip: 3% chance on successful spellcast to increase your Spirit by 150 for 15 sec. (Proc
//! chance: 3%)" (Wowhead's Forever tooltip of the item and of its equip spell, Aura of the Blue
//! Dragon 23688, 27 November 2026). The pinned client's row of 23688 rolls 2%, which the
//! reference's rules keep. 106 of 11,340 characters simmed in the five days to 27 November 2026
//! wore a Darkmoon card; 5 wore this one.
//!
//! The tests dress the Elemental Shaman of `fixtures/forever-gaps/elemental-shaman-the-elements`
//! in place of its second trinket.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const BLUE_DRAGON: i64 = 19288;
/// The fixture's second trinket (Burst of Knowledge 11832).
const TRINKET: usize = 13;

fn shaman(trinket: Value) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/forever-gaps/elemental-shaman-the-elements.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["consumables"] = json!({});
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.resize(17, json!({}));
    equipment[TRINKET] = trinket;
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// The chance of the card's stat proc effect.
fn card_chance(prepared: &Value) -> f64 {
    let chances: Vec<f64> = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| {
            effect["kind"] == "spell_data_stat_proc"
                && effect["trigger_aura"] == "Darkmoon Card: Blue Dragon"
        })
        .map(|effect| effect["proc_chance"].as_f64().unwrap())
        .collect();
    assert_eq!(chances.len(), 1, "{chances:?}");
    chances[0]
}

#[test]
fn forever_rolls_three_percent() {
    let request = shaman(json!({ "id": BLUE_DRAGON }));
    assert_eq!(card_chance(&prepared(&request, Rules::Forever)), 0.03);
}

#[test]
fn the_reference_keeps_the_pinned_two_percent() {
    let request = shaman(json!({ "id": BLUE_DRAGON }));
    assert_eq!(card_chance(&prepared(&request, Rules::GoReference)), 0.02);
}

/// The card prepares, passes the gate and runs: the buff is the reference's 150 Spirit for 15
/// sec, up some of the time.
#[test]
fn the_card_runs_with_its_spirit_buff() {
    let mut request = shaman(json!({ "id": BLUE_DRAGON }));
    request["simOptions"]["iterations"] = json!(200);
    let prepared: PreparedV2 = serde_json::from_value(prepared(&request, Rules::Forever)).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    let uptime = result["raidMetrics"]["parties"][0]["players"][0]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["id"]["spellId"] == 23684)
        .and_then(|aura| aura["uptimeSecondsAvg"].as_f64())
        .unwrap_or(0.0);
    assert!(uptime > 0.0, "the buff never procced");
}
