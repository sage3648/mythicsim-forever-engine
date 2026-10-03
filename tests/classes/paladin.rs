//! Paladin tests: the prepared v2 gate on the production Retribution Paladin request. Its Go
//! result and first-fight log are compared with the rest of the fixture family.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepared_coverage, PreparedError,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn retribution_json() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-retribution-paladin.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn reasons(value: Value) -> Vec<String> {
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    match check_prepared(&prepared) {
        Err(PreparedError::Unsupported(reasons)) => reasons,
        other => panic!("expected unsupported, got {other:?}"),
    }
}

#[test]
fn production_retribution_request_is_supported() {
    let prepared: PreparedV2 = serde_json::from_value(retribution_json()).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
}

/// Leader of the Pack activates at the reset and Moonkin Aura, later in the same exclusive
/// category, displaces it before the pull.
#[test]
fn leader_of_the_pack_is_displaced_by_moonkin_aura() {
    let value = retribution_json();
    let leader = value["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == "Leader of the Pack (External)")
        .unwrap();
    assert_eq!(leader["active"], false);
    assert_eq!(leader["displaced_by"], "Moonkin Aura (External)");
}

#[test]
fn seal_listeners_need_their_talent_effects() {
    for kind in [
        "sanctified_judgement",
        "vengeance",
        "vindication",
        "sacred_arbiter",
        "twist_of_light",
    ] {
        let mut value = retribution_json();
        value["effects"]
            .as_array_mut()
            .unwrap()
            .retain(|effect| effect["kind"] != kind);
        assert!(
            reasons(value)
                .iter()
                .any(|reason| reason.contains("listens to combat events without an effect")),
            "{kind}"
        );
    }
}

#[test]
fn a_seal_without_its_effect_has_no_behavior() {
    let mut value = retribution_json();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "seal_of_righteousness");
    assert!(
        reasons(value).contains(&"rotation reaches spell 20293 without a known behavior".into())
    );
}

/// Only the melee swings are modeled; the ranged and any-swing readings are not.
#[test]
fn auto_time_to_next_reads_only_melee_swings() {
    let mut value = retribution_json();
    let item = &mut value["player"]["rotation"]["priorityList"][3]["action"]["condition"]["and"]
        ["vals"][1]["cmp"]["lhs"];
    assert_eq!(item["autoTimeToNext"]["autoType"], "MeleeAuto");
    *item = json!({"autoTimeToNext": {"autoType": "RangedAuto"}});
    assert!(reasons(value)
        .iter()
        .any(|reason| reason
            .contains("autoTimeToNext auto type Some(\"RangedAuto\") is unsupported")));
}
