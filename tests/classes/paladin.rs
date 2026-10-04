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

/// A tank's hardcast is followed only through the reduced avoidance rolls.
#[test]
fn a_tanking_hardcast_needs_reduced_avoidance_combinations() {
    let mut value = tank_json("protection-paladin-no-dynamite");
    value["enemy"]
        .as_object_mut()
        .unwrap()
        .remove("reduced_avoidance_rolls");
    assert!(reasons(value).contains(
        &"rotation reaches spell 24239, a hardcast while the target swings at the player".into()
    ));
}

/// Templar's Bulwark fires below the defensive health threshold or from the rotation; its
/// absorb shield and Forbearance run only through its effect.
#[test]
fn templars_bulwark_is_supported_with_its_effect() {
    for case in [
        "protection-bulwark-threshold",
        "protection-bulwark-rotation",
    ] {
        let prepared: PreparedV2 = serde_json::from_value(tank_json(case)).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{case}");
        let mut value = tank_json(case);
        value["effects"]
            .as_array_mut()
            .unwrap()
            .retain(|effect| effect["kind"] != "templars_bulwark");
        assert!(
            reasons(value)
                .iter()
                .any(|reason| reason.contains("1311015")
                    && (reason.contains("without a known behavior")
                        || reason.contains("with a defensive health threshold is unsupported"))),
            "{case}"
        );
    }
}

/// Stoneform below the defensive health threshold changes the physical damage the target's
/// swings deal, which the runtime follows only through the live school multiplier.
#[test]
fn stoneform_below_a_health_threshold_needs_the_live_school_multiplier() {
    let prepared: PreparedV2 =
        serde_json::from_value(tank_json("protection-paladin-dwarf-stoneform")).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
    let mut value = tank_json("protection-paladin-dwarf-stoneform");
    assert_eq!(
        value["enemy"]["school_damage_taken_auras"],
        json!(["player:Stoneform"])
    );
    for rolls in value["enemy"]["rolls"].as_array_mut().unwrap() {
        rolls
            .as_object_mut()
            .unwrap()
            .remove("school_damage_taken_multiplier");
    }
    assert!(reasons(value).contains(
        &"player:Stoneform changes the player's physical damage taken, which the runtime does \
          not follow"
            .into()
    ));
}

/// A survival cooldown neither a class nor a racial effect describes is not followed below
/// the defensive health threshold.
#[test]
fn an_undescribed_survival_cooldown_with_a_health_threshold_is_unsupported() {
    let mut value = tank_json("protection-paladin-dwarf-stoneform");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "stoneform");
    assert!(reasons(value)
        .iter()
        .any(|reason| reason.contains("with a defensive health threshold is unsupported")));
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

/// The builds beyond the presets: Exorcism and Holy Wrath, a lower Holy Shield rank, Light's
/// Vigil, Seal of the Crusader, and the talents and libram the talent sweep rejected.
#[test]
fn broadened_paladin_builds_are_supported() {
    for case in [
        "ret-undead-exorcism",
        "ret-exorcism-humanoid",
        "protection-holy-shield-rank2",
        "shockadin-lights-vigil",
        "ret-crusader-opener",
        "ret-crusader-fervor-over-raid",
        "ret-crusader-held",
        "protection-eye-for-an-eye",
        "shockadin-infusion-of-light",
        "shockadin-holy-alacrity",
        "ret-pursuit-of-justice",
    ] {
        let prepared: PreparedV2 = serde_json::from_value(tank_json(case)).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{case}");
    }
}

/// Seal of the Crusader's attack power rides on the stat aura combinations of the ranks the
/// rotation names.
#[test]
fn seal_of_the_crusader_needs_its_stat_aura() {
    let mut value = tank_json("ret-crusader-held");
    for effect in value["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "stat_auras" {
            effect["auras"]
                .as_array_mut()
                .unwrap()
                .retain(|aura| !aura.as_str().unwrap().starts_with("Seal of the Crusader"));
        }
    }
    assert!(reasons(value).contains(
        &"Seal of the Crusader 20308 without its stat aura combinations is unsupported".into()
    ));
}

