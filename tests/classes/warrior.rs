//! Warrior tests: the prepared v2 gate on the production Fury Warrior request. Its Go result
//! and first-fight log are compared with the rest of the fixture family.

mod prepare;

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

fn first_fight_log(mut value: Value) -> String {
    value["sim"]["iterations"] = json!(1);
    value["sim"]["debug_first_iteration"] = json!(true);
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    let report = forever_engine::simulate_prepared(&prepared).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

fn lines_with<'a>(logs: &'a str, target: u32, spell: &str) -> Vec<&'a str> {
    logs.lines()
        .filter(|line| line.contains(&format!("[Target {target}] {{SpellID: {spell}")))
        .collect()
}

/// Whirlwind's cap on the row is four targets, so a fifth is never struck, and each of the
/// four takes both hands' strikes.
#[test]
fn whirlwind_strikes_four_of_five_targets() {
    let logs = first_fight_log(warrior_fixture("warrior-whirlwind-5-targets"));
    for target in 1..=4 {
        let hits = lines_with(&logs, target, "1680");
        for (tag, hand) in [("Tag: 1", "main"), ("Tag: 2", "off")] {
            assert!(
                hits.iter().any(|line| line.contains(tag)),
                "Target {target}: no {hand} hand strike"
            );
        }
    }
    assert!(lines_with(&logs, 5, "1680").is_empty());
}

/// Cleave strikes two targets, the cast target and the next, and calculates both hits before
/// it deals either: the damage lines of one cast come after both debug lines.
#[test]
fn cleave_strikes_two_targets_from_the_cast_target() {
    let logs = first_fight_log(warrior_fixture("production-warrior-2-targets"));
    let lines: Vec<&str> = logs.lines().collect();
    let cast = lines
        .iter()
        .position(|line| line.contains("Casting {SpellID: 20569}"))
        .unwrap();
    let rest = &lines[cast..];
    let position = |needle: &str, deal: bool| {
        rest.iter()
            .position(|line| line.contains(needle) && line.contains("[DEBUG]") != deal)
            .unwrap()
    };
    let first_debug = position("[Target 1] {SpellID: 20569} ", false);
    let second_debug = position("[Target 2] {SpellID: 20569} ", false);
    let first_deal = position("[Target 1] {SpellID: 20569} ", true);
    let second_deal = position("[Target 2] {SpellID: 20569} ", true);
    assert!(first_debug < second_debug);
    assert!(second_debug < first_deal && first_deal < second_deal);
}

/// Thunder Clap's debuff lands on each target the clap lands on, up to the row's four.
#[test]
fn thunder_clap_debuffs_each_target_it_lands_on() {
    let logs = first_fight_log(warrior_fixture("arms-warrior-thunder-clap-5-targets"));
    for target in 1..=4 {
        let gained = format!("[Target {target}] Aura gained: {{SpellID: 11581}}");
        assert!(logs.lines().any(|line| line.contains(&gained)), "{gained}");
    }
    assert!(!logs
        .lines()
        .any(|line| line.contains("[Target 5] Aura gained: {SpellID: 11581}")));
}

/// A landed melee hit while two targets are active casts a copy of its damage on the next
/// target, and each copy spends a charge.
#[test]
fn sweeping_strikes_copies_a_hit_to_the_next_target() {
    let logs = first_fight_log(warrior_fixture("production-arms-warrior-2-targets"));
    assert!(lines_with(&logs, 2, "12723}").len() > 3, "{logs}");
    assert!(logs.contains("{SpellID: 12723} stacks: 5 --> 4"));
}

/// Sweeping Strikes strikes "an additional nearby opponent", so with one target Go does not
/// let Thunder Clap spend a charge on a normalized attack against the target it already hit
/// (#667), though a rotation may still cast the aura. The clap still lands.
#[test]
fn sweeping_strikes_does_nothing_against_one_target() {
    let logs = first_fight_log(warrior_fixture("arms-warrior-thunder-clap-1-target"));
    assert!(lines_with(&logs, 1, "12723, Tag: 1}").is_empty(), "{logs}");
    assert!(logs.contains("[Target 1] Aura gained: {SpellID: 11581}"));
}

/// A multidot line puts Rend, and the Deep Wounds a crit of its tick starts, on each target
/// whose dot is down.
#[test]
fn a_multidot_line_puts_rend_on_every_target() {
    let logs = first_fight_log(warrior_fixture("arms-warrior-multidot-rend-3-targets"));
    for target in 1..=3 {
        assert!(
            !lines_with(&logs, target, "11574} tick").is_empty(),
            "Target {target}: no Rend tick"
        );
    }
}

