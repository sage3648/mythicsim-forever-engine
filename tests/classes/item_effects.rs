//! Item effects Go registers in code that need a shared mechanic: stat auras that stack,
//! Thunderfury's slow in the attack speed category and dots that share an aura. Their Go results
//! and first-fight logs are compared with the rest of the fixture family; these tests keep what
//! each mechanism has to do in a fight, and what the gate refuses.

use forever_engine::{contracts::prepared_v2::PreparedV2, prepared_coverage};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{name}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn first_fight_log(mut value: Value) -> String {
    value["sim"]["iterations"] = json!(1);
    value["sim"]["debug_first_iteration"] = json!(true);
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    let report = forever_engine::simulate_prepared(&prepared).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

/// The refusals of a valid prepared input, as `code: reason`.
fn refusals(value: Value) -> Vec<String> {
    crate::refusal_codes(value)
        .into_iter()
        .map(|(code, reason)| format!("{code}: {reason}"))
        .collect()
}

fn effect_of<'a>(value: &'a mut Value, kind: &str, key: &str, label: &str) -> &'a mut Value {
    value["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind && effect[key] == label)
        .unwrap_or_else(|| panic!("no {kind} effect for {label}"))
}

/// How many times the target swings in the first fight.
fn target_swings(logs: &str) -> usize {
    logs.lines()
        .filter(|line| line.contains("] [Target 1] Casting {OtherID: 3, Tag: 1}"))
        .count()
}

