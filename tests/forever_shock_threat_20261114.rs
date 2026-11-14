//! Shock threat under Forever's rules (Discord #bugs, 14 November 2026: a rotation casting Frost
//! Shock made more threat than the same rotation casting Earth Shock).
//!
//! Earth Shock is the high threat shock. Its Forever tooltip ends "Causes a high amount of
//! threat" at every rank (8042 to 10414) and Frost Shock's tooltip does not, the same wording as
//! the Classic client, where measured threat doubles Earth Shock's damage. Forever's rules give
//! Earth Shock twice its damage as threat and Frost Shock its damage; the reference keeps its
//! own numbers (Frost Shock doubled, Earth Shock not).

use forever_engine::{prepare_json_with, Rules};
use serde_json::Value;
use std::{fs, path::Path};

const EARTH_SHOCK: i64 = 10414;
const FROST_SHOCK: i64 = 10473;

/// MythicSim's Enhancement reference as the app builds it (talents in the client's order) for
/// Forever's rules, and the recorded production request for the reference's.
fn prepared(rules: Rules) -> Value {
    let fixture = match rules {
        Rules::Forever => "fixtures/boss-attacks/enhancement-shaman.request.json",
        Rules::GoReference => {
            "fixtures/mage/prepared-v2/production-enhancement-shaman.request.json"
        }
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
    let request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    prepare_json_with(request.to_string().as_bytes(), "rust", rules).unwrap()
}

fn threat_multiplier(prepared: &Value, spell_id: i64) -> f64 {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| {
            spell["action_id"]["spell_id"].as_i64() == Some(spell_id)
                && spell["action_id"]["tag"].as_i64().unwrap_or(0) == 0
        })
        .unwrap_or_else(|| panic!("no spell {spell_id}"))["threat_multiplier"]
        .as_f64()
        .unwrap()
}

#[test]
fn earth_shock_is_the_high_threat_shock() {
    let forever = prepared(Rules::Forever);
    assert_eq!(threat_multiplier(&forever, EARTH_SHOCK), 2.0);
    assert_eq!(threat_multiplier(&forever, FROST_SHOCK), 1.0);

    let reference = prepared(Rules::GoReference);
    assert_eq!(threat_multiplier(&reference, EARTH_SHOCK), 1.0);
    assert_eq!(threat_multiplier(&reference, FROST_SHOCK), 2.0);
}
