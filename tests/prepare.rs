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