/// Each stack of Bonereaver's Edge adds 700 armor penetration, and the stat auras number a
/// combination by the stacks, not only by whether the aura is up.
#[test]
fn a_stacking_stat_aura_is_numbered_by_its_stacks() {
    for name in [
        "warrior-bonereavers-edge",
        "protection-paladin-bonereavers-edge",
        "survival-hunter-bonereavers-edge",
        "combat-rogue-3-targets-bonereavers-edge",
    ] {
        let value = fixture(name);
        let auras = value["effects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|effect| effect["kind"] == "stat_auras")
            .unwrap();
        let labels: Vec<&str> = auras["auras"]
            .as_array()
            .unwrap()
            .iter()
            .map(|label| label.as_str().unwrap())
            .collect();
        let stacks: Vec<i64> = auras["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|stacks| stacks.as_i64().unwrap())
            .collect();
        let position = labels
            .iter()
            .position(|label| *label == "Bonereaver's Edge")
            .unwrap();
        assert_eq!(stacks[position], 3, "{name}");
        // The bits before the aura: one for each aura read as active or not.
        let offset: u32 = stacks[..position]
            .iter()
            .map(|stacks| if *stacks > 0 { 2 } else { 1 })
            .sum();
        let total: u32 = stacks
            .iter()
            .map(|stacks| if *stacks > 0 { 2 } else { 1 })
            .sum();
        let combos = auras["combos"].as_array().unwrap();
        assert_eq!(combos.len(), 1 << total, "{name}");
        assert!(auras["changed"]
            .as_array()
            .unwrap()
            .contains(&json!("ArmorPenetration")));
        let penetration = |combo: usize| combos[combo]["ArmorPenetration"].as_f64().unwrap();
        for level in 0..=3usize {
            assert_eq!(
                penetration(level << offset) - penetration(0),
                700.0 * level as f64,
                "{name}"
            );
        }
        assert_eq!(
            prepared_coverage(&serde_json::from_value(value).unwrap()),
            Vec::<String>::new()
        );
    }
}

/// The handler adds a stack after it activates the aura: the first fight climbs to 3 stacks and
/// loses them all when the aura fades. Without the stack the aura never holds any.
#[test]
fn a_weapon_proc_that_stacks_adds_a_stack_after_activating() {
    let value = fixture("warrior-bonereavers-edge");
    let logs = first_fight_log(value.clone());
    for climb in ["0 --> 1", "1 --> 2", "2 --> 3", "3 --> 0"] {
        assert!(
            logs.lines()
                .any(|line| line.contains(&format!("{{SpellID: 21153}} stacks: {climb}"))),
            "{climb}"
        );
    }

    let mut without = value;
    effect_of(
        &mut without,
        "stat_proc",
        "trigger_aura",
        "Bonereaver's Edge Proc",
    )
    .as_object_mut()
    .unwrap()
    .remove("add_stack");
    let logs = first_fight_log(without);
    assert!(!logs.contains("{SpellID: 21153} stacks:"));
}

/// Armor penetration comes off the target's armor in each hit: the same fight with the aura
/// stripped of its penetration takes more armor from every physical hit.
#[test]
fn armor_penetration_is_read_live() {
    let value = fixture("warrior-bonereavers-edge");
    let reduced = |value: &Value| -> f64 {
        let mut value = value.clone();
        value["sim"]["iterations"] = json!(1);
        let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
        forever_engine::simulate_prepared(&prepared).unwrap().result["raidMetrics"]["dps"]["avg"]
            .as_f64()
            .unwrap()
    };
    let mut flat = value.clone();
    for effect in flat["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "stat_auras" {
            for combo in effect["combos"].as_array_mut().unwrap() {
                combo["ArmorPenetration"] = json!(0.0);
            }
        }
    }
    assert!(reduced(&value) > reduced(&flat));
}

/// A stat proc with stacks needs the effect that adds them: the trigger listens to hits.
#[test]
fn bonereavers_edge_needs_its_effect() {
    let mut value = fixture("warrior-bonereavers-edge");
    let index = value["effects"]
        .as_array()
        .unwrap()
        .iter()
        .position(|effect| effect["trigger_aura"] == "Bonereaver's Edge Proc")
        .unwrap();
    value["effects"].as_array_mut().unwrap().remove(index);
    assert!(refusals(value).contains(
        &"aura_listener_unclaimed: player aura \"Bonereaver's Edge Proc\" listens to combat \
          events without an effect"
            .to_string()
    ));
}

/// Thunderfury casts its strike and then its bounce, a batch window after the hit, and a landed
/// strike puts Cyclone on its target.
#[test]
fn thunderfury_strikes_then_bounces() {
    let logs = first_fight_log(fixture("warrior-thunderfury"));
    let strike = logs
        .lines()
        .position(|line| line.contains("Casting {SpellID: 21992, Tag: 1}"))
        .expect("the strike");
    let lines: Vec<&str> = logs.lines().skip(strike).take(12).collect();
    let bounce = lines
        .iter()
        .position(|line| line.contains("Casting {SpellID: 21992, Tag: 2}"))
        .expect("the bounce");
    let cyclone = lines
        .iter()
        .position(|line| line.contains("[Target 1] Aura gained: {SpellID: 27648}"))
        .expect("Cyclone");
    let resistance = lines
        .iter()
        .position(|line| line.contains("[Target 1] Aura gained: {SpellID: 21992}"))
        .expect("the resistance aura");
    assert!(cyclone < bounce && bounce < resistance, "{lines:#?}");
}

/// Cyclone is the only slow of the attack speed category here, so the target swings slower than
/// it would without it, and a slow of nothing leaves the swings as they were.
#[test]
fn cyclone_slows_the_target_that_swings_at_the_tank() {
    let value = fixture("protection-paladin-thunderfury");
    let mut unslowed = value.clone();
    effect_of(
        &mut unslowed,
        "thunderfury",
        "trigger_aura",
        "Thunderfury Proc",
    )["slow_multiplier"] = json!(1.0);
    let slowed = target_swings(&first_fight_log(value));
    let unslowed = target_swings(&first_fight_log(unslowed));
    assert!(slowed < unslowed, "{slowed} swings against {unslowed}");
}

/// The raid's permanent Thunder Clap bids as much and outlasts Cyclone, so it keeps the
/// category and Cyclone slows nothing more.
#[test]
fn a_permanent_clap_keeps_the_attack_speed_category_from_cyclone() {
    let value = fixture("protection-paladin-thunderfury-thunder-clap-debuff");
    let mut unslowed = value.clone();
    effect_of(
        &mut unslowed,
        "thunderfury",
        "trigger_aura",
        "Thunderfury Proc",
    )["slow_multiplier"] = json!(1.0);
    let logs = first_fight_log(value);
    assert!(logs.contains("[Target 1] Aura gained: {SpellID: 27648}"));
    assert_eq!(
        target_swings(&logs),
        target_swings(&first_fight_log(unslowed))
    );
}

/// The warrior's Thunder Clap and Cyclone bid in one category, and a warrior that casts the clap
/// under Thunderfury is prepared with the clap's effect: both slows are described. The clap
/// outlasts Cyclone and bids the same, so it keeps the category whenever it is up.
#[test]
fn a_warrior_shares_the_attack_speed_category_with_cyclone() {
    let value = fixture("protection-warrior-3-targets-thunderfury");
    let kinds: Vec<&str> = value["effects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|effect| effect["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"thunder_clap") && kinds.contains(&"thunderfury"));
    let memberships = |label: &str| -> Value {
        value["target"]["auras"]
            .as_array()
            .unwrap()
            .iter()
            .find(|aura| aura["label"] == label)
            .unwrap()["exclusive_memberships"]
            .clone()
    };
    assert_eq!(memberships("Cyclone")[0]["category"], "AtkSpdReduction");
    let mut unslowed = value.clone();
    effect_of(
        &mut unslowed,
        "thunderfury",
        "trigger_aura",
        "Thunderfury Proc",
    )["slow_multiplier"] = json!(1.0);
    assert_eq!(
        target_swings(&first_fight_log(value.clone())),
        target_swings(&first_fight_log(unslowed))
    );
    assert_eq!(
        memberships("Thunder Clap (Player)")[0]["category"],
        "AtkSpdReduction"
    );
}

