//! Hotfix push 113718 under Forever's rules: Wild Growth's periodic heal takes 0.066 of spell
//! power a tick at every rank (was 0.033). The push is in the client's hotfix file, not in the
//! patch notes; the reference keeps the frozen rows.

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const WILD_GROWTH: i64 = 1238215;
/// The Restoration druid reference build's talents in hotfix push 113531's order, and in the
/// reference's (`tests/hotfix_20261110.rs`).
const RESTORATION_DRUID: &str = "05302001--5053025153113051";
const RESTORATION_DRUID_REFERENCE: &str = "05002001--5050035153113051";

fn prepared(talents: &str, rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/healers/restoration-druid.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(talents);
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn tick_coefficient(prepared: &Value) -> Option<f64> {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["spell_id"] == WILD_GROWTH)
        .and_then(|spell| spell["dot"]["bonus_coefficient"].as_f64())
}

#[test]
fn wild_growth_ticks_take_twice_the_spell_power() {
    let forever = tick_coefficient(&prepared(RESTORATION_DRUID, Rules::Forever));
    assert_eq!(forever, Some(0.066));
    if let Some(reference) =
        tick_coefficient(&prepared(RESTORATION_DRUID_REFERENCE, Rules::GoReference))
    {
        assert!((reference - 0.033).abs() < 1e-6, "{reference}");
    }
}
