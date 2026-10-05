//! Hunter prepared v2 regressions. The Go goldens of the production Hunters are
//! compared with the rest of the fixture family in `mage/prepared_v2.rs`; these tests pin the
//! ranged auto attack, the ranged cast time, physical ticks, the pets and what the gate rejects.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, simulate_prepared, PreparedError,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn production() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-marksmanship-hunter.prepared.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn parse(value: Value) -> PreparedV2 {
    serde_json::from_value(value).unwrap()
}

fn reasons(value: Value) -> Vec<String> {
    match check_prepared(&parse(value)) {
        Err(PreparedError::Unsupported(reasons)) => reasons,
        other => panic!("expected unsupported, got {other:?}"),
    }
}

/// The first fight's log of a short run.
fn first_fight_log(mut value: Value) -> String {
    value["sim"]["iterations"] = json!(1);
    value["sim"]["debug_first_iteration"] = json!(true);
    let report = simulate_prepared(&parse(value)).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

fn player_line<'a>(logs: &'a str, at: &str, text: &str) -> Option<&'a str> {
    logs.lines().find(|line| {
        line.starts_with(at) && line.contains("[marksmanship-hunter (#1)]") && line.contains(text)
    })
}

#[test]
fn production_marksmanship_hunter_is_supported() {
    assert!(check_prepared(&parse(production())).is_ok());
}

/// Go attack.go: the first Auto Shot fires at the pull and lands after travel, 12 yards at
/// missile speed 40.
#[test]
fn auto_shot_fires_at_the_pull_and_lands_after_travel() {
    let logs = first_fight_log(production());
    assert!(
        player_line(&logs, "[0.00]", "Casting {OtherID: 4}").is_some(),
        "{logs}"
    );
    assert!(
        player_line(&logs, "[0.30]", "{OtherID: 4} Hit for").is_some()
            || player_line(&logs, "[0.30]", "{OtherID: 4} Crit for").is_some()
            || player_line(&logs, "[0.30]", "{OtherID: 4} Miss").is_some(),
        "{logs}"
    );
}

/// A ranged weapon out of range never starts its swing.
#[test]
fn a_ranged_weapon_out_of_range_never_shoots() {
    let mut value = production();
    value["melee"]["ranged"]["max_range"] = json!(10.0);
    let logs = first_fight_log(value);
    assert!(!logs.contains("{OtherID: 4}"), "{logs}");
}

/// Go hunter.go RegisterRangedSpell: Aimed Shot's two second cast divides by the ranged haste
/// multiplier, the quiver's 15%, unrounded.
#[test]
fn aimed_shot_casts_over_the_ranged_hasted_time() {
    let logs = first_fight_log(production());
    let line =
        player_line(&logs, "[-2.50]", "Casting {SpellID: 20904}").expect("prepull Aimed Shot");
    assert!(line.contains("Cast Time = 1.739130434s"), "{line}");
}

/// Go spell_resistances.go: a hawk's physical tick ignores armor.
#[test]
fn a_hawk_tick_ignores_armor() {
    let logs = first_fight_log(production());
    let line = logs
        .lines()
        .find(|line| line.contains("{SpellID: 1293527, Tag: 1} [DEBUG]"))
        .expect("a hawk tick");
    let stage = |name: &str| {
        let start = line.find(name).unwrap() + name.len();
        line[start..].split(',').next().unwrap().to_string()
    };
    assert_eq!(
        stage("AfterAttackerMods:"),
        stage("AfterResistances:"),
        "{line}"
    );
}

#[test]
fn an_unknown_auto_attack_type_is_rejected() {
    let mut value = production();
    let list = value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    list[1]["action"]["condition"]["and"]["vals"][0]["cmp"]["lhs"]["autoTimeToNext"]["autoType"] =
        json!("SomeAuto");
    let reasons = reasons(value);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("autoTimeToNext autoType")),
        "{reasons:?}"
    );
}

#[test]
fn a_magic_hit_sting_tick_is_rejected() {
    let mut value = production();
    for effect in value["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "serpent_sting" {
            effect["tick_outcome"] = json!("magic_hit");
        }
    }
    assert_eq!(reasons(value), ["Serpent Sting ticks with magic_hit"]);
}

#[test]
fn a_replaceable_main_hand_swing_in_range_is_rejected() {
    let mut value = production();
    value["unrepresented"] = json!(["main hand swings can be replaced"]);
    assert_eq!(
        reasons(value),
        ["unrepresented by the exporter: main hand swings can be replaced"]
    );
}

