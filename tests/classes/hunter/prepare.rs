//! Rust preparation of a Hunter against the pinned Go exporter.
//!
//! Each case is a Hunter fixture request without buffs, debuffs, consumables and professions, with
//! weapons that carry no item effect (see tools/hunter_prepare_goldens.py), so what it prepares is
//! the Hunter's own: its spells, talent auras, class effects, pet and stats. The goldens keep a
//! digest of every spell, aura, effect and pet Go exported, so a failure names the item that
//! changed. Regenerate them only from the Go exporter.

use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Deserialize)]
struct Golden {
    case: String,
    request_sha256: String,
    sections: BTreeMap<String, Value>,
}

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/classes/hunter/prepare")
}

/// The text a digest is taken of: tools/mage_prepare_goldens.py `canonical`.
fn canonical(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push('z'),
        Value::Bool(true) => out.push('t'),
        Value::Bool(false) => out.push('f'),
        Value::Number(number) => {
            let bits = number.as_f64().expect("a finite number").to_bits();
            out.push('n');
            out.push_str(&format!("{bits:016x}"));
        }
        Value::String(text) => out.push_str(&format!("s{}:{text}", text.len())),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(&Value::String(key.clone()), out);
                out.push(':');
                canonical(&map[key], out);
            }
            out.push('}');
        }
    }
}

/// FNV-1a over the bytes from one offset basis.
fn fnv1a(bytes: &[u8], basis: u64) -> u64 {
    bytes.iter().fold(basis, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The goldens' digest, tools/hunter_prepare_goldens.py `digest`: two FNV-1a streams, which only
/// have to notice a change, not resist one.
fn digest(value: &Value) -> String {
    let mut text = String::new();
    canonical(value, &mut text);
    format!(
        "{:016x}{:016x}",
        fnv1a(text.as_bytes(), 0xcbf2_9ce4_8422_2325),
        fnv1a(text.as_bytes(), 0x8422_2325_cbf2_9ce4)
    )
}

fn label(section: &str, item: &Value) -> String {
    match section {
        "player.auras" | "target.auras" => item["label"].as_str().unwrap_or("?").to_string(),
        "effects" => item["kind"].as_str().unwrap_or("?").to_string(),
        "pets" => item["label"].as_str().unwrap_or("?").to_string(),
        _ => serde_json::to_string(&item["action_id"]).unwrap(),
    }
}

fn section<'a>(prepared: &'a Value, name: &str) -> &'a Value {
    match name {
        "player.spells" => &prepared["player"]["spells"],
        "player.auras" => &prepared["player"]["auras"],
        "target.auras" => &prepared["target"]["auras"],
        "effects" => &prepared["effects"],
        "player.major_cooldowns" => &prepared["player"]["major_cooldowns"],
        "player.stats" => &prepared["player"]["stats"],
        "player.pseudo_stats" => &prepared["player"]["pseudo_stats"],
        "player.mana" => &prepared["player"]["mana"],
        "player.talents" => &prepared["player"]["talents"],
        "encounter" => &prepared["encounter"],
        "melee" => &prepared["melee"],
        "pets" => &prepared["pets"],
        "unrepresented" => &prepared["unrepresented"],
        other => panic!("unknown golden section {other}"),
    }
}

/// What differs from the golden: one line per part.
fn differences(golden: &Golden, prepared: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if prepared["request_sha256"].as_str() != Some(golden.request_sha256.as_str()) {
        out.push("request digest".to_string());
    }
    for (name, expected) in &golden.sections {
        let actual = section(prepared, name);
        match expected {
            Value::String(want) => {
                if &digest(actual) != want {
                    out.push(format!("{name}: differs ({actual})"));
                }
            }
            Value::Array(items) => {
                // A build without a pet has no pets section at all, where the golden has none listed.
                let actual = match actual {
                    Value::Null => &[][..],
                    other => other.as_array().expect("a list section"),
                };
                if actual.len() != items.len() {
                    out.push(format!(
                        "{name}: {} items in Go, {} in Rust",
                        items.len(),
                        actual.len()
                    ));
                }
                for (want, got) in items.iter().zip(actual) {
                    if digest(got) != want[1].as_str().unwrap() {
                        out.push(format!(
                            "{name}: {} differs, Go labels it {}",
                            label(name, got),
                            want[0]
                        ));
                    }
                }
            }
            other => panic!("golden section {name} is {other}"),
        }
    }
    out
}

