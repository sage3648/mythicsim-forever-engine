//! Libram of Law (272435) under Forever's rules since Blizzard's hotfixes of 25 November 2026:
//! "Fixed a bug causing Libram of Law to not affect Judgement of Command." Its 4% more Judgement
//! damage reaches Judgement of Command beside Judgement of Righteousness and Fury. The Go
//! reference keeps Righteousness and Fury alone.

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// Judgement of Command, ranks 1 to 5.
const JUDGEMENT_OF_COMMAND: [i64; 5] = [20425, 20962, 20961, 20967, 20968];

/// The live Retribution reference's talents of 26 November 2026, which only Forever's rules take.
const RETRIBUTION: &str = "050050002-503-05233301001301351";

/// The Retribution production request with the libram in its libram slot, or none, on the live
/// tree under Forever's rules.
fn prepared(libram: Option<i64>, rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-retribution-paladin.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    if rules == Rules::Forever {
        request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(RETRIBUTION);
    }
    let items = request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    // The last item is the request's libram.
    items.pop();
    if let Some(id) = libram {
        items.push(json!({ "id": id }));
    }
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

/// Each Judgement of Command rank's damage multiplier, by spell id.
fn command_multipliers(prepared: &Value) -> Vec<(i64, f64)> {
    let out: Vec<(i64, f64)> = prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|spell| {
            let id = spell["action_id"]["spell_id"].as_i64()?;
            JUDGEMENT_OF_COMMAND
                .contains(&id)
                .then(|| (id, spell["damage_multiplier"].as_f64().unwrap()))
        })
        .collect();
    assert!(!out.is_empty(), "no Judgement of Command registered");
    out
}

fn ratio(rules: Rules) -> Vec<f64> {
    let with = command_multipliers(&prepared(Some(272435), rules));
    let without = command_multipliers(&prepared(None, rules));
    assert_eq!(with.len(), without.len());
    with.iter()
        .zip(&without)
        .map(|((a, x), (b, y))| {
            assert_eq!(a, b);
            x / y
        })
        .collect()
}

#[test]
fn libram_of_law_raises_judgement_of_command_under_forevers_rules() {
    for r in ratio(Rules::Forever) {
        assert!((r - 1.04).abs() < 1e-12, "{r}");
    }
}

#[test]
fn the_reference_leaves_judgement_of_command_alone() {
    for r in ratio(Rules::GoReference) {
        assert!((r - 1.0).abs() < 1e-12, "{r}");
    }
}
