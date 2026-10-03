use forever_engine::{implemented_prepared_effects, CLIENT_BUILD, SOURCE_REVISION};
use serde_json::Value;
use std::fs;

fn manifest() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("release/manifest.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn release_identity_matches_the_engine() {
    let manifest = manifest();
    assert_eq!(manifest["package_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest["go_reference"]["revision"], SOURCE_REVISION);
    assert_eq!(manifest["client_build"], CLIENT_BUILD);
    assert_eq!(
        manifest["contracts"]["prepared_inputs"],
        serde_json::json!([1, 2])
    );
}

#[test]
fn declared_capabilities_match_implemented_effects() {
    let manifest = manifest();
    let declared: Vec<&str> = manifest["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capability| capability["id"] == "prepared-v2-contract")
        .unwrap()["implemented_effects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|effect| effect.as_str().unwrap())
        .collect();
    assert_eq!(declared, implemented_prepared_effects());
}
