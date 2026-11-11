//! Expertise in an item swap set. A Droptimizer run of a level 37 Retribution Paladin on the
//! Manual Crowd Pummeler opener was refused on 9 November 2026 for the set that swapped onto
//! Servomechanic Sledgehammer (4548): `stat auras ["Item Swap: Swapped"] change ExpertisePercent,
//! which the runtime holds fixed`. The weapon carries 10 expertise rating, which takes 1% off the
//! target's dodge and parry while it is in the hand. Dwarven Tree Chopper (2907, 6 rating) was
//! refused the same way by the weapon swap survey.
//!
//! The request is the Retribution Paladin of `forever_item_gaps_20261109.rs` (fixtures/forever-gaps,
//! names replaced), whose opener swaps from the Pummeler to the set's weapon.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, prepared_refusals,
    simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const SERVOMECHANIC_SLEDGEHAMMER: i64 = 4548;
const DWARVEN_TREE_CHOPPER: i64 = 2907;
const MAIN_HAND: usize = 14;

fn base() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/forever-gaps/retribution-paladin-swap-sword-of-zeal.request.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// The opener swaps onto this weapon.
fn swapping_to(item: i64) -> Value {
    let mut request = base();
    player(&mut request)["itemSwap"]["items"][MAIN_HAND] = json!({ "id": item });
    request
}

fn prepare(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"))
}

fn gate_reasons(prepared: &Value) -> Vec<String> {
    let typed: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    prepared_refusals(&typed)
        .into_iter()
        .map(|refusal| refusal.reason)
        .collect()
}

fn run(prepared: &Value) -> Value {
    let typed: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    check_prepared(&typed).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&typed).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0].clone()
}

fn stat_auras(prepared: &Value) -> &Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "stat_auras")
        .expect("a stat auras effect")
}

/// The expertise percent of the stat aura combinations with the swap set off and on.
fn combo_expertise(prepared: &Value) -> (f64, f64) {
    let effect = stat_auras(prepared);
    let bit = effect["auras"]
        .as_array()
        .unwrap()
        .iter()
        .position(|aura| aura == "Item Swap: Swapped")
        .expect("the swap is a stat aura");
    assert!(
        effect.get("stacks").is_none(),
        "a combination is a mask of active auras"
    );
    let combos = effect["combos"].as_array().unwrap();
    let expertise = |mask: usize| combos[mask]["ExpertisePercent"].as_f64().unwrap();
    (expertise(0), expertise(1 << bit))
}

/// The same opener on the reference's talent tree, which its rules prepare: a Pummeler worn
/// with this weapon in the set.
fn reference_swapping_to(item: i64) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/forever-gaps/retribution-paladin-swap-without-crowd-pummeler.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let paladin = player(&mut request);
    paladin["buffs"] = json!({});
    paladin["talentsString"] = json!("52003-503-05225231001330321");
    paladin["equipment"]["items"][MAIN_HAND] = json!({ "id": 9449 });
    paladin["enableItemSwap"] = json!(true);
    paladin["itemSwap"] = json!({
        "items": (0..17)
            .map(|slot| if slot == MAIN_HAND { json!({ "id": item }) } else { json!({}) })
            .collect::<Vec<_>>()
    });
    request["raid"]["buffs"] = json!({});
    request["raid"]["parties"][0]["buffs"] = json!({});
    request
}

/// The set's weapon carries its expertise into the combinations, and the request passes the
/// gate, under both rules: the reference reads a unit's expertise live too.
#[test]
fn a_swap_onto_an_expertise_weapon_passes_the_gate() {
    for (item, expertise) in [
        (SERVOMECHANIC_SLEDGEHAMMER, 1.0),
        (DWARVEN_TREE_CHOPPER, 0.6),
    ] {
        for (request, rules) in [
            (swapping_to(item), Rules::Forever),
            (reference_swapping_to(item), Rules::GoReference),
        ] {
            let prepared = prepare(&request, rules);
            assert_eq!(gate_reasons(&prepared), Vec::<String>::new(), "item {item}");
            let (main, swapped) = combo_expertise(&prepared);
            assert!(
                (swapped - main - expertise).abs() < 1e-9,
                "item {item}: {main} then {swapped}"
            );
        }
    }
}

/// A swap whose weapons carry no expertise leaves it out of the combinations, as before.
#[test]
fn a_swap_without_expertise_carries_none() {
    let prepared = prepare(&base(), Rules::Forever);
    assert_eq!(gate_reasons(&prepared), Vec::<String>::new());
    let effect = stat_auras(&prepared);
    assert!(effect["combos"]
        .as_array()
        .unwrap()
        .iter()
        .all(|combo| combo.get("ExpertisePercent").is_none()));
}

/// The rotation without the Pummeler's use, and with the swap set put on at the pull: the set's
/// weapon is in the hand for the whole fight.
fn swapped_at_the_pull(item: i64) -> Value {
    let mut request = swapping_to(item);
    request["simOptions"]["iterations"] = json!(1000);
    let list = player(&mut request)["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    list.retain(|entry| entry["action"]["castSpell"]["spellId"]["itemId"] != 9449);
    let swap = list
        .iter_mut()
        .find(|entry| entry["action"].get("itemSwap").is_some())
        .unwrap();
    swap["action"].as_object_mut().unwrap().remove("condition");
    request
}

/// This weapon worn with no swap set, and the rotation without the Pummeler's use or the swap.
fn worn(item: i64) -> Value {
    let mut request = swapped_at_the_pull(item);
    let paladin = player(&mut request);
    paladin["enableItemSwap"] = json!(false);
    paladin["equipment"]["items"][MAIN_HAND] = json!({ "id": item });
    paladin["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .retain(|entry| entry["action"].get("itemSwap").is_none());
    request
}

/// The share of the main hand's white swings the target dodged.
fn white_dodge_rate(player: &Value) -> f64 {
    let swings = player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"] == json!({ "otherId": "OtherActionAttack", "tag": 1 }))
        .expect("the main hand's swings");
    let target = &swings["targets"][0];
    target["dodges"].as_f64().unwrap() / target["casts"].as_f64().unwrap()
}

/// The target's dodge reads the swapped weapon's expertise: a fight swapped onto Servomechanic
/// Sledgehammer at the pull is dodged as often as one that wears it, and about a point less than
/// one in the Pummeler, which has no expertise. Held at the Pummeler's, the swapped fight would
/// be dodged as often as the Pummeler's.
#[test]
fn the_targets_dodge_follows_the_swapped_weapons_expertise() {
    let rate = |request: &Value| {
        let prepared = prepare(request, Rules::Forever);
        assert_eq!(gate_reasons(&prepared), Vec::<String>::new());
        white_dodge_rate(&run(&prepared))
    };
    let swapped = rate(&swapped_at_the_pull(SERVOMECHANIC_SLEDGEHAMMER));
    let sledgehammer = rate(&worn(SERVOMECHANIC_SLEDGEHAMMER));
    let pummeler = rate(&worn(9449));
    assert!(
        (swapped - sledgehammer).abs() < 0.004,
        "swapped {swapped}, worn {sledgehammer}"
    );
    assert!(
        pummeler - swapped > 0.007,
        "swapped {swapped}, Pummeler {pummeler}"
    );
}
