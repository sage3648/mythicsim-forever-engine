//! Zarla's Snack Pouch (276744), a Forever trinket that refused real players' Quick Sims and a
//! Droptimizer on 13 November 2026 ("no item with id 276744": a Marksmanship Hunter, two
//! Affliction Warlocks and a Retribution Paladin). Its row is Wowhead's Forever tooltip of 13
//! November 2026, since the client ships its ItemSparse row encrypted: a unique trinket of item
//! level 55 with +13 Nature Resistance and "Equip: Increases damage and healing done by magical
//! spells and effects by up to 11."
//!
//! Its use, "Calls forth a Black Dragon Whelp to protect you in battle for a short period of time.
//! (15 Min Cooldown)", summons creature 274297 (spell 1315190, SpellEffect 1347318), and no client
//! row states what the whelp deals. The engine never presses it: the trinket is worn with its
//! stats alone (`tools/rust_client_items.py`, UNPRESSED_USE).
//!
//! No refused request was at hand, so the tests dress the Elemental Shaman of
//! `fixtures/forever-gaps/elemental-shaman-the-elements` in place of its second trinket.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const ZARLAS_SNACK_POUCH: i64 = 276744;

/// The fixture's second trinket (Burst of Knowledge 11832).
const TRINKET: usize = 13;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// The Elemental Shaman with this item in the second trinket slot and no consumables.
fn shaman(trinket: Value) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/forever-gaps/elemental-shaman-the-elements.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    let player = player(&mut request);
    player["consumables"] = json!({});
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.resize(17, json!({}));
    equipment[TRINKET] = trinket;
    request
}

fn prepared(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// A prepared input through the gate and run: the player's metrics.
fn simulate(value: &Value) -> Value {
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    let player = result["raidMetrics"]["parties"][0]["players"][0].clone();
    assert!(player["dps"]["avg"].as_f64().unwrap() > 0.0);
    player
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap_or(0.0)
}

/// The pouch prepares and runs on a caster: 11 spell damage, 11 healing and 13 Nature
/// Resistance over the same character with the slot empty, and nothing else.
#[test]
fn the_pouch_is_its_tooltips_stats() {
    let with = prepared(&shaman(json!({ "id": ZARLAS_SNACK_POUCH })));
    let without = prepared(&shaman(json!({})));
    simulate(&with);
    for (name, amount) in [
        ("SpellDamage", 11.0),
        ("HealingPower", 11.0),
        ("NatureResistance", 13.0),
    ] {
        assert_eq!(stat(&with, name) - stat(&without, name), amount, "{name}");
    }
    for name in ["Intellect", "Stamina", "SpellCritRating", "Armor"] {
        assert_eq!(stat(&with, name), stat(&without, name), "{name}");
    }
}

/// The kinds of the prepared effects, in order. An effect's stat snapshots carry the pouch's
/// stats, so only the kinds compare.
fn effect_kinds(prepared: &Value) -> Vec<Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|effect| effect["kind"].clone())
        .collect()
}

/// The whelp's use is never pressed: the pouch registers no effect and no cooldown, so the
/// prepared input holds the same effects and cooldowns as the empty slot's.
#[test]
fn the_whelp_use_is_not_registered() {
    let with = prepared(&shaman(json!({ "id": ZARLAS_SNACK_POUCH })));
    let without = prepared(&shaman(json!({})));
    assert_eq!(effect_kinds(&with), effect_kinds(&without));
    assert_eq!(
        with["player"]["major_cooldowns"],
        without["player"]["major_cooldowns"]
    );
    assert!(!with.to_string().contains("1315190"));
}

/// The reference's rules have no row for the pouch and refuse it as unknown.
#[test]
fn the_reference_rules_still_refuse_the_pouch() {
    let request = shaman(json!({ "id": ZARLAS_SNACK_POUCH }));
    match prepare_json_with(request.to_string().as_bytes(), "rust", Rules::GoReference) {
        Err(PrepareError::Refused(refusal)) => assert!(
            refusal
                .to_string()
                .contains(&format!("no item with id {ZARLAS_SNACK_POUCH}")),
            "{refusal}"
        ),
        other => panic!("expected a refusal, got {other:?}"),
    }
}
