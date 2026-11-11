//! Forever client items that refused real players on 9 and 10 November 2026 ("no item with id"),
//! from Wowhead's Forever tooltips of 10 November (`tools/rust_client_items.py`):
//!
//! - Old Leather Jacket (280505): a leather chest of plain stats (109 Armor, +15 Strength, +2
//!   Agility, +2 Stamina) that the import left out for its name, which reads as development data.
//! - Krol'dok Battlegear, the five plate pieces of a Forever set the reference does not register:
//!   Chest 275005, Helmet 275006, Gauntlets 287012, Legplates 287013, Pauldrons 287014. A single
//!   piece is a plain item. Of its bonuses the (2) +100 Armor and (3) Increased Defense +5 are
//!   applied since 12 November 2026; the (4) heal on block and (5) Stamina when taking damage
//!   are not, so an equipment that reaches one of those is refused and names the bonus.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const OLD_LEATHER_JACKET: i64 = 280505;
const KROLDOK_CHEST: i64 = 275005;
const KROLDOK_HELMET: i64 = 275006;
const KROLDOK_GAUNTLETS: i64 = 287012;
const KROLDOK_LEGPLATES: i64 = 287013;
const KROLDOK_PAULDRONS: i64 = 287014;

const HEAD: usize = 0;
const SHOULDER: usize = 2;
const CHEST: usize = 4;
const HANDS: usize = 6;
const LEGS: usize = 8;

fn read(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The DPS Warrior of the Pummel request without buffs, 20 iterations, with the given items in
/// their slots and the five armor slots this file fills otherwise empty.
fn warrior(items: &[(usize, i64)]) -> Value {
    let mut request = read("fixtures/forever-gaps/dps-warrior-pummel.request.json");
    request["simOptions"]["iterations"] = json!(20);
    request["raid"]["buffs"] = json!({});
    request["raid"]["parties"][0]["buffs"] = json!({});
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["buffs"] = json!({});
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.resize(17, json!({}));
    for slot in [HEAD, SHOULDER, CHEST, HANDS, LEGS] {
        equipment[slot] = json!({});
    }
    for &(slot, id) in items {
        equipment[slot] = json!({ "id": id });
    }
    request
}

fn prepare(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// Prepared, through the gate and run.
fn run(request: &Value) {
    let prepared: PreparedV2 = serde_json::from_value(prepare(request)).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    simulate_prepared(&prepared).unwrap();
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap()
}

/// The difference the items make to the warrior's prepared stat.
fn added(items: &[(usize, i64)], name: &str) -> f64 {
    stat(&prepare(&warrior(items)), name) - stat(&prepare(&warrior(&[])), name)
}

fn refusal(request: &Value, rules: Rules) -> String {
    match prepare_json_with(request.to_string().as_bytes(), "rust", rules) {
        Err(PrepareError::Refused(refusal)) => refusal.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn old_leather_jacket_is_a_plain_chest() {
    let jacket = [(CHEST, OLD_LEATHER_JACKET)];
    run(&warrior(&jacket));
    assert_eq!(added(&jacket, "Armor"), 109.0 + 2.0 * 2.0);
    assert_eq!(added(&jacket, "Agility"), 2.0);
    assert!(added(&jacket, "Strength") >= 15.0);
    assert!(added(&jacket, "Stamina") >= 2.0);
}

#[test]
fn one_kroldok_piece_is_a_plain_item() {
    for (slot, id) in [
        (CHEST, KROLDOK_CHEST),
        (HEAD, KROLDOK_HELMET),
        (HANDS, KROLDOK_GAUNTLETS),
        (LEGS, KROLDOK_LEGPLATES),
        (SHOULDER, KROLDOK_PAULDRONS),
    ] {
        run(&warrior(&[(slot, id)]));
    }
    let chest = [(CHEST, KROLDOK_CHEST)];
    assert_eq!(added(&chest, "Armor"), 490.0);
    assert_eq!(added(&chest, "DefenseRating"), 13.0);
    let with = prepare(&warrior(&chest));
    let auras = with.to_string();
    assert!(
        !auras.contains("Krol'dok Battlegear 2P"),
        "no set bonus aura"
    );
}

/// Since 12 November 2026 the set's (2) +100 Armor and (3) Increased Defense +5 are applied
/// (`tests/forever_client_items_20261112.rs`); the (4) heal on block and the (5) Stamina when
/// taking damage still refuse by name.
#[test]
fn a_kroldok_set_bonus_that_is_not_prepared_is_refused_by_name() {
    let two = warrior(&[(CHEST, KROLDOK_CHEST), (LEGS, KROLDOK_LEGPLATES)]);
    run(&two);
    assert!(prepare(&two).to_string().contains("Krol'dok Battlegear 2P"));
    let five = warrior(&[
        (HEAD, KROLDOK_HELMET),
        (SHOULDER, KROLDOK_PAULDRONS),
        (CHEST, KROLDOK_CHEST),
        (HANDS, KROLDOK_GAUNTLETS),
        (LEGS, KROLDOK_LEGPLATES),
    ]);
    let reason = refusal(&five, Rules::Forever);
    assert!(
        reason.contains("set bonus Krol'dok Battlegear 4P is not prepared yet (item 287012)"),
        "{reason}"
    );
}

#[test]
fn the_reference_rules_still_refuse_the_items() {
    for (slot, id) in [(CHEST, OLD_LEATHER_JACKET), (CHEST, KROLDOK_CHEST)] {
        let reason = refusal(&warrior(&[(slot, id)]), Rules::GoReference);
        assert!(
            reason.contains(&format!("no item with id {id}")),
            "{reason}"
        );
    }
}