/// Holy Light, Flash of Light and Holy Shock's heal on the target or the player, with
/// Illumination, Infusion of Light, Holy Alacrity and Blessing of Light, and the Goblin
/// Sapper Charge's hit on a paladin whose absorb shields are inactive or Templar's Bulwark.
#[test]
fn paladin_heals_are_supported() {
    for case in [
        "shockadin-holy-light-illumination",
        "shockadin-holy-light-self",
        "shockadin-flash-of-light-blessed",
        "shockadin-holy-shock-heal",
        "shockadin-holy-light-infusion",
        "protection-flash-of-light-self",
        "protection-paladin-sapper",
        "retribution-paladin-sapper",
        "protection-bulwark-sapper",
    ] {
        let prepared: PreparedV2 = serde_json::from_value(tank_json(case)).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{case}");
    }
}

/// A heal runs only through its effect, and Illumination's listener only through its own.
#[test]
fn heals_and_illumination_need_their_effects() {
    let mut value = tank_json("shockadin-holy-light-illumination");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "paladin_heals");
    assert!(
        reasons(value).contains(&"rotation reaches spell 25292 without a known behavior".into())
    );
    let mut value = tank_json("shockadin-holy-light-illumination");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "illumination");
    assert!(reasons(value)
        .iter()
        .any(|reason| reason.contains("listens to combat events without an effect")));
}

#[test]
fn holy_light_haste_and_eye_for_an_eye_need_their_effects() {
    for (case, kind) in [
        ("protection-eye-for-an-eye", "eye_for_an_eye"),
        ("shockadin-infusion-of-light", "holy_light_haste"),
        ("shockadin-holy-alacrity", "holy_light_haste"),
    ] {
        let mut value = tank_json(case);
        value["effects"]
            .as_array_mut()
            .unwrap()
            .retain(|effect| effect["kind"] != kind);
        assert!(
            reasons(value)
                .iter()
                .any(|reason| reason.contains("listens to combat events without an effect")),
            "{case}"
        );
    }
}

/// A class may activate an aura its effect carries without claiming it, so a swing-changing
/// aura any effect names is refused while the target swings at the player.
#[test]
fn a_carried_aura_that_changes_the_swing_is_refused() {
    let mut value = tank_json("tank-protection-paladin-autos");
    value["enemy"]["changing_auras"]
        .as_array_mut()
        .unwrap()
        .push(json!("player:Phantom Ward"));
    value["effects"].as_array_mut().unwrap().push(json!({
        "kind": "inert_listener",
        "unit": "pet",
        "aura": "Phantom Ward",
        "reason": "carried, not claimed"
    }));
    assert!(reasons(value)
        .contains(&"player:Phantom Ward changes the target's swings at the player".to_string()));
}

/// Seal of Fury on the tanks, with its shield, Improved Seal of Fury and Templar's Bulwark,
/// and twisted with Seal of Command through Twist of Light.
#[test]
fn seal_of_fury_is_supported() {
    for case in [
        "protection-paladin-seal-of-fury",
        "ret-protection-paladin-seal-of-fury",
        "holy-protection-paladin-seal-of-fury",
        "retribution-paladin-seal-of-fury-twist",
        "protection-paladin-seal-of-fury-bulwark-sapper",
    ] {
        let prepared: PreparedV2 = serde_json::from_value(tank_json(case)).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{case}");
    }
    let mut value = tank_json("protection-paladin-seal-of-fury");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "seal_of_fury");
    assert!(
        reasons(value).contains(&"rotation reaches spell 20423 without a known behavior".into())
    );
}

/// Lay on Hands, which the upstream P5 Protection rotation casts below 10% health, runs only
/// through its effect.
#[test]
fn lay_on_hands_needs_its_effect() {
    let prepared: PreparedV2 =
        serde_json::from_value(tank_json("protection-paladin-lay-on-hands")).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
    let mut value = tank_json("protection-paladin-lay-on-hands");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "lay_on_hands");
    assert!(
        reasons(value).contains(&"rotation reaches spell 10310 without a known behavior".into())
    );
}
