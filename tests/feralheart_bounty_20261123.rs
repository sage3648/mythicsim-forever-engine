//! Blizzard's WoW Forever hotfixes of 23 November 2026: "The 4-piece Feralheart set for Druids
//! now correctly grants 4 Energy regeneration every 1 second for 5 seconds."
//!
//! Nature's Bounty (450608) on a landed white hit casts the periodic energize 27784. Client
//! 1.60.1.71139 rows: SpellEffect 704635 is a periodic energize (aura 24) of 4 every 1000 ms
//! whose power type moved from mana (71037) to energy, SpellDuration 28 is 5000 ms, and no
//! SpellAuraOptions row lets it stack, so a new proc restarts it. Under Forever's rules the
//! energy comes as those five ticks; the Go reference keeps 20 Energy at once.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::Value;
use std::{fs, path::Path};

/// The live Feral board request with Feralheart Spaulders, Vest, Hands and Kilt worn.
fn request() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/feralheart-20261123/feral-feralheart-4p.request.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn bounty(prepared: &Value) -> &Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "natures_bounty")
        .expect("the Nature's Bounty effect")
}

#[test]
fn forever_energy_is_four_each_second_for_five_seconds() {
    let request = request();
    let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err:?}"));
    let effect = bounty(&prepared);
    assert_eq!(effect["energy"], 4.0);
    assert_eq!(effect["energy_ticks"], 5);
    assert_eq!(effect["energy_period_ns"], 1_000_000_000_i64);

    let result = simulate_prepared(&serde_json::from_value(prepared).unwrap())
        .unwrap()
        .result;
    let player = &result["raidMetrics"]["parties"][0]["players"][0];
    let energy = player["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| {
            resource["id"]["spellId"] == 450608 && resource["type"] == "ResourceTypeEnergy"
        })
        .expect("Nature's Bounty energy metrics");
    let events = energy["events"].as_f64().unwrap();
    let gain = energy["gain"].as_f64().unwrap();
    assert!(events > 0.0, "the energize ticks");
    // Every tick offers 4 Energy, never the old 20 at once.
    assert!(
        (gain / events - 4.0).abs() < 1e-9,
        "gain {gain} over {events} ticks"
    );
}

#[test]
fn a_prepared_effect_without_ticks_gives_the_energy_at_once() {
    let request = request();
    let mut prepared =
        prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever).unwrap();
    let effect = prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "natures_bounty")
        .unwrap();
    effect["energy"] = 20.0.into();
    effect.as_object_mut().unwrap().remove("energy_ticks");
    effect.as_object_mut().unwrap().remove("energy_period_ns");
    let result = simulate_prepared(&serde_json::from_value(prepared).unwrap())
        .unwrap()
        .result;
    let player = &result["raidMetrics"]["parties"][0]["players"][0];
    let energy = player["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| {
            resource["id"]["spellId"] == 450608 && resource["type"] == "ResourceTypeEnergy"
        })
        .unwrap();
    let events = energy["events"].as_f64().unwrap();
    let gain = energy["gain"].as_f64().unwrap();
    assert!(
        (gain / events - 20.0).abs() < 1e-9,
        "gain {gain} over {events} procs"
    );
}
