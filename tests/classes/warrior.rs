//! Warrior tests: the prepared v2 gate on the production Fury Warrior request. Its Go result
//! and first-fight log are compared with the rest of the fixture family.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepared_coverage, PreparedError,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn fury_json() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-warrior.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn reasons(value: Value) -> Vec<String> {
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    match check_prepared(&prepared) {
        Err(PreparedError::Unsupported(reasons)) => reasons,
        other => panic!("expected unsupported, got {other:?}"),
    }
}

fn effect_mut<'a>(value: &'a mut Value, kind: &str) -> &'a mut Value {
    value["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap()
}

#[test]
fn production_fury_request_is_supported() {
    let prepared: PreparedV2 = serde_json::from_value(fury_json()).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
}

/// A stance change runs only with the stance category and the stance passives exported.
#[test]
fn a_stance_change_needs_the_stance_passives() {
    let mut value = fury_json();
    let last = value["player"]["rotation"]["priorityList"]
        .as_array()
        .unwrap()
        .len()
        - 1;
    let item = &mut value["player"]["rotation"]["priorityList"][last]["action"];
    assert_eq!(item["castSpell"]["spellId"]["spellId"], 2458);
    item["castSpell"]["spellId"]["spellId"] = json!(2457);
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "pseudo_stat_auras");
    assert!(reasons(value.clone()).contains(&"rotation reaches spell 2457, a stance change".into()));
    assert!(crate::refusal_codes(value).contains(&(
        "class_limit",
        "rotation reaches spell 2457, a stance change".into()
    )));
}

/// The raid's Expose Armor holds the armor category for good, so the warrior's own Sunder
/// Armor can never be cast. Without it the stacks would change the target's armor.
#[test]
fn sunder_armor_needs_a_blocked_category() {
    let mut value = fury_json();
    effect_mut(&mut value, "sunder_armor")["blocked"] = json!(false);
    assert_eq!(
        reasons(value),
        ["rotation reaches spell 11597, which stacks the warrior's own Sunder Armor"]
    );
}

/// Retaliation cast by hand runs only with its effect exported.
#[test]
fn retaliation_in_the_rotation_needs_its_effect() {
    let mut value = fury_json();
    value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            json!({"action": {"castSpell": {"spellId": {"spellId": 20230}}}}),
        );
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "retaliation");
    assert!(reasons(value).contains(&"rotation casts spell 20230, which has no behavior".into()));
}

/// The warrior's own Sunder Armor and Slam without Improved Slam run against Go.
#[test]
fn own_sunder_armor_and_a_swing_stopping_slam_are_supported() {
    for case in [
        "arms-warrior-own-sunder",
        "protection-warrior-own-sunder",
        "arms-warrior-no-improved-slam",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
        let prepared: PreparedV2 = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{case}");
    }
}

/// Without the target's armor category the warrior's own stacks could not set its armor.
#[test]
fn own_sunder_armor_needs_the_armor_category() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/protection-warrior-own-sunder.prepared.json");
    let mut value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| !(effect["kind"] == "exclusive_category" && effect["unit"] == "target"));
    assert!(reasons(value).contains(
        &"rotation reaches spell 11597, which stacks the warrior's own Sunder Armor".into()
    ));
}

/// Go gives `currentRage` no value without a rage bar, which drops the term.
#[test]
fn rage_without_a_rage_bar_is_unsupported() {
    let mut value = fury_json();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "rage_bar");
    let reasons = reasons(value);
    assert!(reasons.contains(&"rotation item 15 reads rage, which the player lacks".into()));
    assert!(reasons
        .contains(&"player aura \"RageBar\" listens to combat events without an effect".into()));
}

fn protection_json() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-protection-warrior.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn production_tank_requests_are_supported() {
    for case in [
        "production-protection-warrior",
        "production-fury-protection-warrior",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
        let prepared: PreparedV2 = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{case}");
    }
}

/// Revenge's trigger hears the target's swings; without its effect nothing runs it.
/// Last Stand below the defensive health threshold, with its maximum health on the stat aura
/// combinations, and a Greater Stoneshield Potion beside it.
#[test]
fn last_stand_below_a_health_threshold_is_supported() {
    for case in [
        "protection-warrior-last-stand",
        "fury-protection-warrior-last-stand",
        "protection-warrior-sapper-last-stand",
        "protection-warrior-greater-stoneshield",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
        let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{case}");
        // Last Stand runs only through its effect.
        let mut value = value;
        value["effects"]
            .as_array_mut()
            .unwrap()
            .retain(|effect| effect["kind"] != "last_stand");
        assert!(
            reasons(value).iter().any(|reason| reason.contains("12975")),
            "{case}"
        );
    }
}

