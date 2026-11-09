//! Dragon's Call (10847) in an item swap set, under Forever's rules. Its proc summons an Emerald
//! Dragon Whelp, a gear pet that used to exist only for a weapon worn at the pull, so a swap onto
//! the weapon was refused ("an item swap with the effect of item 10847 is not prepared yet").
//!
//! Client rows and Wowhead's Forever tooltip (10 November 2026): "Chance on hit: Calls forth an
//! Emerald Dragon Whelp to protect you in battle for a short period of time", with no rate or
//! duration on the tooltip; the engine keeps the weapon's 1 PPM, the summon's 45 second category
//! cooldown and the 15 second guardian it already used for a worn Dragon's Call.
//!
//! The request is a Retribution Paladin on the Manual Crowd Pummeler opener (fixtures/forever-gaps,
//! names replaced) whose swap set holds the weapon under test.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, prepared_refusals,
    simulate_prepared, PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const DRAGONS_CALL: i64 = 10847;
const CROWD_PUMMELER: i64 = 9449;
const WORN_SHORTSWORD: i64 = 25;
const MAIN_HAND: usize = 14;
const ACID_SPIT: i64 = 9591;

fn base() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/forever-gaps/retribution-paladin-swap-sword-of-zeal.request.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// The opener swaps onto Dragon's Call.
fn swapping_to_dragons_call() -> Value {
    let mut request = base();
    player(&mut request)["itemSwap"]["items"][MAIN_HAND] = json!({ "id": DRAGONS_CALL });
    request
}

/// Dragon's Call worn at the pull, a plain sword (Worn Shortsword) in the swap set. The rotation
/// swaps to the sword at a minute into the fight.
fn dragons_call_swapped_away() -> Value {
    let mut request = base();
    let paladin = player(&mut request);
    paladin["equipment"]["items"][MAIN_HAND] = json!({ "id": DRAGONS_CALL });
    paladin["itemSwap"]["items"][MAIN_HAND] = json!({ "id": WORN_SHORTSWORD });
    let swap = paladin["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["action"].get("itemSwap").is_some())
        .unwrap();
    swap["action"]["condition"] = json!({
        "cmp": {
            "lhs": { "currentTime": {} },
            "op": "OpGe",
            "rhs": { "const": { "val": "60s" } }
        }
    });
    request
}

/// Dragon's Call worn with no swap set.
fn dragons_call_worn() -> Value {
    let mut request = base();
    let paladin = player(&mut request);
    paladin["enableItemSwap"] = json!(false);
    paladin["equipment"]["items"][MAIN_HAND] = json!({ "id": DRAGONS_CALL });
    request
}

/// The rotation without its item swap action, so the set never goes on.
fn without_the_swap_action(mut request: Value) -> Value {
    let list = player(&mut request)["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    list.retain(|entry| entry["action"].get("itemSwap").is_none());
    request
}

fn prepare(request: &Value, rules: Rules) -> Result<Value, PrepareError> {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
}

fn run(request: &Value) -> (Value, Value) {
    let value = prepare(request, Rules::Forever).unwrap_or_else(|err| panic!("{err}"));
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    (
        value,
        result["raidMetrics"]["parties"][0]["players"][0].clone(),
    )
}

fn effects<'a>(prepared: &'a Value, kind: &str) -> Vec<&'a Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == kind)
        .collect()
}

/// The whelp's Acid Spit damage over the fight, zero when the player had no whelp out.
fn whelp_spit_damage(player: &Value) -> f64 {
    player["pets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|pet| pet["name"] == "Emerald Dragon Whelp")
        .flat_map(|pet| pet["actions"].as_array().unwrap())
        .filter(|action| action["id"]["spellId"].as_i64() == Some(ACID_SPIT))
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["damage"].as_f64().unwrap_or(0.0))
        .sum()
}

/// The swap onto Dragon's Call prepares, passes the gate and summons whelps once it is worn.
#[test]
fn a_swap_onto_dragons_call_runs_and_summons_the_whelp() {
    let (prepared, player) = run(&swapping_to_dragons_call());
    assert_eq!(effects(&prepared, "emerald_dragon_whelp").len(), 1);
    let swap = effects(&prepared, "item_swap");
    assert_eq!(swap.len(), 1);
    assert!(
        swap[0]["callbacks"]
            .to_string()
            .contains("Emerald Dragon Whelp Proc"),
        "the swap does not switch the weapon's proc: {}",
        swap[0]["callbacks"]
    );
    assert!(whelp_spit_damage(&player) > 0.0, "no whelp ever spat");
}