/// Every copy of the boss swings at a tank in Go, each on its own timer, and so does each in
/// Rust: every target casts its auto attack at the player and the player's listeners hear all
/// of them. Each copy's own Thunder Clap slow moves its own timer.
#[test]
fn every_copy_of_the_boss_swings_at_the_tank() {
    let logs = first_fight_log(warrior_fixture("production-protection-warrior-3-targets"));
    for target in 1..=3 {
        let swing = format!("[Target {target}] Casting {{OtherID: 3, Tag: 1}}");
        assert!(
            logs.matches(&swing).count() > 10,
            "Target {target}: it does not swing"
        );
        let hit = format!("[Target {target}] [protection-warrior (#1)] {{OtherID: 3, Tag: 1}}");
        assert!(logs.contains(&hit), "Target {target}: it never lands a hit");
    }
    // The player's Revenge answers a dodge, parry or block from any of them.
    assert!(logs.contains("Casting {SpellID: 25288}"));
}

/// Retaliation's strike goes back at the copy whose melee hit spent the charge.
#[test]
fn retaliation_strikes_the_copy_that_hit() {
    let logs = first_fight_log(warrior_fixture("protection-warrior-own-sunder-3-targets"));
    for target in 1..=3 {
        assert!(
            !lines_with(&logs, target, "20240").is_empty(),
            "Target {target}: no Retaliation strike"
        );
    }
}

/// The times a target's swings at the player start, in seconds.
fn swing_times(logs: &str, target: u32) -> Vec<f64> {
    logs.lines()
        .filter(|line| line.contains(&format!("[Target {target}] Casting {{OtherID: 3, Tag: 1}}")))
        .map(|line| {
            line[1..line.find(']').unwrap()]
                .parse()
                .expect("a time stamp")
        })
        .collect()
}

/// Thunder Clap slows each target it lands on, up to the row's four: that copy's swings come
/// a fifth further apart while the debuff holds, and the fifth target keeps its pace, as Go
/// moves each copy's melee speed on its own.
#[test]
fn thunder_clap_slows_only_the_copies_it_lands_on() {
    let logs = first_fight_log(warrior_fixture("production-protection-warrior-5-targets"));
    // A parry shortens one interval, so look at the longest the target ever waits.
    let longest = |target: u32| {
        let times = swing_times(&logs, target);
        times
            .windows(2)
            .map(|pair| ((pair[1] - pair[0]) * 100.0).round() / 100.0)
            .fold(0.0, f64::max)
    };
    for target in 1..=4 {
        assert!(
            logs.contains(&format!(
                "[Target {target}] Aura gained: {{SpellID: 11581}}"
            )),
            "Target {target}: not clapped"
        );
        assert_eq!(longest(target), 2.4, "Target {target}: not slowed");
    }
    assert!(!logs.contains("[Target 5] Aura gained: {SpellID: 11581}"));
    assert_eq!(longest(5), 2.0, "Target 5: slowed");
}

/// Challenging Shout, which Go casts on every target, has no behavior in Rust, and against
/// several targets the gate says what it would do there. Demoralizing Shout runs.
#[test]
fn challenging_shout_is_refused_by_name_against_several_targets() {
    let mut value = warrior_fixture("production-warrior-2-targets");
    let rotation = value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    rotation[0]["action"] = json!({"castSpell": {"spellId": {"spellId": 1161}}});
    let reasons = reasons(value);
    assert!(
        reasons.contains(
            &"rotation reaches spell 1161, which taunts every target in a fight against several targets"
                .to_string()
        ),
        "{reasons:?}"
    );
    assert!(reasons.contains(&"rotation reaches spell 1161 without a known behavior".to_string()));
}

/// Demoralizing Shout rolls a magic hit on every target in unit index order and debuffs each
/// it lands on, and each copy of the boss swings at the tank with the attack power its own
/// debuff leaves it, as Go reads it from the unit that swings.
#[test]
fn demoralizing_shout_debuffs_every_target_and_cuts_each_swing() {
    let logs = first_fight_log(warrior_fixture(
        "protection-warrior-demoralizing-shout-5-targets",
    ));
    for target in 1..=5 {
        let cast = lines_with(&logs, target, "11556");
        assert!(!cast.is_empty(), "Target {target}: no shout");
        let gained = logs
            .find(&format!(
                "[Target {target}] Aura gained: {{SpellID: 11556}}"
            ))
            .unwrap_or_else(|| panic!("Target {target}: no debuff"));
        let swing = format!(
            "[Target {target}] [protection-warrior (#1)] {{OtherID: 3, Tag: 1}} [DEBUG] MAP: 600.0"
        );
        assert!(
            logs[gained..].contains(&swing),
            "Target {target}: no swing under the debuff"
        );
        assert!(
            !logs[..gained].contains(&swing),
            "Target {target}: a swing under a debuff it did not hold yet"
        );
    }
}