/// Racial survival cooldowns and Shield Wall below the defensive health threshold, on
/// Protection Warriors without Last Stand.
#[test]
fn survival_cooldowns_below_a_health_threshold_are_supported() {
    for case in [
        "protection-warrior-dwarf-stoneform",
        "protection-warrior-orc-shatter-curse",
        "protection-warrior-sapper-orc-shatter-curse",
        "protection-warrior-shield-wall",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("fixtures/mage/prepared-v2/{case}.prepared.json"));
        let value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{case}");
        // Shield Wall runs only through its effect.
        let mut value = value;
        value["effects"]
            .as_array_mut()
            .unwrap()
            .retain(|effect| effect["kind"] != "shield_wall");
        assert!(
            reasons(value).iter().any(|reason| reason.contains("871")),
            "{case}"
        );
    }
}

#[test]
fn revenge_trigger_needs_its_effect() {
    let mut value = protection_json();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "revenge");
    assert!(reasons(value)
        .contains(&"player aura \"Revenge - Trigger\" reacts to the target's swings".into()));
}

/// The runtime reads the player's damage taken live, so an aura the exporter finds changing
/// it must be one an effect multiplies it with.
#[test]
fn an_untracked_damage_taken_aura_is_unsupported() {
    let mut value = protection_json();
    value["enemy"]["damage_taken_auras"]
        .as_array_mut()
        .unwrap()
        .push(json!("player:Enrage"));
    assert_eq!(
        reasons(value),
        ["player:Enrage changes the player's damage taken, which nothing multiplies"]
    );
}

fn warrior_fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{name}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// Enrage and Blood Craze act on the Goblin Sapper Charge's hit on the player with nothing
/// tanking it, and Improved Hamstring's trigger needs its effect.
#[test]
fn hit_taken_listeners_act_on_the_sappers_hit() {
    for name in [
        "warrior-enrage-sapper",
        "warrior-blood-craze-sapper",
        "arms-warrior-improved-hamstring",
    ] {
        let prepared: PreparedV2 = serde_json::from_value(warrior_fixture(name)).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{name}");
    }
    let mut value = warrior_fixture("arms-warrior-improved-hamstring");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "improved_hamstring");
    assert!(reasons(value).contains(
        &"player aura \"Improved Hamstring - Trigger\" listens to combat events without an effect"
            .into()
    ));
}

/// Sweeping Strikes' hit spell shares the cast's action ID and comes first in the spellbook,
/// so the major cooldown names its own spell by position, as Go holds the spell itself.
#[test]
fn sweeping_strikes_cooldown_names_its_own_spell() {
    let value = warrior_fixture("arms-warrior-sweeping-strikes");
    let spells = value["player"]["spells"].as_array().unwrap();
    let first = spells
        .iter()
        .find(|spell| spell["action_id"]["spell_id"] == 12723)
        .unwrap();
    assert_eq!(first["class_spell"], "sweeping_strikes_hit");
    let cooldown = value["player"]["major_cooldowns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cooldown| cooldown["action_id"]["spell_id"] == 12723)
        .unwrap();
    let own = cooldown["spell"].as_u64().unwrap() as usize;
    assert_eq!(spells[own]["class_spell"], "sweeping_strikes");

    let mut value = value.clone();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "sweeping_strikes");
    assert!(
        reasons(value).contains(&"rotation reaches spell 12723 without a known behavior".into())
    );
}

/// Battlegear of Might's rage and the Premier High Warlord's Shield Wall's strike back act on
/// the target's swings only with their effects.
#[test]
fn gear_listeners_of_the_target_swings_need_their_effects() {
    for (name, kind, label) in [
        (
            "protection-warrior-battlegear-of-might",
            "battlegear_of_might_rage",
            "Battlegear of Might 5P",
        ),
        (
            "protection-warrior-premier-shield-wall",
            "spell_data_damage_proc",
            "Premier High Warlord's Shield Wall",
        ),
    ] {
        let mut value = warrior_fixture(name);
        let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{name}");
        value["effects"]
            .as_array_mut()
            .unwrap()
            .retain(|effect| effect["kind"] != kind);
        assert!(
            reasons(value).contains(&format!(
                "player aura {label:?} reacts to the target's swings"
            )),
            "{name}"
        );
    }
}

/// A class whose spells have not been checked against several targets is refused there.
#[test]
fn several_targets_are_refused_until_the_class_is_checked() {
    let mut value = fury_json();
    value["encounter"]["target_count"] = json!(3);
    assert!(reasons(value)
        .contains(&"3 targets: several targets are not supported for ClassWarrior yet".into()));
}
