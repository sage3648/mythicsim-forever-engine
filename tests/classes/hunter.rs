//! Hunter prepared v2 regressions. The Go goldens of the production Hunters are
//! compared with the rest of the fixture family in `mage/prepared_v2.rs`; these tests pin the
//! ranged auto attack, the ranged cast time, physical ticks, the pets and what the gate rejects.

mod prepare;

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
    // The rotation finds the uptime spent at its next evaluation, which waits on the pet's GCD
    // or focus.
    assert!(
        at_dismissal > end / 2.0 && at_dismissal < end / 2.0 + 2.0,
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

/// Go pet_abilities.go newScorpidPoison (community #694): a landed poison on a poisoned target
/// refreshes the dot and adds a stack up to five, and once the stacks are full the Scorpid's
/// rotation spends its focus on Claw. Before, Apply dropped the stack, so each cast was one
/// stack again and the rotation never reached Claw.
#[test]
fn scorpid_poison_stacks_to_five() {
    let logs = first_fight_log(fixture("beast-mastery-hunter-scorpid"));
    assert!(logs.contains("{SpellID: 24587} stacks: 0 --> 1"), "{logs}");
    assert!(logs.contains("{SpellID: 24587} stacks: 4 --> 5"), "{logs}");
    assert!(!logs.contains("{SpellID: 24587} stacks: 5 --> 6"), "{logs}");
    assert!(logs.contains("Casting {SpellID: 3009}"), "{logs}");
}

/// The Tallstrider fixture with nothing holding the target's major armor reduction category:
/// no permanent Expose Armor and no Sunder Armor ramp.
fn tallstrider_without_armor_reductions() -> Value {
    let mut value = fixture("beast-mastery-hunter-tallstrider");
    for aura in value["target"]["auras"].as_array_mut().unwrap() {
        if aura["tag"] == "MajorArmorReduction" {
            aura["active"] = json!(false);
        }
    }
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "sunder_armor_ramp");
    value
}