/// A raid's permanent debuff in the Demoralizing category bids as much as the shout and
/// outlasts it, so the category holds the player's shout off: it is cast and rolled, and its
/// aura is never gained.
#[test]
fn a_permanent_debuff_holds_the_shout_off() {
    for case in [
        "protection-warrior-demoralizing-shout-over-roar-debuff",
        "protection-warrior-demoralizing-shout-over-shout-debuff",
    ] {
        let logs = first_fight_log(warrior_fixture(case));
        assert!(
            !lines_with(&logs, 1, "11556").is_empty(),
            "{case}: no shout"
        );
        assert!(
            !logs.contains("Aura gained: {SpellID: 11556}"),
            "{case}: the shout's aura was gained"
        );
    }
}

/// The shout runs only with its effect.
#[test]
fn demoralizing_shout_needs_its_effect() {
    let mut value = warrior_fixture("fury-warrior-demoralizing-shout");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "demoralizing_shout");
    assert!(reasons(value)
        .contains(&"rotation reaches spell 11556 without a known behavior".to_string()));
}

/// An extra attack that a landed hit casts at once, replaced by a queued Cleave, starts a
/// Cleave while another still deals its hits. Go's Cleave shares one slice of results between
/// its casts, so the first cast then deals the second's last hit again in place of its own,
/// whose result is never dealt.
#[test]
fn a_cleave_cast_again_while_it_deals_deals_the_later_hit_twice() {
    for name in [
        "production-arms-warrior-2-targets",
        "production-warrior-5-targets",
    ] {
        let logs = first_fight_log(warrior_fixture(name));
        let lines: Vec<&str> = logs.lines().collect();
        let dealt_twice = lines.iter().enumerate().any(|(index, line)| {
            line.contains("{SpellID: 20569} ")
                && line.contains(" for ")
                && !line.contains("[DEBUG]")
                && lines[index.saturating_sub(40)..index].contains(line)
        });
        assert!(dealt_twice, "{name}");
    }
}

/// The lines of a first-fight log at one timestamp.
fn lines_at<'a>(logs: &'a str, time: &str) -> Vec<&'a str> {
    let prefix = format!("[{time}] ");
    logs.lines()
        .filter(|line| line.starts_with(&prefix))
        .collect()
}

fn has_line(logs: &str, time: &str, needle: &str) -> bool {
    lines_at(logs, time)
        .iter()
        .any(|line| line.contains(needle))
}

/// A prepull move runs unchecked, the Movement aura's stacks count the yards, and Charge
/// triples the speed until the run ends, which also ends the dash aura, all as Go logs it. The
/// swings start once the run has put the warrior in range, before the pull.
#[test]
fn a_prepull_charge_runs_to_the_target() {
    let logs = first_fight_log(warrior_fixture("production-warrior-prepull-charge"));
    assert!(has_line(&logs, "-6.00", "[DEBUG] Moving to 25.0 yards"));
    assert!(has_line(&logs, "-6.00", "{OtherID: 20} stacks: 0 --> 1"));
    // 25 yards at 7 yards a second take 3.571 seconds.
    assert!(has_line(&logs, "-2.43", "{OtherID: 20} stacks: 1 --> 25"));
    assert!(has_line(&logs, "-2.43", "Aura faded: {OtherID: 20}"));
    assert!(has_line(&logs, "-1.00", "Casting {SpellID: 11578}"));
    assert!(has_line(
        &logs,
        "-1.00",
        "[DEBUG] Movement speed changed from 7.00 (0.00%) to 21.00 (200.00%)"
    ));
    assert!(has_line(
        &logs,
        "-1.00",
        "Gained 18.000 rage from {SpellID: 11578}"
    ));
    // The run to 4.5 yards takes 0.976 seconds at the dash speed, and ends the aura.
    assert!(has_line(&logs, "-0.02", "{OtherID: 20} stacks: 4 --> 0"));
    assert!(has_line(&logs, "-0.02", "Aura faded: {SpellID: 11578}"));
    assert!(has_line(
        &logs,
        "-0.02",
        "[DEBUG] Movement speed changed from 21.00 (200.00%) to 7.00 (0.00%)"
    ));
    assert!(has_line(&logs, "0.00", "Casting {OtherID: 3, Tag: 1}"));
}