#[test]
fn every_hunter_case_prepares_as_go_does() {
    let mut cases = 0;
    let mut failures = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(directory()).unwrap().flatten().collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(case) = name.strip_suffix(".golden.json") else {
            continue;
        };
        cases += 1;
        let golden: Golden = serde_json::from_slice(&fs::read(entry.path()).unwrap()).unwrap();
        assert_eq!(golden.case, case);
        let request = fs::read(directory().join(format!("{case}.request.json"))).unwrap();
        match forever_engine::prepare_json(&request, case) {
            Ok(prepared) => {
                let diffs = differences(&golden, &prepared);
                if !diffs.is_empty() {
                    failures.push(format!("{case}:\n  {}", diffs.join("\n  ")));
                }
            }
            Err(err) => failures.push(format!("{case}: {err}")),
        }
    }
    assert!(cases >= 13, "the goldens are missing: {cases}");
    assert!(
        failures.is_empty(),
        "{} of {cases} differ from the Go exporter:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A Hunter without options has no ammo, quiver or pet to read, so it is refused rather than
/// prepared as some default.
#[test]
fn a_hunter_without_options_is_refused() {
    let request = fs::read(directory().join("beast-mastery-cat.request.json")).unwrap();
    let mut value: Value = serde_json::from_slice(&request).unwrap();
    value["raid"]["parties"][0]["players"][0]["hunter"] = serde_json::json!({});
    match forever_engine::prepare_json(&serde_json::to_vec(&value).unwrap(), "x") {
        Err(forever_engine::PrepareError::Refused(refusal)) => {
            assert_eq!(refusal.code, "request")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The pet is the hunter's second unit after the target, enabled by the reset with a full focus
/// bar, and a build without a pet type or without an uptime has none.
#[test]
fn a_pet_exists_only_with_a_type_and_an_uptime() {
    let request = fs::read(directory().join("beast-mastery-cat.request.json")).unwrap();
    let prepared = forever_engine::prepare_json(&request, "x").unwrap();
    let pets = prepared["pets"].as_array().unwrap();
    assert_eq!(pets.len(), 1);
    assert_eq!(pets[0]["label"], "hunter (#1) - Cat");
    assert_eq!(pets[0]["focus"]["max"], 100.0);

    for edit in [
        serde_json::json!({"petType": "PetNone"}),
        serde_json::json!({"petUptime": 0}),
    ] {
        let mut value: Value = serde_json::from_slice(&request).unwrap();
        let options =
            &mut value["raid"]["parties"][0]["players"][0]["hunter"]["options"]["classOptions"];
        for (key, change) in edit.as_object().unwrap() {
            options[key] = change.clone();
        }
        let prepared =
            forever_engine::prepare_json(&serde_json::to_vec(&value).unwrap(), "x").unwrap();
        assert!(prepared.get("pets").is_none(), "{edit}");
    }
}

/// Thunderstomp cleaves every active target and Rust simulates one hit, so a Gorilla against two
/// targets is prepared by Go with the ability named unrepresented, and Rust refuses it.
#[test]
fn a_gorilla_against_two_targets_is_refused() {
    let request = fs::read(directory().join("beast-mastery-gorilla.request.json")).unwrap();
    let mut value: Value = serde_json::from_slice(&request).unwrap();
    let targets = value["encounter"]["targets"].as_array_mut().unwrap();
    targets.push(targets[0].clone());
    match forever_engine::prepare_json(&serde_json::to_vec(&value).unwrap(), "x") {
        Err(forever_engine::PrepareError::Refused(refusal)) => {
            assert_eq!(refusal.code, "unrepresented");
            assert!(refusal.reason.contains("1264455"), "{}", refusal.reason);
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}