/// In melee range the main hand swings, Go's Raptor Strike replacement keeping each swing
/// while the rotation never queues one, and the ranged weapon's minimum range stops Auto Shot.
#[test]
fn in_melee_range_the_main_hand_swings_instead_of_auto_shot() {
    let mut value = production();
    value["player"]["distance_yards"] = json!(5.0);
    assert!(check_prepared(&parse(value.clone())).is_ok());
    let logs = first_fight_log(value);
    assert!(logs.contains("Casting {OtherID: 3, Tag: 1}"), "{logs}");
    assert!(!logs.contains("Casting {OtherID: 4}"), "{logs}");
}

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{name}.prepared.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The lines of one unit's log, by its label.
fn unit_lines<'a>(logs: &'a str, unit: &str) -> Vec<&'a str> {
    let tag = format!("[{unit}]");
    logs.lines().filter(|line| line.contains(&tag)).collect()
}

/// A log line's timestamp in seconds.
fn at(line: &str) -> f64 {
    line[1..line.find(']').unwrap()].parse().unwrap()
}

const WHELP: &str = "survival-hunter (#1) - Emerald Dragon Whelp";

#[test]
fn production_pet_and_melee_hunters_are_supported() {
    for name in [
        "production-beast-mastery-hunter",
        "production-survival-hunter",
        "survival-hunter-no-whelp",
    ] {
        assert!(check_prepared(&parse(fixture(name))).is_ok(), "{name}");
    }
}

/// Go pet.go: the Cat starts at its owner's 12 yards and moves in, one yard a stack.
#[test]
fn the_cat_moves_in_from_its_owners_distance() {
    let logs = first_fight_log(fixture("production-beast-mastery-hunter"));
    let cat = unit_lines(&logs, "hunter (#1) - Cat");
    assert!(
        cat.iter()
            .any(|line| line.starts_with("[0.00]") && line.contains("stacks: 0 --> 12")),
        "{logs}"
    );
}

/// Go emerald_dragon_whelp.go: each reset dismisses the whelp; Dragon's Call summons it,
/// it hardcasts Acid Spit, and its timeout dismisses it 15 seconds later.
#[test]
fn dragons_call_summons_the_whelp_for_fifteen_seconds() {
    let logs = first_fight_log(fixture("production-survival-hunter"));
    let whelp = unit_lines(&logs, WHELP);
    assert!(whelp[0].starts_with("[0.00]") && whelp[0].ends_with("Pet dismissed"));
    let summoned = whelp
        .iter()
        .position(|line| line.ends_with("Pet summoned"))
        .expect("Dragon's Call procs in the first fight");
    let dismissed = whelp[summoned..]
        .iter()
        .find(|line| line.ends_with("Pet dismissed"))
        .expect("the timeout dismisses the whelp");
    assert!(
        (at(dismissed) - at(whelp[summoned]) - 15.0).abs() < 1e-9,
        "{logs}"
    );
    assert!(
        whelp[summoned..]
            .iter()
            .any(|line| line.contains("Casting {SpellID: 9591} (Cost = 90.000, Cast Time = 3s")),
        "{logs}"
    );
}

/// The whelp skips a spit it would not live to finish: with a two second summon it only
/// swings.
#[test]
fn the_whelp_skips_a_spit_it_would_not_finish() {
    let mut value = fixture("production-survival-hunter");
    for effect in value["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "emerald_dragon_whelp" {
            effect["duration_ns"] = json!(2_000_000_000i64);
        }
    }
    let logs = first_fight_log(value);
    let whelp = unit_lines(&logs, WHELP);
    assert!(
        whelp.iter().any(|line| line.ends_with("Pet summoned")),
        "{logs}"
    );
    assert!(whelp
        .iter()
        .any(|line| line.contains("Casting {OtherID: 3, Tag: 1}")));
    assert!(
        !whelp.iter().any(|line| line.contains("{SpellID: 9591}")),
        "{logs}"
    );
}

/// Dragon's Call needs the simulated pet to be the guardian it summons.
#[test]
fn dragons_call_without_its_summoned_guardian_is_refused() {
    let mut value = fixture("production-survival-hunter");
    value["pets"][0]["summoned"] = json!(false);
    let error = simulate_prepared(&parse(value)).unwrap_err().to_string();
    assert!(error.contains("is not a simulated summoned pet"), "{error}");
}