/// Go reads a unit's position only when the rotation runs or a move starts, so a Charge cast
/// while the first move is still running checks the range the move started from and fails
/// silently. The warrior stays out of melee range and never swings.
#[test]
fn charge_cast_while_moving_checks_the_old_range() {
    let logs = first_fight_log(warrior_fixture(
        "production-warrior-prepull-charge-while-moving",
    ));
    assert!(logs.contains("[DEBUG] Moving to 25.0 yards"));
    assert!(!logs.contains("Casting {SpellID: 11578}"));
    assert!(!logs.contains("Movement speed changed"));
    assert!(!logs.contains("Casting {OtherID: 3"));
}

/// A second move starts from the position the first has reached and replaces it, without
/// casting the Movement spell again.
#[test]
fn a_second_prepull_move_replaces_the_first() {
    let logs = first_fight_log(warrior_fixture("production-warrior-prepull-move-cancelled"));
    assert_eq!(logs.matches("Casting {OtherID: 20}").count(), 1);
    assert_eq!(logs.matches("[DEBUG] Moving to").count(), 2);
}

/// A warrior still moving at the pull cannot start a cast with a cast time, and its swings wait
/// for a range it never reaches.
#[test]
fn a_warrior_moving_at_the_pull_cannot_slam() {
    let logs = first_fight_log(warrior_fixture(
        "production-arms-warrior-prepull-move-at-the-pull",
    ));
    assert!(logs.contains("[DEBUG] Moving to 40.0 yards"));
    assert!(!logs.contains("Casting {SpellID: 11605"));
    assert!(!logs.contains("Casting {OtherID: 3"));
}

/// Go drops a prepull action after the pull, a hidden one and one whose condition is constant
/// false, so none of the moves runs.
#[test]
fn prepull_moves_go_drops_never_run() {
    let logs = first_fight_log(warrior_fixture("production-warrior-prepull-move-dropped"));
    assert!(!logs.contains("Moving to"));
    assert!(!logs.contains("Casting {OtherID: 20}"));
}

/// Charge needs the movement speed and its own effect.
#[test]
fn charge_needs_its_effect_and_the_movement_speed() {
    let mut value = warrior_fixture("production-warrior-prepull-charge");
    effect_mut(&mut value, "player_movement")["speed_auras"] = json!(["Unholy Aura"]);
    assert_eq!(
        reasons(value.clone()),
        ["a prepull move with Unholy Aura, which changes the movement speed, is unsupported"]
    );
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "player_movement");
    assert_eq!(
        reasons(value),
        ["a prepull move has no exported movement speed"]
    );
    let mut value = warrior_fixture("production-warrior-prepull-charge");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "warrior_charge");
    assert_eq!(
        reasons(value),
        ["rotation reaches spell 11578 without a known behavior"]
    );
}

/// A move that stops a ranged auto swing is not simulated, and the refusals of another class
/// and of a speed aura are accepted fixtures.
#[test]
fn a_prepull_move_is_refused_where_it_is_not_simulated() {
    let mut value = warrior_fixture("production-warrior-prepull-charge");
    value["melee"]["auto_swing_ranged"] = json!(true);
    assert!(reasons(value)
        .contains(&"a prepull move with a ranged auto swing is unsupported".to_string()));
    let rogue = warrior_fixture("production-assassination-rogue-prepull-move");
    assert!(crate::refusal_codes(rogue).contains(&(
        "prepull_unsupported",
        "a prepull move is unsupported for ClassRogue".into()
    )));
    let unholy = warrior_fixture("production-arms-warrior-prepull-move-with-unholy-aura");
    assert!(reasons(unholy).contains(
        &"a prepull move with Unholy Aura, which changes the movement speed, is unsupported".into()
    ));
}

