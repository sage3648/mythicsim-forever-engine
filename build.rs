//! Read the reference pin from upstream/sources.json, its only written copy.

use std::{env, fs, path::Path};

fn main() {
    let path = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("upstream/sources.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(&path).expect("read upstream/sources.json");
    let sources: serde_json::Value = serde_json::from_str(&text).expect("parse sources.json");
    let reference = sources["sources"]
        .as_array()
        .and_then(|sources| sources.iter().find(|source| source["id"] == "reference"))
        .expect("sources.json has a reference source");
    for (variable, field) in [
        ("FOREVER_REFERENCE_REVISION", "pinned_revision"),
        ("FOREVER_CLIENT_BUILD", "client_build"),
    ] {
        let value = reference[field]
            .as_str()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| panic!("sources.json reference lacks {field}"));
        println!("cargo:rustc-env={variable}={value}");
    }
}
