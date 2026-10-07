//! Rust preparation against the accepted prepared v2 fixtures: every fixture's request is
//! read from the application's protojson and checked against what the Go exporter wrote.

use forever_engine::contracts::request::Request;
use serde::Deserialize;
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Deserialize)]
struct Manifest {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    request: RequestSource,
    prepared: String,
}

#[derive(Deserialize)]
struct RequestSource {
    path: String,
    #[serde(default)]
    pointer: String,
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn manifest() -> Manifest {
    let path = root().join("fixtures/mage/prepared-v2/manifest.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The request bytes a case names, the whole file or the value at its JSON pointer.
fn request_bytes(case: &Case) -> Vec<u8> {
    let bytes = fs::read(root().join(&case.request.path)).unwrap();
    if case.request.pointer.is_empty() {
        return bytes;
    }
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    serde_json::to_vec(value.pointer(&case.request.pointer).unwrap()).unwrap()
}

fn prepared(case: &Case) -> Value {
    let path = root()
        .join("fixtures/mage/prepared-v2")
        .join(&case.prepared);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn every_fixture_request_digest_matches_go() {
    let mut failures = Vec::new();
    let cases = manifest().cases;
    for case in &cases {
        let request = match Request::from_json(&request_bytes(case)) {
            Ok(request) => request,
            Err(err) => {
                failures.push(format!("{}: {err}", case.id));
                continue;
            }
        };
        let expected = prepared(case)["request_sha256"]
            .as_str()
            .unwrap()
            .to_string();
        if request.sha256() != expected {
            failures.push(format!(
                "{}: digest {} != {expected}",
                case.id,
                request.sha256()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {}:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

/// The paths at which two JSON values differ, at most `limit` of them.
fn differences(go: &Value, rust: &Value, path: &str, out: &mut Vec<String>, limit: usize) {
    if out.len() >= limit {
        return;
    }
    match (go, rust) {
        (Value::Object(a), Value::Object(b)) => {
            for (key, value) in a {
                match b.get(key) {
                    Some(other) => differences(value, other, &format!("{path}/{key}"), out, limit),
                    None => out.push(format!("{path}/{key}: missing in Rust")),
                }
            }
            for key in b.keys() {
                if !a.contains_key(key) && out.len() < limit {
                    out.push(format!("{path}/{key}: only in Rust"));
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                out.push(format!(
                    "{path}: {} items in Go, {} in Rust",
                    a.len(),
                    b.len()
                ));
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                differences(x, y, &format!("{path}/{i}"), out, limit);
            }
        }
        (Value::Number(a), Value::Number(b)) if a.as_f64() == b.as_f64() => {}
        (a, b) if a == b => {}
        (a, b) => out.push(format!("{path}: Go {a}, Rust {b}")),
    }
}

/// Every fixture Rust prepares must give the prepared state the Go exporter wrote; the rest
/// must be refused, never prepared wrongly. Set PREPARE_REPORT to print every case.
#[test]
fn rust_preparation_matches_go_or_refuses() {
    let report = std::env::var("PREPARE_REPORT").is_ok();
    let mut matched = 0;
    let mut refused = std::collections::BTreeMap::<String, usize>::new();
    let mut mismatched = Vec::new();
    let cases = manifest().cases;
    for case in &cases {
        match forever_engine::prepare_json(&request_bytes(case), &case.id) {
            Ok(rust) => {
                let go = prepared(case);
                let mut diffs = Vec::new();
                differences(&go, &rust, "", &mut diffs, 12);
                if diffs.is_empty() {
                    matched += 1;
                    if report {
                        println!("match   {}", case.id);
                    }
                } else {
                    mismatched.push(format!("{}:\n  {}", case.id, diffs.join("\n  ")));
                }
            }
            Err(forever_engine::PrepareError::Refused(refusal)) => {
                *refused.entry(refusal.code.to_string()).or_default() += 1;
                if report {
                    println!("refuse  {}: {refusal}", case.id);
                }
            }
            Err(err) => mismatched.push(format!("{}: {err}", case.id)),
        }
    }
    println!(
        "prepared in Rust and equal to Go: {matched} of {}",
        cases.len()
    );
    println!("refused: {refused:?}");
    assert!(
        mismatched.is_empty(),
        "{} prepared differently from Go:\n{}",
        mismatched.len(),
        mismatched.join("\n")
    );
}
