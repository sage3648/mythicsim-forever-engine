//! Item swapping (sim/core/item_swaps.go): MythicSim's Manual Crowd Pummeler opener, which
//! swaps the main hand back to the player's own weapon once the Pummeler's haste fades, and the
//! targeted swaps of the fixture family (dual wield, a shield, a swap while casting or on the
//! global cooldown, a swap weapon that carries weapon skill). Their Go results and first-fight
//! logs are compared with the rest of the family; these tests keep what the swap does in a fight
//! and what the gate refuses.

use forever_engine::{contracts::prepared_v2::PreparedV2, prepared_coverage};
use serde_json::{json, Value};
use std::{fs, path::Path};

const SUPPORTED: [&str; 14] = [
    "mcp-shockadin-paladin",
    "mcp-retribution-paladin",
    "mcp-arms-warrior",
    "mcp-enhancement-shaman",
    "mcp-elehance-shaman-2-targets",
    "mcp-elehance-shaman-3-targets",
    "mcp-feral-druid",
    "mcp-feral-bear-druid",
    "item-swap-weapon-skill",
    "item-swap-dual-wield",
    "item-swap-shield",
    "item-swap-mid-cast",
    "item-swap-mid-gcd",
    "item-swap-orc-windfury-totem",
];

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{name}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn first_fight_log(mut value: Value) -> String {
    value["sim"]["iterations"] = json!(1);
    value["sim"]["debug_first_iteration"] = json!(true);
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    let report = forever_engine::simulate_prepared(&prepared).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

/// The refusals of a valid prepared input, as `code: reason`.
fn refusals(value: Value) -> Vec<String> {
    crate::refusal_codes(value)
        .into_iter()
        .map(|(code, reason)| format!("{code}: {reason}"))
        .collect()
}

fn item_swap(value: &mut Value) -> &mut Value {
    value["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "item_swap")
        .expect("an item swap effect")
}

#[test]
fn the_pummeler_openers_and_the_targeted_swaps_are_supported() {
    for case in SUPPORTED {
        let prepared: PreparedV2 = serde_json::from_value(fixture(case)).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{case}");
    }
}

/// The opener uses the Pummeler at the pull and swaps to the own weapon on the first evaluation
/// after its haste fades: the swap set's aura, the stats change toward Swap1 and a global
/// cooldown of at least 1.5 seconds. The iteration's end swaps back.
#[test]
fn the_opener_swaps_once_the_pummelers_haste_fades() {
    let logs = first_fight_log(fixture("mcp-arms-warrior"));
    let lines: Vec<&str> = logs.lines().collect();
    let at = |text: &str| {
        lines
            .iter()
            .position(|line| line.contains(text))
            .unwrap_or_else(|| panic!("no {text}"))
    };
    let faded = at("Aura faded: {SpellID: 13494}");
    let swap = at("Item Swap to set Swap1");
    assert!(faded < swap);
    let stats = lines[swap..]
        .iter()
        .position(|line| line.contains("Item Swap - Stats Change"))
        .unwrap();
    assert!(lines[swap + stats..]
        .iter()
        .take(3)
        .any(|line| line.contains("Extending GCD for 1.5s due to rotation / CDs.")));
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("Item Swap - Stats Change"))
            .count(),
        2,
        "the swap and the iteration's end"
    );
}

/// A swap set weapon with skill moves its hand's table once it is worn (patch 135).
#[test]
fn the_swap_weapons_skill_moves_the_main_hand_table() {
    let mut value = fixture("item-swap-weapon-skill");
    let swapped = item_swap(&mut value)["swapped"].as_array().unwrap().clone();
    let skill = swapped
        .iter()
        .find(|change| change["path"] == "/melee/main_hand_skill")
        .expect("the main hand table changes");
    assert_eq!(skill["value"]["skill"].as_f64(), Some(8.0));
    assert!(value["melee"].get("main_hand_skill").is_none());
}

/// Spirit Weapons turns Rockbiter's threat into a bonus while Rockbiter is up; a swap that puts
/// Rockbiter on or off changes the threat multiplier, which the runtime does not follow.
#[test]
fn a_toggled_rockbiter_with_spirit_weapons_is_refused() {
    assert!(refusals(fixture("item-swap-spirit-weapons")).contains(
        &"item_swap_unsupported: an item swap changes /player/pseudo_stats/threat_multiplier, \
          which the runtime does not follow"
            .to_string()
    ));
}

#[test]
fn an_item_swap_of_an_unchecked_class_is_refused() {
    let mut value = fixture("mcp-arms-warrior");
    value["player"]["class"] = json!("ClassRogue");
    assert!(refusals(value).contains(
        &"item_swap_unsupported: an item swap of a ClassRogue is unsupported".to_string()
    ));
}

#[test]
fn a_swapped_value_no_runtime_part_follows_is_refused() {
    let mut value = fixture("mcp-arms-warrior");
    item_swap(&mut value)["swapped"]
        .as_array_mut()
        .unwrap()
        .push(json!({"path": "/player/pseudo_stats/threat_multiplier", "value": 1.3}));
    assert!(refusals(value).contains(
        &"item_swap_unsupported: an item swap changes /player/pseudo_stats/threat_multiplier, \
          which the runtime does not follow"
            .to_string()
    ));
}

/// A spell's crit with the swap set worn is its crit at the reset with the weapon
/// specializations' modifiers the swap switches; any other value is refused.
#[test]
fn a_specialization_crit_must_follow_its_modifiers() {
    let mut value = fixture("item-swap-dual-wield");
    let change = item_swap(&mut value)["swapped"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|change| {
            change["path"]
                .as_str()
                .unwrap()
                .ends_with("/bonus_crit_percent")
        })
        .expect("a crit change");
    let path = change["path"].as_str().unwrap().to_string();
    change["value"] = json!(change["value"].as_f64().unwrap() + 1.0);
    assert!(refusals(value).contains(&format!(
        "item_swap_unsupported: an item swap changes {path}, which the runtime does not follow"
    )));
}

/// A swap only of the main hand is followed: another slot in the swap set is refused.
#[test]
fn a_swap_of_another_slot_is_refused() {
    let mut value = fixture("mcp-arms-warrior");
    item_swap(&mut value)["slots"] = json!(["ItemSlotMainHand", "ItemSlotOffHand"]);
    assert!(refusals(value)
        .iter()
        .any(|refusal| refusal.starts_with("item_swap_unsupported: an item swap of slots")));
}
