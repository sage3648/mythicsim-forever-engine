//! A Rogue's swap set under Forever's rules (MythicSim Rust-only): copies of the weapons worn
//! with poisons of their own, which a rotation's item swap puts on and takes off. Ambiwar's case
//! (Discord #feature-requests, 5 November 2026) is Deadly Poison on both hands until it has five
//! stacks, then a copy of the main hand carrying Instant Poison.
//!
//! The request is the Assassination board's reference (one target, the board seed), as the
//! worker builds it. A swap costs what the engine's item swap costs every class: the global
//! cooldown extends to 1.5 seconds (Go `ItemSwap.SwapItems`, `GCDDefault`) and both swings start
//! a full swing after the swap (`StopMeleeUntil`).

use forever_engine::{contracts::prepared_v2::PreparedV2, prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const INSTANT: i64 = 26891;
const DEADLY: i64 = 27186;
/// The board's weapons: Felstriker (Crusader) and Heartseeker (Crusader).
const MAIN_HAND: (i64, i64) = (12590, 1900);
const OFF_HAND: (i64, i64) = (12783, 1900);

fn board_request() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/classes/rogue/item_swap/assassination-board.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["saveAllValues"] = json!(true);
    request
}

/// The board's request with these poisons worn and a swap set: each `(slot, item, enchant,
/// poison)` of it, and a first rotation line that swaps to it while the Main set is worn.
fn with_swap(worn: [i64; 2], set: &[(usize, (i64, i64), i64)]) -> Value {
    let mut request = board_request();
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["consumables"]["mhImbueId"] = json!(worn[0]);
    player["consumables"]["ohImbueId"] = json!(worn[1]);
    let mut items = vec![json!({}); 17];
    for &(slot, (id, enchant), poison) in set {
        items[slot] = json!({"id": id, "enchant": enchant});
        let field = if slot == 14 {
            "mhImbueSwapId"
        } else {
            "ohImbueSwapId"
        };
        player["consumables"][field] = json!(poison);
    }
    player["enableItemSwap"] = json!(true);
    player["itemSwap"] = json!({ "items": items });
    let line = json!({"action": {
        "condition": {"activeItemSwapSet": {"swapSet": "Main"}},
        "itemSwap": {"swapSet": "Swap1"},
    }});
    player["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .insert(0, line);
    request
}

fn prepare(request: &Value, rules: Rules) -> Result<Value, String> {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules).map_err(|err| err.to_string())
}

fn prepared(request: &Value) -> Value {
    let value = prepare(request, Rules::Forever).unwrap();
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        forever_engine::prepared_coverage(&prepared),
        Vec::<String>::new()
    );
    value
}

/// Each iteration's DPS, as bits.
fn dps(value: Value) -> Vec<u64> {
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    let report = forever_engine::simulate_prepared(&prepared).unwrap();
    report.result["raidMetrics"]["dps"]["allValues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap().to_bits())
        .collect()
}

fn effect<'a>(value: &'a Value, kind: &str) -> &'a Value {
    value["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind}"))
}

/// The swap set's poisons are each poison's hands with the set worn: the export says the
/// reset's, and the swap's changes say the swap set's, which the gate follows.
#[test]
fn the_swap_sets_poisons_are_its_hands() {
    let value = prepared(&with_swap([DEADLY, DEADLY], &[(14, MAIN_HAND, INSTANT)]));
    let (main_hand, off_hand) = (
        json!(["ProcMaskMeleeMHAuto", "ProcMaskMeleeMHSpecial"]),
        json!(["ProcMaskMeleeOHAuto", "ProcMaskMeleeOHSpecial"]),
    );
    assert_eq!(effect(&value, "instant_poison")["proc_mask"], json!([]));
    assert_eq!(
        effect(&value, "deadly_poison")["proc_mask"],
        json!([
            "ProcMaskMeleeMHAuto",
            "ProcMaskMeleeOHAuto",
            "ProcMaskMeleeMHSpecial",
            "ProcMaskMeleeOHSpecial"
        ])
    );
    let swap = effect(&value, "item_swap");
    assert_eq!(swap["slots"], json!(["ItemSlotMainHand"]));
    let changes: Vec<(String, Value)> = swap["swapped"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|change| change["path"].as_str().unwrap().ends_with("/proc_mask"))
        .map(|change| {
            let path = change["path"].as_str().unwrap();
            let index: usize = path.split('/').nth(2).unwrap().parse().unwrap();
            let kind = value["effects"][index]["kind"]
                .as_str()
                .unwrap()
                .to_string();
            (kind, change["value"].clone())
        })
        .collect();
    assert_eq!(
        changes,
        vec![
            ("instant_poison".to_string(), main_hand),
            ("deadly_poison".to_string(), off_hand)
        ]
    );
}

