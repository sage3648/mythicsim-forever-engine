//! Forever's rule of 22 November 2026, from the client's `GetManaRegen()` on a level 40
//! Shaman, client 1.60.1.71037, in two gear sets:
//!
//! - A Shaman regenerates 6.25 mana a second and an eighth of its Spirit, as Priests and Mages
//!   do, where the reference gives it 7.5 and a tenth.
//!
//! The Go reference keeps its own rule.

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The client's readings: Spirit, then `GetManaRegen()`'s mana a second out of casting and while
/// casting. The character wore 2 MP5 and had Warchief's Blessing (16609, 10 MP5) in both sets and
/// nothing else that regenerates mana, so the casting rate is the 12 MP5 alone (2.4 a second, read
/// as a float) and the out of casting rate less it is what Spirit gives.
const READINGS: [(f64, f64, f64); 2] = [(117.0, 23.276, 2.401), (151.0, 27.526, 2.401)];

fn request() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/elemental-shaman-dwarf-stoneform.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(50);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    request
}

fn prepare(rules: Rules) -> Value {
    prepare_json_with(request().to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

/// The prepared Shaman's Spirit and the mana a second its Spirit gives.
fn spirit_regen(prepared: &Value) -> (f64, f64) {
    let player = &prepared["player"];
    (
        player["stats"]["Spirit"].as_f64().unwrap(),
        player["mana"]["spirit_regen_per_second"].as_f64().unwrap(),
    )
}

#[test]
fn the_client_readings_are_six_and_a_quarter_and_an_eighth_of_spirit() {
    for (spirit, not_casting, casting) in READINGS {
        let from_spirit = not_casting - casting;
        assert!(
            (from_spirit - (6.25 + spirit / 8.0)).abs() < 0.001,
            "{spirit} Spirit gives {from_spirit}"
        );
        assert!(
            (from_spirit - (7.5 + spirit / 10.0)).abs() > 1.0,
            "{spirit} Spirit also fits the reference's rule"
        );
    }
}

#[test]
fn a_forever_shaman_regenerates_six_and_a_quarter_and_an_eighth_of_spirit() {
    let (spirit, regen) = spirit_regen(&prepare(Rules::Forever));
    assert!(spirit > 0.0);
    assert_eq!(regen, spirit.mul_add(1.0 / 8.0, 6.25));
}

#[test]
fn the_reference_shaman_keeps_seven_and_a_half_and_a_tenth_of_spirit() {
    let (spirit, regen) = spirit_regen(&prepare(Rules::GoReference));
    assert_eq!(regen, spirit.mul_add(1.0 / 10.0, 7.5));
}