/// The Diamond Flask is a five second channel cast before the pull: its self hot ticks each
/// second and the last tick, at the pull, activates a Strength aura for a minute.
#[test]
fn the_diamond_flask_channels_then_grants_strength() {
    let logs = first_fight_log(warrior_fixture("production-warrior-diamond-flask"));
    assert!(has_line(&logs, "-5.00", "Casting {ItemID: 20130}"));
    assert!(has_line(&logs, "-5.00", "Aura gained: {ItemID: 20130}"));
    assert!(has_line(&logs, "0.00", "Aura faded: {ItemID: 20130}"));
    assert!(has_line(&logs, "0.00", "Aura gained: {SpellID: 1318070}"));
    assert!(has_line(
        &logs,
        "0.00",
        "Gained {\"Strength\": 20.000,} from {SpellID: 1318070}."
    ));
    assert!(has_line(&logs, "60.00", "Aura faded: {SpellID: 1318070}"));
}

/// The flask's major cooldown never activates on its own, so without a cast in the rotation or
/// the prepull the autocast leaves it alone.
#[test]
fn the_diamond_flask_is_never_autocast() {
    let mut value = warrior_fixture("production-warrior-diamond-flask");
    value["player"]["rotation"]["prepullActions"] = json!([]);
    value["player"]["prepull_actions"] = json!(0);
    let logs = first_fight_log(value);
    assert!(!logs.contains("ItemID: 20130"));
    assert!(!logs.contains("1318070"));
}

/// A rotation that casts the flask itself takes it from the major cooldowns, so the exporter
/// describes it from the spellbook; the cast begins when the rotation reaches it.
#[test]
fn a_flask_the_rotation_casts_is_described() {
    let logs = first_fight_log(warrior_fixture(
        "production-warrior-diamond-flask-in-rotation",
    ));
    let cast = logs
        .lines()
        .find(|line| line.contains("Casting {ItemID: 20130}"))
        .unwrap();
    let time: f64 = cast[1..cast.find(']').unwrap()].parse().unwrap();
    assert!(time >= 10.0, "{cast}");
    assert!(logs.contains("Aura gained: {SpellID: 1318070}"));
}

/// Without its effect the flask has no behavior, whichever way the rotation reaches it.
#[test]
fn the_diamond_flask_needs_its_effect() {
    let mut value = warrior_fixture("production-warrior-diamond-flask");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "diamond_flask");
    assert!(reasons(value).contains(&"rotation reaches item 20130 without a known behavior".into()));
}

/// The Frost hit of Iceblade Hacker and Warblade of Caer Darrow (shared.NewProcDamageEffect with
/// the melee defense type): the effect hears the melee hits of the hand holding the weapon, at a
/// fixed chance of 1, and its spell is of the melee defense type.
#[test]
fn a_melee_weapon_damage_proc_hears_the_hits_of_its_own_hand() {
    let main = &["ProcMaskMeleeMHAuto", "ProcMaskMeleeMHSpecial"][..];
    let off = &["ProcMaskMeleeOHAuto", "ProcMaskMeleeOHSpecial"][..];
    let both = &[
        "ProcMaskMeleeMHAuto",
        "ProcMaskMeleeMHSpecial",
        "ProcMaskMeleeOHAuto",
        "ProcMaskMeleeOHSpecial",
    ][..];
    for (name, aura, hands) in [
        ("warrior-iceblade-hacker", "Iceblade Hacker", main),
        (
            "arms-warrior-warblade-of-caer-darrow",
            "Warblade of Caer Darrow",
            main,
        ),
        (
            "warrior-warblade-of-caer-darrow-off-hand",
            "Warblade of Caer Darrow",
            off,
        ),
        (
            "warrior-warblade-of-caer-darrow-both-hands",
            "Warblade of Caer Darrow",
            both,
        ),
    ] {
        let value = warrior_fixture(name);
        let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(prepared_coverage(&prepared), Vec::<String>::new(), "{name}");
        let effect = value["effects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|effect| effect["trigger_aura"] == aura)
            .unwrap();
        assert_eq!(effect["outcome"], "melee_special_hit_and_crit", "{name}");
        let spells = value["player"]["spells"].as_array().unwrap();
        let masks_of = |position: &Value| -> Vec<&str> {
            spells[position.as_u64().unwrap() as usize]["proc_mask"]
                .as_array()
                .unwrap()
                .iter()
                .map(|mask| mask.as_str().unwrap())
                .collect()
        };
        assert_eq!(
            spells[effect["spell"].as_u64().unwrap() as usize]["defense_type"],
            "DefenseTypeMelee",
            "{name}"
        );
        // Every spell the manager rolls for carries a mask of a wielding hand, as a Whirlwind
        // that swings both hands does; the other hand's spells are not rolled for.
        let chances = effect["chances"].as_array().unwrap();
        assert!(!chances.is_empty(), "{name}");
        for chance in chances {
            assert_eq!(chance["chance"], 1.0, "{name}");
            let masks = masks_of(&chance["spell"]);
            assert!(
                masks.iter().any(|mask| hands.contains(mask)),
                "{name}: {masks:?}"
            );
        }
        // Each hand holding the weapon has a spell the manager rolls for.
        for hand in hands {
            assert!(
                chances
                    .iter()
                    .any(|chance| masks_of(&chance["spell"]).contains(hand)),
                "{name}: {hand}"
            );
        }
        // A spell the trigger hears that the manager does not roll for swings only the other hand.
        let rolled: Vec<u64> = chances
            .iter()
            .map(|chance| chance["spell"].as_u64().unwrap())
            .collect();
        for heard in effect["trigger_spells"].as_array().unwrap() {
            if !rolled.contains(&heard.as_u64().unwrap()) {
                let masks = masks_of(heard);
                assert!(
                    !masks.iter().any(|mask| hands.contains(mask)),
                    "{name}: {masks:?}"
                );
            }
        }
    }
}

/// Without its effect the aura listens to hits with nothing that handles them.
#[test]
fn a_melee_weapon_damage_proc_needs_its_effect() {
    let mut value = warrior_fixture("warrior-iceblade-hacker");
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["trigger_aura"] != "Iceblade Hacker");
    assert!(reasons(value).contains(
        &"player aura \"Iceblade Hacker\" listens to combat events without an effect".into()
    ));
}

