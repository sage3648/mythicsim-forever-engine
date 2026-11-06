//! Alterac Valley reputation off hands and shield whose live rows differ from the reference
//! database (Wowhead's Forever tooltips and gear planner, 6 November 2026; both AV quartermasters,
//! Thanthaldis Snowgleam 13217 and Jekyll Flandring 13219, sell them). The live game dropped
//! these items from item level 65 to 60 and the reference rows kept the item level 65 budget.
//! Forever's rules read the live rows (`data/forever-live-items.jsonl`); the reference's rules
//! keep the reference rows.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// Stormstrike Hammer, a one-hander that leaves the off hand free.
const STORMSTRIKE_HAMMER: i64 = 19104;

/// The holy paladin fixture with no talents (no talent moves these stats, and the two rules'
/// trees differ) wearing only Stormstrike Hammer and the given off hand (0 for none).
fn request(off_hand: i64) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/holy-paladin-healing.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!("");
    let mut items = vec![json!({ "id": STORMSTRIKE_HAMMER })];
    if off_hand != 0 {
        items.push(json!({ "id": off_hand }));
    }
    player["equipment"] = json!({ "items": items });
    serde_json::to_vec(&request).unwrap()
}

fn prepared(request: &[u8], rules: Rules) -> Value {
    prepare_json_with(request, "rust", rules).unwrap_or_else(|err| panic!("{err}"))
}

/// What the off hand adds to each named stat, under the given rules.
fn added(off_hand: i64, rules: Rules, stats: &[&str]) -> Vec<f64> {
    let without = prepared(&request(0), rules);
    let with = prepared(&request(off_hand), rules);
    stats
        .iter()
        .map(|stat| {
            let value = |prepared: &Value| prepared["player"]["stats"][stat].as_f64().unwrap();
            value(&with) - value(&without)
        })
        .collect()
}

/// An item, the stats it moves, their values on its live tooltip and in the reference row.
type OffHand = (i64, &'static [&'static str], &'static [f64], &'static [f64]);

/// Each item, the stats its tooltip states under Forever's rules, and the reference's.
const OFF_HANDS: [OffHand; 6] = [
    // Tome of Arcane Domination: Arcane 31 (was 34), 3 mana per 5 sec.
    (19308, &["ArcaneDamage", "MP5"], &[31.0, 3.0], &[34.0, 3.0]),
    // Tome of Shadow Force: 7 Stamina, Shadow 31 (was 34).
    (19309, &["ShadowDamage"], &[31.0], &[34.0]),
    // Tome of the Ice Lord: 8 Intellect, Frost 31 (was 34).
    (19310, &["FrostDamage"], &[31.0], &[34.0]),
    // Tome of Fiery Arcana: Fire 37 (was 40).
    (19311, &["FireDamage"], &[37.0], &[40.0]),
    // Lei of the Lifegiver: healing 37 and damage 12 (was 40 and 13), 5 mana per 5 sec.
    (
        19312,
        &["HealingPower", "SpellDamage", "MP5"],
        &[37.0, 12.0, 5.0],
        &[40.0, 13.0, 5.0],
    ),
    // Therazane's Touch: damage and healing 29 (was 31).
    (
        19315,
        &["HealingPower", "SpellDamage"],
        &[29.0, 29.0],
        &[31.0, 31.0],
    ),
];

#[test]
fn the_off_hands_carry_their_live_tooltip_numbers() {
    for (id, stats, live, reference) in OFF_HANDS {
        assert_eq!(added(id, Rules::Forever, stats), live, "item {id}");
        assert_eq!(
            added(id, Rules::GoReference, stats),
            reference,
            "item {id} under the reference's rules"
        );
    }
}

/// The Immovable Object: 2291 Armor, 41 Block, 14 Stamina and 25 Block Value, so 66 block value
/// in all (the planner's `blockamount`); the reference row carries 71.
#[test]
fn the_immovable_object_blocks_for_its_live_amount() {
    let stats = ["BlockValue", "Armor"];
    assert_eq!(added(19321, Rules::Forever, &stats), [66.0, 2291.0]);
    assert_eq!(added(19321, Rules::GoReference, &stats), [71.0, 2291.0]);
}

#[test]
fn the_prepared_requests_run() {
    for id in [19308, 19315, 19312] {
        let prepared = prepared(&request(id), Rules::Forever);
        let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
        check_prepared(&prepared).unwrap();
        simulate_prepared(&prepared).unwrap();
    }
}