/// Swapping to the copy at the pull is wearing its poison from the pull: the same fight, to the
/// bit, as a Rogue that wears the swap set's poisons and swaps to an identical copy. Both pay
/// the swap's global cooldown and swing restart at the pull, and every poison proc after it
/// hears the hands the set worn says.
#[test]
fn a_swap_at_the_pull_fights_as_the_swap_sets_poisons_worn() {
    let swapped = with_swap([DEADLY, DEADLY], &[(14, MAIN_HAND, INSTANT)]);
    let worn = with_swap([INSTANT, DEADLY], &[(14, MAIN_HAND, INSTANT)]);
    let swapped = dps(prepared(&swapped));
    assert_eq!(swapped, dps(prepared(&worn)));
    // And the swap moved the poisons: Deadly Poison on both hands fights otherwise.
    let deadly = with_swap([DEADLY, DEADLY], &[(14, MAIN_HAND, DEADLY)]);
    assert_ne!(swapped, dps(prepared(&deadly)));
}

/// Both hands: the copies swap the main hand's Instant Poison and the off hand's Deadly Poison.
#[test]
fn a_swap_of_both_hands_fights_as_both_poisons_worn() {
    let set = [(14, MAIN_HAND, DEADLY), (15, OFF_HAND, INSTANT)];
    let swapped = dps(prepared(&with_swap([INSTANT, DEADLY], &set)));
    assert_eq!(swapped, dps(prepared(&with_swap([DEADLY, INSTANT], &set))));
    let unswapped = [(14, MAIN_HAND, INSTANT), (15, OFF_HAND, DEADLY)];
    assert_ne!(
        swapped,
        dps(prepared(&with_swap([INSTANT, DEADLY], &unswapped)))
    );
}

/// The swap costs the global cooldown and restarts both swings: the next main hand and off hand
/// swings land a full swing after the swap.
#[test]
fn the_swap_extends_the_global_cooldown_and_restarts_both_swings() {
    let mut value = prepared(&with_swap([DEADLY, DEADLY], &[(14, MAIN_HAND, INSTANT)]));
    value["sim"]["iterations"] = json!(1);
    value["sim"]["debug_first_iteration"] = json!(true);
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    let report = forever_engine::simulate_prepared(&prepared).unwrap();
    let logs = report.result["logs"].as_str().unwrap();
    let lines: Vec<&str> = logs.lines().collect();
    let swap = lines
        .iter()
        .position(|line| line.contains("Item Swap to set Swap1"))
        .expect("the swap");
    assert!(lines[swap..]
        .iter()
        .take(3)
        .any(|line| line.contains("Extending GCD for 1.5s due to rotation / CDs.")));
    let time = |line: &str| -> f64 { line[1..line.find(']').unwrap()].parse().unwrap() };
    let at = time(lines[swap]);
    // Both swings restart: each lands more than a second after the swap (the board's daggers
    // swing every 1.3 seconds or more, hasted), not where its timer stood.
    for (hand, tag) in [("main hand", 1), ("off hand", 2)] {
        let swing = format!("Casting {{OtherID: 3, Tag: {tag}}}");
        let next = lines[swap..]
            .iter()
            .find(|line| line.contains(&swing))
            .map(|line| time(line))
            .unwrap_or_else(|| panic!("no {hand} swing after the swap"));
        assert!(
            next - at > 1.0,
            "{hand}: the swap at {at}, the swing at {next}"
        );
    }
}

/// Another weapon, a stone the sets differ in, or the reference's rules are refused.
#[test]
fn what_a_rogues_swap_set_cannot_change_is_refused() {
    let refused = |request: &Value, rules: Rules, text: &str| {
        let err = prepare(request, rules).unwrap_err();
        assert!(err.contains(text), "{err}");
    };
    let other_weapon = with_swap([DEADLY, DEADLY], &[(14, (12783, 1900), INSTANT)]);
    refused(
        &other_weapon,
        Rules::Forever,
        "a Rogue's swap set holds the same weapons with other poisons",
    );
    let other_enchant = with_swap([DEADLY, DEADLY], &[(14, (12590, 0), INSTANT)]);
    refused(
        &other_enchant,
        Rules::Forever,
        "a Rogue's swap set holds the same weapons with other poisons",
    );
    // Elemental Sharpening Stone on the copy.
    let stone = with_swap([DEADLY, DEADLY], &[(14, MAIN_HAND, 22756)]);
    refused(
        &stone,
        Rules::Forever,
        "a stone or oil the Rogue's sets differ in",
    );
    // Untalented, since the board's talents read in Forever's trees.
    let mut reference = with_swap([DEADLY, DEADLY], &[(14, MAIN_HAND, INSTANT)]);
    reference["raid"]["parties"][0]["players"][0]["talentsString"] = json!("");
    refused(
        &reference,
        Rules::GoReference,
        "a Rogue under the reference's rules",
    );
}