/// Go newDustCloud: the pet casts it only while its target aura is down (and, since patch 110,
/// no Sunder Armor or Expose Armor holds the major armor reduction category).
#[test]
fn dust_cloud_is_cast_again_only_once_it_falls_off() {
    let logs = first_fight_log(tallstrider_without_armor_reductions());
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

/// Go lacerating_strikes.go (patches 87 and 132, `TestLaceratingStrikesRefreshRollsOver`): a
/// Mongoose Bite on a running bleed rolls what the bleed still owes into the new one and keeps
/// its tick timer, as Forever's Deep Wounds does. The 21 seconds start over from the bite, the
/// next tick lands when it was due, and the owed damage joins the bite's 40%, spread over the
/// ticks that fit before the bleed runs out; a bleed fades only once every tick has landed. The
/// bleed reports under its own id, 1310536, not Mongoose Bite's with a tag.
#[test]
fn a_refreshed_lacerating_strikes_bleed_rolls_over_and_keeps_its_tick_timer() {
    const BLEED: &str = "{SpellID: 1310536}";
    let logs = first_fight_log(fixture("survival-hunter-no-whelp"));
    let amount = |line: &str| -> f64 {
        let damage = line.split(" for ").nth(1).unwrap();
        damage[..damage.find(' ').unwrap()].parse().unwrap()
    };
    // The running bleed: when its tick timer started, the ticks since, the ticks left and the
    // amount of each.
    let mut bleed: Option<(f64, i32, i32, f64)> = None;
    let end = logs
        .lines()
        .filter(|line| line.starts_with('['))
        .map(at)
        .fold(0.0, f64::max);
    let mut bite = None;
    let (mut refreshes, mut hits) = (0, 0);
    for line in logs.lines() {
        if line.contains("[Target 1] {SpellID: 14271} Hit for")
            || line.contains("[Target 1] {SpellID: 14271} Crit for")
        {
            bite = Some((at(line), amount(line)));
        } else if line.contains(&format!("[Target 1] Aura gained: {BLEED}")) {
            let (when, damage) = bite.take().expect("a bite before the bleed");
            assert_eq!(when, at(line), "{line}");
            assert!(bleed.is_none(), "{line}");
            bleed = Some((when, 0, 7, damage * 0.4 / 7.0));
        } else if line.contains(&format!("[Target 1] Aura refreshed: {BLEED}")) {
            let (when, damage) = bite.take().expect("a bite before the refresh");
            let (start, ticked, left, tick) = bleed.expect("a running bleed");
            assert_eq!(when, at(line), "{line}");
            let next = start + 3.0 * f64::from(ticked + 1);
            let fits = ((21.0 - (next - when) + 1e-9) / 3.0).floor() as i32 + 1;
            bleed = Some((
                start,
                ticked,
                fits,
                (tick * f64::from(left) + damage * 0.4) / f64::from(fits),
            ));
            refreshes += 1;
        } else if line.contains(&format!("{BLEED} tick ")) {
            let (start, ticked, left, tick) = bleed.expect("a running bleed");
            assert!(
                (at(line) - (start + 3.0 * f64::from(ticked + 1))).abs() < 1e-6,
                "{line}: off the tick timer started at {start}"
            );
            if line.contains(" tick Hit for ") {
                assert!((amount(line) - tick).abs() < 0.01, "{line}: want {tick:.3}");
                hits += 1;
            }
            bleed = Some((start, ticked + 1, left - 1, tick));
        } else if line.contains(&format!("[Target 1] Aura faded: {BLEED}")) {
            let (_, _, left, _) = bleed.take().expect("a running bleed");
            // The fight's end fades a bleed with ticks left.
            if at(line) < end {
                assert_eq!(left, 0, "{line}: the bleed faded with ticks left");
            }
        }
    }
    assert!(refreshes > 1 && hits > 7, "{logs}");
    assert!(!logs.contains("{SpellID: 14271, Tag: 1}"), "{logs}");
}

/// The seconds of the first fight at which the player's Movement aura (Go `OtherActionMove`)
/// gains and fades, in order, with the player's label.
fn movement_windows(logs: &str, player: &str) -> Vec<(f64, f64)> {
    let tag = format!("[{player}]");
    let mut windows = Vec::new();
    let mut start = None;
    for line in logs.lines().filter(|line| line.contains(&tag)) {
        if line.contains("Aura gained: {OtherID: 20}") {
            start = Some(at(line));
        } else if line.contains("Aura faded: {OtherID: 20}") {
            windows.push((start.take().expect("a gain before the fade"), at(line)));
        }
    }
    windows
}

#[test]
fn a_hunter_that_moves_in_its_rotation_is_supported() {
    for name in ["survival-hunter-weaving-moves", "marksmanship-hunter-moves"] {
        assert!(check_prepared(&parse(fixture(name))).is_ok(), "{name}");
    }
}

/// Go movement.go: the player runs at seven yards a second, so the prepull run from 5 yards
/// out to 20 takes 15 / 7 seconds, and the rotation's moves in the fight take the distance
/// they cover at that speed, 30% faster for the 3 seconds after a Strider Kick.
#[test]
fn a_move_takes_the_distance_at_seven_yards_a_second() {
    let logs = first_fight_log(fixture("survival-hunter-weaving-moves"));
    let windows = movement_windows(&logs, "survival-hunter (#1)");
    let (start, end) = windows[0];
    assert!(start < 0.0, "{windows:?}");
    assert!((end - start - 15.0 / 7.0).abs() < 0.011, "{windows:?}");
    // The last move may be cut short by the end of the fight.
    let in_fight: Vec<_> = windows[..windows.len() - 1]
        .iter()
        .filter(|(start, _)| *start >= 0.0)
        .collect();
    assert!(!in_fight.is_empty(), "{windows:?}");
    assert!(
        in_fight
            .iter()
            .all(|(start, end)| end - start > 0.75 && end - start < 2.2),
        "{windows:?}"
    );
    assert!(logs.contains("[DEBUG] Moving to 5.0 yards"), "{logs}");
    assert!(logs.contains("[DEBUG] Moving to 12.0 yards"), "{logs}");
}

/// Go attack.go `swing`: a ranged auto cannot fire while its unit moves. It looks again every
/// 500ms, so none fires inside a run, and neither does a cast with a cast time.
#[test]
fn the_ranged_auto_waits_for_a_move_to_end() {
    let logs = first_fight_log(fixture("marksmanship-hunter-moves"));
    let windows = movement_windows(&logs, "marksmanship-hunter (#1)");
    assert!(windows.len() >= 3, "{windows:?}");
    for (start, end) in windows {
        for line in logs.lines().filter(|line| {
            line.contains("[marksmanship-hunter (#1)]")
                && (line.contains("Casting {OtherID: 4}")
                    || line.contains("Casting {SpellID: 20904}"))
        }) {
            let when = at(line);
            assert!(when <= start || when >= end, "{line} inside {start}..{end}");
        }
    }
}

/// A move in the priority list is supported for every class, and needs the speed the exporter
/// read.
#[test]
fn a_rotation_move_needs_the_exported_speed() {
    let mut value: Value = serde_json::from_slice(
        &fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/mage/prepared-v2/production-warrior.prepared.json"),
        )
        .unwrap(),
    )
    .unwrap();
    value["player"]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            json!({"action": {"move": {"rangeFromTarget": {"const": {"val": "5"}}}}}),
        );
    let refusals = super::refusal_codes(value.clone());
    assert!(
        refusals.contains(&(
            "movement_unsupported",
            "a move has no exported movement speed".to_string()
        )),
        "{refusals:?}"
    );
    // The exporter writes the effect when the rotation moves.
    value["effects"].as_array_mut().unwrap().push(json!({
        "kind": "player_movement", "speed_multiplier": 1.0, "speed_auras": []
    }));
    assert!(super::refusal_codes(value).is_empty());
}

