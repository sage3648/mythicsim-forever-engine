//! Wolfshead Helm (8345) on the live client: "an additional 5 Rage from activating Enrage and an
//! additional 5 Energy from activating Shifting Power" (Wowhead Forever's tooltip of the helm and
//! its spell 17768, 6 November 2026). Forever's rules give Shifting Power 5 more Energy; the Go
//! reference keeps its 20.

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const WOLFSHEAD_HELM: i64 = 8345;
const HEAD: usize = 0;
/// The live Feral board's talents in the client's order.
const TALENTS: &str = "050022-55000032023012125001-055";

fn request(helm: bool) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-feral-druid.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(TALENTS);
    if helm {
        request["raid"]["parties"][0]["players"][0]["equipment"]["items"][HEAD] =
            json!({ "id": WOLFSHEAD_HELM });
    }
    request
}

/// The Energy a Shifting Power cast restores, as the prepared input states it.
fn shifting_power_energy(request: &Value, rules: Rules) -> f64 {
    let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"));
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "shifting_power")
        .expect("a shifting_power effect")["energy"]
        .as_f64()
        .unwrap()
}

#[test]
fn wolfshead_helm_adds_five_energy_to_shifting_power() {
    let base = shifting_power_energy(&request(false), Rules::Forever);
    let helm = shifting_power_energy(&request(true), Rules::Forever);
    assert_eq!(helm - base, 5.0);
}