/// Go pet.go ExecuteCustomRotation: once less of the fight remains than the pet's uptime
/// allows, its rotation disables it; it acts no more, and later evaluations only log so.
#[test]
fn a_pet_past_its_uptime_is_disabled() {
    let logs = first_fight_log(fixture("beast-mastery-hunter-pet-uptime-half"));
    let end = logs
        .lines()
        .filter_map(|line| line.get(1..).and_then(|rest| rest.split(']').next()))
        .filter_map(|t| t.parse::<f64>().ok())
        .fold(0.0, f64::max);
    let cat = unit_lines(&logs, "hunter (#1) - Cat");
    let dismissed = cat
        .iter()
        .position(|line| line.ends_with("Pet dismissed"))
        .expect("the Cat is disabled");
    let at_dismissal = at(cat[dismissed]);
    assert!(
        at_dismissal > end / 2.0 && at_dismissal < end / 2.0 + 0.5,
        "{at_dismissal} of {end}"
    );
    assert!(
        !cat[dismissed..].iter().any(|line| line.contains("Casting")),
        "{logs}"
    );
    assert!(cat[dismissed + 1..]
        .iter()
        .any(|line| line.ends_with("No pet summoned")));
}

/// Go pet_abilities.go newScorpidPoison: Apply's deactivation drops the stack, so each landed
/// cast is one stack again, and the Scorpid's rotation never reaches Claw.
#[test]
fn scorpid_poison_is_one_stack_a_cast() {
    let logs = first_fight_log(fixture("beast-mastery-hunter-scorpid"));
    assert!(logs.contains("{SpellID: 24587} stacks: 0 --> 1"), "{logs}");
    assert!(!logs.contains("{SpellID: 24587} stacks: 1 --> 2"), "{logs}");
    assert!(!logs.contains("Casting {SpellID: 3009}"), "{logs}");
}

/// Go newDustCloud: the pet casts it only while its target aura is down.
#[test]
fn dust_cloud_is_cast_again_only_once_it_falls_off() {
    let logs = first_fight_log(fixture("beast-mastery-hunter-tallstrider"));
    let events: Vec<&str> = logs
        .lines()
        .filter(|line| {
            line.contains("Casting {SpellID: 1265904}")
                || line.contains("Aura gained: {SpellID: 1265904}")
                || line.contains("Aura faded: {SpellID: 1265904}")
        })
        .collect();
    assert!(events.len() > 3, "{logs}");
    let mut active = false;
    for line in events {
        if line.contains("Casting") {
            assert!(!active, "{line}");
        } else {
            active = line.contains("gained");
        }
    }
}

/// Go talents_marksmanship.go registerRapidRecuperation: a landed Serpent Sting grants the
/// casting regeneration a spell batch window later.
#[test]
fn rapid_recuperation_follows_a_landed_serpent_sting() {
    let logs = first_fight_log(fixture("marksmanship-hunter-rapid-recuperation"));
    let sting = logs
        .lines()
        .find(|line| line.contains("{SpellID: 25295} Hit for"))
        .expect("Serpent Sting lands");
    let gained = logs
        .lines()
        .find(|line| line.contains("Aura gained: {SpellID: 1242512}"))
        .expect("Rapid Recuperation procs");
    assert!((at(gained) - at(sting) - 0.01).abs() < 1e-9, "{logs}");
}

/// Go arcane_shot.go: an Arcane school hit on the ranged table that lands after travel.
#[test]
fn arcane_shot_lands_after_travel() {
    let logs = first_fight_log(fixture("marksmanship-hunter-arcane-shot"));
    let cast = logs
        .lines()
        .find(|line| line.contains("Casting {SpellID: 14287}"))
        .expect("Arcane Shot is cast");
    let hit = logs
        .lines()
        .find(|line| {
            line.contains("{SpellID: 14287} Hit for") || line.contains("{SpellID: 14287} Crit for")
        })
        .expect("Arcane Shot hits");
    assert!(hit.contains("(SpellSchool: 64)"), "{hit}");
    assert!(at(hit) > at(cast), "{cast} {hit}");
}

/// Go newSwipe: its cast condition needs three active targets, so a Bear never swipes one.
#[test]
fn a_bear_never_swipes_one_target() {
    let logs = first_fight_log(fixture("beast-mastery-hunter-bear"));
    assert!(logs.contains("Casting {SpellID: 3009}"), "{logs}");
    assert!(!logs.contains("Casting {SpellID: 1264502}"), "{logs}");
}