/// Thunderfury's trigger and its two target auras need the effect.
#[test]
fn thunderfury_needs_its_effect() {
    let mut value = fixture("warrior-thunderfury");
    let index = value["effects"]
        .as_array()
        .unwrap()
        .iter()
        .position(|effect| effect["kind"] == "thunderfury")
        .unwrap();
    value["effects"].as_array_mut().unwrap().remove(index);
    let reasons = refusals(value);
    assert!(
        reasons.contains(
            &"aura_listener_unclaimed: player aura \"Thunderfury Proc\" listens to combat \
              events without an effect"
                .to_string()
        ),
        "{reasons:?}"
    );
}

/// Two weapons whose spells share a name put their dots on one aura, which ticks both dots.
#[test]
fn dots_that_share_an_aura_tick_together() {
    let logs = first_fight_log(fixture("warrior-plaguefang-stinging-viper"));
    let mut both = 0;
    let mut previous: Option<&str> = None;
    for line in logs.lines().filter(|line| line.contains(" tick ")) {
        let time = line.split(']').next().unwrap();
        if previous == Some(time) {
            both += 1;
        }
        previous = Some(time);
    }
    // At some second the aura held both dots, and both ticked on it.
    assert!(both > 0, "no second held a tick of both dots");
    assert!(logs.contains("{SpellID: 1309315} tick"));
    assert!(logs.contains("{SpellID: 1291663} tick"));
    // Stinging Viper's cast puts its dot on the aura Plaguefang registered, under its action.
    assert!(!logs.contains("Aura gained: {SpellID: 1291663}"));
}

/// The gate follows the sharing for weapon procs only: a dot that is not a weapon proc's
/// still cannot share an aura.
#[test]
fn only_weapon_proc_dots_share_an_aura() {
    let original = fixture("warrior-plaguefang-stinging-viper");
    let prepared: PreparedV2 = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
    let mut value = original;
    let index = value["effects"]
        .as_array()
        .unwrap()
        .iter()
        .position(|effect| effect["trigger_aura"] == "Stinging Viper Proc")
        .unwrap();
    value["effects"][index]["periodic"] = Value::Null;
    let reasons = refusals(value);
    assert!(
        reasons.iter().any(|reason| reason.starts_with(
            "effect_unimplemented: the dots of spell 1309315 and spell 1291663 share the aura"
        )),
        "{reasons:?}"
    );
}
