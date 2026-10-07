//! Rust preparation of a Shaman against the pinned Go exporter.
//!
//! Each case is a Shaman fixture request without buffs, debuffs, consumables, professions and
//! gear but its weapons (see tools/shaman_prepare_goldens.py), so what it prepares is the
//! Shaman's own: its spells, talent auras, imbues, totems, class effects and stats. The goldens
//! keep a digest of every spell, aura and effect Go exported, so a failure names the item that
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/classes/shaman/prepare")
}

/// The text a digest is taken of: tools/warlock_prepare_goldens.py `canonical`.
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

/// The engine's own SHA-256, which it keeps private to avoid a dependency.
use crate::sha256;

fn digest(value: &Value) -> String {
    let mut text = String::new();
    canonical(value, &mut text);
    sha256::hex(text.as_bytes())
}

fn label(section: &str, item: &Value) -> String {
    match section {
        "player.auras" | "target.auras" => item["label"].as_str().unwrap_or("?").to_string(),
        "effects" => item["kind"].as_str().unwrap_or("?").to_string(),
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
        "melee" => &prepared["melee"],
        "encounter" => &prepared["encounter"],
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
                let actual = actual.as_array().expect("a list section");
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
fn every_shaman_case_prepares_as_go_does() {
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
    assert!(cases >= 30, "the goldens are missing: {cases}");
    assert!(
        failures.is_empty(),
        "{} of {cases} differ from the Go exporter:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A case's request.
fn request(case: &str) -> Value {
    let bytes = fs::read(directory().join(format!("{case}.request.json"))).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn refusal_code(request: &Value) -> &'static str {
    match forever_engine::prepare_json(&serde_json::to_vec(request).unwrap(), "x") {
        Err(forever_engine::PrepareError::Refused(refusal)) => refusal.code,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// An unsupported Shaman option is refused with a stable code, never approximated.
#[test]
fn a_shaman_without_class_options_is_refused() {
    let mut value = request("elemental");
    value["raid"]["parties"][0]["players"][0]["elementalShaman"] = serde_json::json!({});
    assert_eq!(refusal_code(&value), "class_option");
    let mut value = request("enhancement");
    value["raid"]["parties"][0]["players"][0]["enhancementShaman"] = serde_json::json!({});
    assert_eq!(refusal_code(&value), "class_option");
}

/// Go's Shaman shield proc rate starts a periodic self hit the exporter cannot describe.
#[test]
fn a_shield_proc_rate_is_refused() {
    let mut value = request("elemental");
    let options = &mut value["raid"]["parties"][0]["players"][0]["elementalShaman"]["options"];
    options["classOptions"]["shieldProcrate"] = serde_json::json!(1.5);
    assert_eq!(refusal_code(&value), "unrepresented");
}

/// The exporter cannot prepare a Restoration shaman, which has no auto attacks for Windfury
/// Totem's extra attack, so neither does Rust.
#[test]
fn a_restoration_shaman_is_refused() {
    let mut value = request("elemental");
    let player = &mut value["raid"]["parties"][0]["players"][0];
    player.as_object_mut().unwrap().remove("elementalShaman");
    player["restorationShaman"] = serde_json::json!({"options": {"classOptions": {}}});
    assert_eq!(refusal_code(&value), "unrepresented");
}
