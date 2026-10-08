//! The external Power Infusion any class receives from priests in the raid
//! (`individualBuffs.powerInfusions`, Go's generated external cooldown): what the gate needs to
//! accept it and how its sources take turns. The Go result and first-fight log goldens of every
//! class that receives it are compared with the other supported cases in
//! `mage/prepared_v2.rs`.

use super::refusal_codes;
use forever_engine::{contracts::prepared_v2::PreparedV2, simulate_prepared};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn fixture(case: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn without_effect(mut value: Value, kind: &str) -> Value {
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != kind);
    value
}

/// The seconds into the first fight at which the cooldown manager cast Power Infusion on the
/// player.
fn casts(mut value: Value, sources: u64) -> Vec<f64> {
    for effect in value["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "external_cooldown" {
            effect["sources"] = json!(sources);
        }
    }
    value["sim"]["iterations"] = json!(1);
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["logs"]
        .as_str()
        .unwrap()
        .lines()
        .filter(|line| line.ends_with("Major cooldown used: {SpellID: 10060, Tag: -1}"))
        .map(|line| {
            let end = line.find(']').unwrap();
            line[1..end].parse().unwrap()
        })
        .collect()
}

#[test]
fn the_gate_needs_the_cooldown_and_the_aura_effects() {
    let value = fixture("fire-mage-external-power-infusion");
    assert_eq!(refusal_codes(value.clone()), []);
    // The spell is Go's external cooldown; without its effect it has no behavior.
    assert_eq!(
        refusal_codes(without_effect(value.clone(), "external_cooldown")),
        [(
            "unknown_spell",
            "rotation reaches spell 10060 tag -1 without a known behavior".to_string()
        )]
    );
    // The multipliers are the aura's exclusive effects, which only its effect describes.
    assert_eq!(
        refusal_codes(without_effect(value, "power_infusion")),
        [(
            "aura_listener_unclaimed",
            "player aura \"Power Infusions (External)\" bids in Power Infusion's categories \
             without an effect"
                .to_string()
        )]
    );
}

#[test]
fn the_sources_take_turns_and_wait_out_their_cooldowns() {
    let value = fixture("fire-mage-external-power-infusion");
    for sources in 1..=3 {
        let times = casts(value.clone(), sources);
        // The cooldown is three minutes, so each source casts once in a two minute fight, each
        // after the aura of the one before has ended.
        assert_eq!(times.len() as u64, sources, "{times:?}");
        for pair in times.windows(2) {
            assert!(pair[1] - pair[0] >= 15.0, "{times:?}");
        }
    }
}

#[test]
fn a_source_returns_when_its_cooldown_ends() {
    let times = casts(fixture("fire-mage-external-power-infusion-long-fight"), 2);
    assert!(times.len() >= 4, "{times:?}");
    for (earlier, later) in times.iter().zip(&times[2..]) {
        assert!(later - earlier >= 180.0, "{times:?}");
    }
}
