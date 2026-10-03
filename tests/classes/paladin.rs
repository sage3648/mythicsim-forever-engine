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

fn shockadin_json() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-shockadin-paladin.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn tank_json(case: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn tanking_paladins_are_supported() {
    for case in [
        "protection-paladin-no-dynamite",
        "ret-protection-paladin-no-dynamite",
        "holy-protection-paladin-no-dynamite",
    ] {
        let prepared: PreparedV2 = serde_json::from_value(tank_json(case)).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{case}");
    }
}

/// A tank's hardcast is followed only through the "Reduced avoidance" stat aura.
#[test]
fn a_tanking_hardcast_needs_reduced_avoidance_combinations() {
    let mut value = tank_json("protection-paladin-no-dynamite");
    for effect in value["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "stat_auras" {
            let auras = effect["auras"].as_array_mut().unwrap();
            let position = auras
                .iter()
                .position(|aura| aura == "Reduced avoidance")
                .unwrap();
            auras[position] = json!("Renamed avoidance");
        }
    }
    assert!(reasons(value).contains(
        &"rotation reaches spell 24239, a hardcast while the target swings at the player".into()
    ));
}

/// Templar's Bulwark is described only as the survival cooldown Go never fires.
#[test]
fn templars_bulwark_cast_by_the_rotation_is_unsupported() {
    let mut value = tank_json("protection-paladin-no-dynamite");
    let list = value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    list.push(json!({"action": {"castSpell": {"spellId": {"spellId": 1311015}}}}));
    assert!(
        reasons(value).contains(&"a rotation that casts Templar's Bulwark is unsupported".into())
    );
}

#[test]
fn production_shockadin_request_is_supported() {
    let prepared: PreparedV2 = serde_json::from_value(shockadin_json()).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
}

/// The Storm Gauntlets' proc is a listener only its item effect describes.
#[test]
fn storm_gauntlets_need_their_damage_proc() {
    let mut value = shockadin_json();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "spell_data_damage_proc");
    assert!(reasons(value).contains(
        &"player aura \"Storm Gauntlets\" listens to combat events without an effect".into()
    ));
}

#[test]
fn holy_shock_and_divine_favor_need_their_effects() {
    for (kind, spell) in [("holy_shock", 20930), ("divine_favor", 20216)] {
        let mut value = shockadin_json();
        value["effects"]
            .as_array_mut()
            .unwrap()
            .retain(|effect| effect["kind"] != kind);
        assert!(
            reasons(value).contains(&format!(
                "rotation reaches spell {spell} without a known behavior"
            )),
            "{kind}"
        );
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