/// Go items.go Renataki's Charm of Beasts: once the prepull Aimed Shot is cooling, the charm
/// resets it and Aimed Shot is cast again at once.
#[test]
fn renatakis_charm_resets_the_aimed_shot_cooldown() {
    let logs = first_fight_log(fixture("marksmanship-hunter-predators-armor"));
    let lines: Vec<&str> = logs.lines().collect();
    let charm = lines
        .iter()
        .position(|line| line.contains("Major cooldown used: {ItemID: 19953}"))
        .expect("the charm is used");
    assert!(
        lines[charm + 1].contains("Casting {SpellID: 20904}")
            && at(lines[charm + 1]) == at(lines[charm]),
        "{logs}"
    );
}

/// Go item_sets.go Cryptstalker Armor (6): a ranged crit restores 50 mana a spell batch window
/// later, and the set bonus trackers it exposes to the APL list no aura metrics.
#[test]
fn cryptstalker_ranged_crits_restore_mana() {
    let value = fixture("marksmanship-hunter-cryptstalker-armor");
    let logs = first_fight_log(value.clone());
    let lines: Vec<&str> = logs.lines().collect();
    let gained = lines
        .iter()
        .position(|line| line.contains("Gained 50.000 mana from {SpellID: 28753}"))
        .expect("a ranged crit restores mana");
    let crit = lines[..gained]
        .iter()
        .rev()
        .find(|line| line.contains("{OtherID: 4} Crit for"))
        .expect("a ranged crit before it");
    assert!((at(lines[gained]) - at(crit) - 0.01).abs() < 1e-9, "{logs}");
    let report = simulate_prepared(&parse(value)).unwrap();
    let auras = report.result["raidMetrics"]["parties"][0]["players"][0]["auras"].to_string();
    assert!(!auras.contains("28755"), "{auras}");
}

/// Two simulated pets: the Cat enabled at each reset and Dragon's Call's whelp summoned
/// mid-fight each swing as their own unit, and each reports its own metrics.
#[test]
fn a_cat_and_the_whelp_act_as_two_units() {
    let value = fixture("survival-hunter-cat-and-whelp");
    let logs = first_fight_log(value.clone());
    for unit in ["survival-hunter (#1) - Cat", WHELP] {
        assert!(
            unit_lines(&logs, unit)
                .iter()
                .any(|line| line.contains("Casting {OtherID: 3, Tag: 1}")),
            "{unit} swings: {logs}"
        );
    }
    let report = simulate_prepared(&parse(value)).unwrap();
    let pets = &report.result["raidMetrics"]["parties"][0]["players"][0]["pets"];
    let names: Vec<&str> = pets
        .as_array()
        .unwrap()
        .iter()
        .map(|pet| pet["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Cat", "Emerald Dragon Whelp"]);
}

fn survival_against(targets: u32) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/mage/prepared-v2/production-survival-hunter-{targets}-targets.prepared.json"
    ));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// Against several targets the application's lines reach Explosive Trap and Volley, which
/// need the effects that describe them.
#[test]
fn area_spells_need_their_effects_against_several_targets() {
    for (kind, spell) in [("explosive_trap", 14317), ("volley", 14295)] {
        let mut value = survival_against(2);
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

/// Explosive Trap hits each target in turn, then its burn ticks on every target, and Volley's
/// channel ticks on every target, as Go traps.go and volley.go deal them.
#[test]
fn explosive_trap_and_volley_reach_every_target() {
    let logs = first_fight_log(survival_against(5));
    for target in 1..=5 {
        for (spell, what) in [(14317, " Hit"), (14317, " tick "), (14295, " tick ")] {
            let found = logs.lines().any(|line| {
                line.contains(&format!("[Target {target}] {{SpellID: {spell}}}"))
                    && line.contains(what)
            });
            assert!(found, "Target {target}: no {spell}{what}");
        }
    }
}

/// A Serpent Sting tick outcome the runtime does not implement is a Hunter class limit.
#[test]
fn an_unknown_serpent_sting_tick_is_a_class_limit() {
    let mut value = production();
    for effect in value["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "serpent_sting" {
            effect["tick_outcome"] = json!("magic_hit");
        }
    }
    assert!(crate::refusal_codes(value)
        .contains(&("class_limit", "Serpent Sting ticks with magic_hit".into())));
}
