//! Broadened Warlock builds: the Imp's Firebolt, Siphon Life, Bane of Doom, Drain Life,
//! Wrack, Incinerate and a dynamic pet under the owner's stat changes. Their Go result and
//! first-fight log goldens are compared with every other supported case in
//! tests/classes/mage/prepared_v2.rs.

use forever_engine::{check_prepared, contracts::prepared_v2::PreparedV2, PreparedError};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn fixture(case: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn reasons(value: Value) -> Vec<String> {
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    match check_prepared(&prepared) {
        Err(PreparedError::Unsupported(reasons)) => reasons,
        other => panic!("expected unsupported, got {other:?}"),
    }
}

fn remove_effect(value: &mut Value, kind: &str) {
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != kind);
}

#[test]
fn broad_builds_are_supported() {
    for case in [
        "affliction-warlock-broad",
        "destruction-warlock-broad",
        "demonology-warlock-orc",
        "affliction-warlock-death-coil",
        "demonology-warlock-recklessness",
        "affliction-warlock-raid-curse-of-elements",
    ] {
        let prepared: PreparedV2 = serde_json::from_value(fixture(case)).unwrap();
        assert_eq!(check_prepared(&prepared), Ok(()), "{case}");
    }
}

/// Each newly reached spell needs its behavior.
#[test]
fn reached_spells_need_their_effects() {
    for (kind, spell) in [
        ("firebolt", 11763),
        ("siphon_life", 18881),
        ("bane_of_doom", 603),
        ("drain_life", 11700),
        ("wrack", 1316697),
    ] {
        let mut value = fixture("affliction-warlock-broad");
        remove_effect(&mut value, kind);
        let reasons = reasons(value);
        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains(&spell.to_string())),
            "{kind}: {reasons:?}"
        );
    }
    let mut value = fixture("affliction-warlock-death-coil");
    remove_effect(&mut value, "death_coil");
    assert!(reasons(value).iter().any(|reason| reason.contains("17926")));
    let mut value = fixture("demonology-warlock-recklessness");
    remove_effect(&mut value, "curse_of_recklessness");
    assert!(reasons(value).iter().any(|reason| reason.contains("11717")));
    let mut value = fixture("destruction-warlock-broad");
    remove_effect(&mut value, "incinerate");
    assert!(reasons(value)
        .iter()
        .any(|reason| reason.contains("1293813")));
}

/// A dynamic pet follows the owner's changes of the stats the runtime tracks; a change of
/// another stat it inherits is refused.
#[test]
fn dynamic_pet_refuses_untracked_inherited_changes() {
    let mut value = fixture("demonology-warlock-orc");
    for effect in value["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "stat_auras" {
            effect["changed"]
                .as_array_mut()
                .unwrap()
                .push(json!("SpellHitPercent"));
        }
    }
    let reasons = reasons(value);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("inherits changes of SpellHitPercent")),
        "{reasons:?}"
    );
}

/// Dots hasted by real haste are supported since a hawk's swings are one (community #703), as
/// are dots hasted by cast speed.
#[test]
fn real_haste_dots_are_accepted() {
    let mut value = fixture("affliction-warlock-broad");
    for spell in value["player"]["spells"].as_array_mut().unwrap() {
        if spell["action_id"]["spell_id"] == 11700 {
            spell["dot"]["affected_by_real_haste"] = json!(true);
        }
    }
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    assert!(check_prepared(&prepared).is_ok());
}