/// The proc is gated on the weapon being in the hand: with the set never put on, the whelp exists
/// (it is built for the set's weapon) and never comes out.
#[test]
fn the_whelp_comes_only_while_dragons_call_is_equipped() {
    let (_, never_swapped) = run(&without_the_swap_action(swapping_to_dragons_call()));
    assert_eq!(whelp_spit_damage(&never_swapped), 0.0);
    let (_, swapped) = run(&swapping_to_dragons_call());
    assert!(whelp_spit_damage(&swapped) > 0.0);
}

/// A set that is turned off holds no weapon the character can swap to, so there is no whelp.
#[test]
fn a_disabled_swap_set_builds_no_whelp() {
    let mut request = swapping_to_dragons_call();
    player(&mut request)["enableItemSwap"] = json!(false);
    let (prepared, player) = run(&request);
    assert!(effects(&prepared, "emerald_dragon_whelp").is_empty());
    assert_eq!(whelp_spit_damage(&player), 0.0);
}

/// Worn at the pull and swapped away for another sword: the whelp procs before the swap, and a
/// whelp already out stays for its 15 seconds as any summoned guardian does.
#[test]
fn dragons_call_worn_at_the_pull_and_swapped_away_runs() {
    let (_, player) = run(&dragons_call_swapped_away());
    assert!(whelp_spit_damage(&player) > 0.0);
    // It procs for less of the fight than a Dragon's Call that stays in the hand.
    let (_, worn) = run(&dragons_call_worn());
    assert!(whelp_spit_damage(&worn) > whelp_spit_damage(&player));
}

/// The same weapon worn with no swap set builds one whelp, as before.
#[test]
fn dragons_call_worn_without_a_swap_builds_one_whelp() {
    let (prepared, player) = run(&dragons_call_worn());
    assert_eq!(effects(&prepared, "emerald_dragon_whelp").len(), 1);
    assert!(effects(&prepared, "item_swap").is_empty());
    assert!(whelp_spit_damage(&player) > 0.0);
}

/// The reference's rules keep refusing the swap: the whelp is built only for a weapon in a hand
/// at the pull, and the proc opts out. The request is the reference's: its talent tree and a
/// Pummeler worn with the sword in the set.
#[test]
fn the_reference_rules_still_refuse_the_swap() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/forever-gaps/retribution-paladin-swap-without-crowd-pummeler.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let paladin = player(&mut request);
    paladin["buffs"] = json!({});
    paladin["talentsString"] = json!("52003-503-05225231001330321");
    paladin["equipment"]["items"][MAIN_HAND] = json!({ "id": CROWD_PUMMELER });
    paladin["enableItemSwap"] = json!(true);
    paladin["itemSwap"] = json!({
        "items": (0..17)
            .map(|slot| if slot == MAIN_HAND { json!({ "id": DRAGONS_CALL }) } else { json!({}) })
            .collect::<Vec<_>>()
    });
    request["raid"]["buffs"] = json!({});
    request["raid"]["parties"][0]["buffs"] = json!({});
    let reasons: Vec<String> = match prepare(&request, Rules::GoReference) {
        Ok(prepared) => {
            let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
            prepared_refusals(&typed)
                .into_iter()
                .map(|refusal| refusal.reason)
                .collect()
        }
        Err(PrepareError::Refused(refusal)) => vec![refusal.to_string()],
        Err(other) => panic!("{other}"),
    };
    assert_eq!(
        reasons,
        ["item_swap: an item swap with the effect of item 10847 is not prepared yet"]
    );
}

/// Found by the weapon survey of the same change: an item with an attack power bonus against a
/// creature type (Ceremonial Mallet, 279885) in a swap set was refused unless the target was of
/// that type, because only a bonus that applied was described and the swap's toggling of the
/// aura was then one the runtime did not follow. Against a target of another type the bonus is
/// described at zero, and the swap runs.
#[test]
fn a_creature_type_item_in_a_swap_runs_against_a_target_of_another_type() {
    let mut request = base();
    player(&mut request)["itemSwap"]["items"][MAIN_HAND] = json!({ "id": 279885 });
    let (prepared, _) = run(&request);
    let bonuses = effects(&prepared, "mob_type_bonus");
    assert_eq!(bonuses.len(), 1);
    assert_eq!(bonuses[0]["melee_attack_power"], 0.0);
}