/// The auras that change the movement speed are followed through their categories, so a move
/// with one is supported.
#[test]
fn a_move_with_a_speed_aura_is_supported() {
    let mut value = fixture("survival-hunter-weaving-moves");
    let effect = value["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "player_movement")
        .expect("the exporter wrote the movement effect");
    effect["speed_auras"] = json!(["Aspect of the Cheetah"]);
    assert!(super::refusal_codes(value).is_empty());
}

/// A Volley channel is a non-melee cast: it holds the melee swing too, and the swings restart a
/// full swing after it ends (Go volley.go, community #743). The hunter's hands do not swing for
/// the six seconds of the channel, and the first swing after it is not until a swing later.
#[test]
fn volley_holds_the_melee_swing_until_the_channel_ends() {
    let mut value = fixture("production-survival-hunter-2-targets");
    value["sim"]["iterations"] = json!(1);
    let logs = simulate_prepared(&parse(value)).unwrap().result["logs"]
        .as_str()
        .unwrap()
        .to_string();
    let hunter = unit_lines(&logs, "survival-hunter (#1)");
    let volley = hunter
        .iter()
        .copied()
        .find(|line| line.contains("Casting {SpellID: 14295}"))
        .map(at)
        .expect("the hunter channels Volley");
    let channel_end = volley + 6.0;
    let swings: Vec<f64> = hunter
        .iter()
        .copied()
        .filter(|line| {
            line.contains("Completed cast {OtherID: 3, Tag: 1}")
                || line.contains("Completed cast {OtherID: 3, Tag: 2}")
        })
        .map(at)
        .collect();
    assert!(
        swings
            .iter()
            .all(|&swing| swing <= volley + 0.001 || swing >= channel_end),
        "a swing lands in the channel: {swings:?}"
    );
    let first_after = swings
        .iter()
        .copied()
        .find(|&swing| swing >= channel_end)
        .expect("the hunter swings again");
    assert!(first_after - channel_end >= 0.95, "{first_after}");
}

/// Dust Cloud yields to Sunder Armor and Expose Armor (mythicsim patch 110): a Tallstrider
/// against a target with a permanent Expose Armor never casts it, and casts it once nothing
/// holds the major armor reduction category.
#[test]
fn dust_cloud_yields_to_the_major_armor_reductions() {
    let casts = |value: Value| {
        first_fight_log(value)
            .lines()
            .filter(|line| line.contains("Casting {SpellID: 1265904}"))
            .count()
    };
    assert_eq!(casts(fixture("beast-mastery-hunter-tallstrider")), 0);
    assert!(casts(tallstrider_without_armor_reductions()) > 0);
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

/// Strider Kick (fork patch 120) activates an aura of 30% movement speed for 3 seconds after its
/// hit, which the speed category gives and takes back with a log line each.
#[test]
fn strider_kick_speeds_the_hunter_up_for_three_seconds() {
    let logs = first_fight_log(fixture("production-survival-hunter"));
    let kick = logs
        .lines()
        .find(|line| line.contains("Casting {SpellID: 1317257}"))
        .expect("a kick");
    let time = at(kick);
    let stamp = format!("{time:.2}");
    assert!(has_line(
        &logs,
        &stamp,
        "[DEBUG] Movement speed changed from 7.00 (0.00%) to 9.10 (30.00%)"
    ));
    assert!(has_line(&logs, &stamp, "Aura gained: {SpellID: 1317257}"));
    let faded = format!("{:.2}", time + 3.0);
    assert!(has_line(&logs, &faded, "Aura faded: {SpellID: 1317257}"));
    assert!(has_line(
        &logs,
        &faded,
        "[DEBUG] Movement speed changed from 9.10 (30.00%) to 7.00 (0.00%)"
    ));
}

/// A kick without its speed aura changes nothing about the hunter's speed, and the effect is
/// what activates it.
#[test]
fn strider_kick_without_its_speed_aura_leaves_the_speed_alone() {
    let mut value = fixture("production-survival-hunter");
    let effect = value["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "strider_kick")
        .unwrap();
    assert_eq!(effect["speed_aura"], "Strider Kick");
    effect.as_object_mut().unwrap().remove("speed_aura");
    let logs = first_fight_log(value);
    assert!(!logs.contains("Movement speed changed"), "{logs}");
}

/// Fork patch 121: a speed change in the middle of a move runs the rest of it to the same
/// destination. The hunter runs out to 20 yards from 5, and a kick a second in speeds it up 30%
/// for the last 8 yards: 5 + 7 = 12 yards at -3.00, the rest at 9.1 yards a second. Re-planning
/// to the length of the move, 7 yards, ended the run at -2.67 on its way to the wrong place.
#[test]
fn a_speed_change_mid_move_keeps_the_destination() {
    let logs = first_fight_log(fixture("survival-hunter-strider-kick-while-moving"));
    assert!(has_line(&logs, "-4.00", "[DEBUG] Moving to 20.0 yards"));
    assert!(has_line(&logs, "-3.00", "{OtherID: 20} stacks: 5 --> 12"));
    assert!(has_line(
        &logs,
        "-3.00",
        "[DEBUG] Movement speed changed from 7.00 (0.00%) to 9.10 (30.00%)"
    ));
    // 8 yards at 9.1 a second take 0.879 seconds.
    assert!(has_line(&logs, "-2.12", "{OtherID: 20} stacks: 12 --> 20"));
    assert!(has_line(&logs, "-2.12", "Aura faded: {OtherID: 20}"));
}

/// Fork patch 119: the Wolf casts Furious Howl for 60 focus whenever its aura is down, giving
/// the pet and, within 15 yards of the target, the hunter 60 seconds of melee attack power.
#[test]
fn the_wolf_howls_for_itself_and_an_owner_within_range() {
    let logs = first_fight_log(fixture("beast-mastery-hunter-wolf"));
    let wolf = "hunter (#1) - Wolf";
    let cast = unit_lines(&logs, wolf)
        .into_iter()
        .find(|line| line.contains("Casting {SpellID: 24597}"))
        .expect("a howl");
    assert!(cast.contains("Cost = 60.000"), "{cast}");
    let stamp = format!("{:.2}", at(cast));
    // The pet's aura, then the hunter's.
    let gained: Vec<&str> = lines_at(&logs, &stamp)
        .into_iter()
        .filter(|line| line.contains("Aura gained: {SpellID: 24597}"))
        .collect();
    assert_eq!(gained.len(), 2, "{gained:?}");
    assert!(
        gained[0].contains(wolf) && !gained[1].contains(wolf),
        "{gained:?}"
    );
    // It recasts once the aura has faded, not before.
    let casts: Vec<f64> = unit_lines(&logs, wolf)
        .into_iter()
        .filter(|line| line.contains("Casting {SpellID: 24597}"))
        .map(at)
        .collect();
    assert!(
        casts.windows(2).all(|pair| pair[1] - pair[0] >= 60.0),
        "{casts:?}"
    );
}

/// The hunter stands at 25 yards, outside the effect's 15 yard radius, so only the pet is given
/// the aura.
#[test]
fn the_wolf_howls_for_itself_alone_when_its_owner_is_too_far() {
    let logs = first_fight_log(fixture("marksmanship-hunter-wolf-out-of-howl-range"));
    let wolf = "hunter (#1) - Wolf";
    let cast = logs
        .lines()
        .find(|line| line.contains(wolf) && line.contains("Casting {SpellID: 24597}"))
        .expect("a howl");
    let stamp = format!("{:.2}", at(cast));
    let gained: Vec<&str> = lines_at(&logs, &stamp)
        .into_iter()
        .filter(|line| line.contains("Aura gained: {SpellID: 24597}"))
        .collect();
    assert_eq!(gained.len(), 1, "{gained:?}");
    assert!(gained[0].contains(wolf), "{gained:?}");
}

/// The howl runs only with its effect, which claims the pet's aura.
#[test]
fn furious_howl_needs_its_effect() {
    let mut value = fixture("beast-mastery-hunter-wolf");
    assert!(check_prepared(&parse(value.clone())).is_ok());
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["kind"] != "hunter_pet_furious_howl");
    assert!(reasons(value).contains(&"the pet's ability at 1 has no behavior".to_string()));
}

/// Go pet.go makeStatInheritance (fork patch 136): the pet's physical crit is its master's Auto
/// Shot crit, physical and ranged-only crit together. The disable callback takes back the Auto
/// Shot's crit modifiers it added, of which the human's Ruthless leaves none (patch 137): its crit
/// counts for Auto Shot too, so the pet exports no disable stats.
#[test]
fn the_pet_inherits_its_masters_auto_shot_crit() {
    let value = fixture("production-beast-mastery-hunter");
    let pet = &value["pets"][0];
    assert_eq!(
        pet["inheritance"],
        json!([
            {"owner": "PhysicalCritPercent", "pet": "PhysicalCritPercent", "coefficient": 1},
            {"owner": "RangedCritPercent", "pet": "PhysicalCritPercent", "coefficient": 1},
        ])
    );
    assert_eq!(pet["disable_stats"], Value::Null);
}