/// The runtime rolls the melee special table, with or without the crit, for a literal range of
/// a melee spell, and nothing else.
#[test]
fn a_melee_weapon_damage_proc_names_a_table_the_runtime_rolls() {
    let refused = |value: Value| {
        crate::refusal_codes(value)
            .into_iter()
            .filter(|(code, _)| *code == "proc_unsupported")
            .map(|(_, reason)| reason)
            .collect::<Vec<_>>()
    };
    let hit = |outcome: &str| {
        format!(
            "Iceblade Hacker's hit rolls the {outcome} table, which the runtime rolls only for a \
             melee spell's literal range"
        )
    };
    let position = |value: &Value| -> usize {
        value["effects"]
            .as_array()
            .unwrap()
            .iter()
            .position(|effect| effect["trigger_aura"] == "Iceblade Hacker")
            .unwrap()
    };
    let original = warrior_fixture("warrior-iceblade-hacker");
    assert!(refused(original.clone()).is_empty());

    // A table the runtime does not know.
    let mut value = original.clone();
    let index = position(&value);
    value["effects"][index]["outcome"] = json!("melee_special_block_and_crit");
    assert_eq!(refused(value), [hit("melee_special_block_and_crit")]);

    // A table that disagrees with the spell's crit: Go picks the no crit table only for a spell
    // the client bars from critting.
    let mut value = original.clone();
    let index = position(&value);
    value["effects"][index]["outcome"] = json!("melee_special_hit");
    assert_eq!(refused(value.clone()), [hit("melee_special_hit")]);
    value["effects"][index]["can_crit"] = json!(false);
    assert!(refused(value).is_empty());

    // Without its literal range the hit rolls the client effect, which here deals nothing.
    let mut value = original.clone();
    let index = position(&value);
    value["effects"][index]
        .as_object_mut()
        .unwrap()
        .remove("roll");
    assert_eq!(
        refused(value),
        ["Iceblade Hacker's hit is not rolled: a client effect that deals nothing"]
    );

    // A spell of another defense type would take the wrong critical strike multiplier.
    let mut value = original;
    let index = position(&value);
    let spell = value["effects"][index]["spell"].as_u64().unwrap() as usize;
    value["player"]["spells"][spell]["defense_type"] = json!("DefenseTypeMagic");
    assert_eq!(refused(value), [hit("melee_special_hit_and_crit")]);
}

/// The hit takes the melee table where a magic hit never dodges: it dodges, crits for twice its
/// base damage, and a Frost hit still takes the partial resist roll.
#[test]
fn a_melee_weapon_damage_proc_rolls_the_melee_table_in_a_fight() {
    let logs = first_fight_log(warrior_fixture("warrior-iceblade-hacker"));
    let lines = lines_with(&logs, 1, "1298414");
    for expected in [
        "} Dodge (",
        "} Crit for 81.400 damage",
        "} Hit for 40.700 damage",
        "} Crit (25% Resist) for 61.050 damage",
    ] {
        assert!(
            lines.iter().any(|line| line.contains(expected)),
            "{expected}"
        );
    }
}
