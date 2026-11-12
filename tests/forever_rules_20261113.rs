//! Two Warrior rules measured on combat logs of client 1.60.1.70888, under Forever's rules only
//! (the Go reference keeps its own): weapon procs do not hear Shield Slam or Shield Bash while
//! equip effects still do, and a crit of Whirlwind's off hand strike starts Flurry.
//! docs/mechanics.md has the evidence of each.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const SHIELD_SLAM: i64 = 23925;
const SHIELD_BASH: i64 = 1672;
const WHIRLWIND: i64 = 1680;
const FLURRY: i64 = 12966;
const FIERY_BLAZE_ENCHANT: i64 = 36;
const FIERY_BLAZE_HIT: i64 = 6297;
const CRUSADER: i64 = 1900;
const DREADFROST_SABER: i64 = 274603;
/// The live Fury and Protection references' talents on the launch tree.
const FURY: &str = "30303213002-050530045120100231-2";
const PROTECTION: &str = "-05250035-250033120310021251";
/// The main hand's place in the request's equipment.
const MAIN_HAND: usize = 12;

fn request(name: &str, talents: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{name}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(200);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(talents);
    request
}

/// The production Protection Warrior request with this main hand weapon and enchant.
fn protection_with(item: i64, enchant: i64) -> Value {
    let mut request = request("production-protection-warrior", PROTECTION);
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"][MAIN_HAND] =
        json!({"id": item, "enchant": enchant});
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn effect<'a>(prepared: &'a Value, kind: &str, trigger_aura: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind && effect["trigger_aura"] == trigger_aura)
        .unwrap_or_else(|| panic!("no {kind} effect of {trigger_aura}"))
}

fn effect_mut<'a>(prepared: &'a mut Value, kind: &str) -> &'a mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

/// The position in the player's book of the spell with this id and no tag.
fn position(prepared: &Value, spell_id: i64) -> u64 {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .position(|spell| {
            let id = &spell["action_id"];
            id["spell_id"] == spell_id && id["tag"].as_i64().unwrap_or(0) == 0
        })
        .unwrap_or_else(|| panic!("no spell {spell_id}")) as u64
}

/// The positions of the other main hand specials the warrior casts (Revenge, Sunder Armor and
/// the like), which a weapon proc of the main hand still hears.
fn other_main_hand_specials(prepared: &Value) -> Vec<u64> {
    let shields = [
        position(prepared, SHIELD_SLAM),
        position(prepared, SHIELD_BASH),
    ];
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, spell)| spell["proc_mask"] == json!(["ProcMaskMeleeMHSpecial"]))
        .map(|(position, _)| position as u64)
        .filter(|position| !shields.contains(position))
        .collect()
}

/// The spells a `chances` list rolls on.
fn chance_spells(effect: &Value) -> Vec<u64> {
    effect["chances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|chance| chance["spell"].as_u64().unwrap())
        .collect()
}

fn first_fight_log(prepared: &Value, seed: u64) -> String {
    let mut value = prepared.clone();
    value["sim"]["iterations"] = json!(1);
    value["sim"]["debug_first_iteration"] = json!(true);
    value["sim"]["seed"] = json!(seed);
    let prepared = serde_json::from_value(value).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

/// The lines of a log grouped by the moment they share.
fn moments(logs: &str) -> Vec<Vec<&str>> {
    let mut moments: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in logs.lines() {
        let time = &line[..line.find(']').unwrap() + 1];
        match moments.last_mut() {
            Some((at, lines)) if *at == time => lines.push(line),
            _ => moments.push((time, vec![line])),
        }
    }
    moments.into_iter().map(|(_, lines)| lines).collect()
}

/// A result line of the player's hit on a target.
fn hit_line(line: &str) -> bool {
    line.contains("] [Target ") && line.contains("(SpellSchool: ")
}

/// Weapon procs (the Fiery Blaze enchant here, Crusader on the reference's own main hand) do
/// not roll on Shield Slam or Shield Bash under Forever's rules, while they still roll on the
/// other main hand specials.
#[test]
fn weapon_procs_do_not_hear_shield_slam_or_shield_bash() {
    for (enchant, kind, trigger) in [
        (
            FIERY_BLAZE_ENCHANT,
            "spell_data_damage_proc",
            "Enchant: Fiery Blaze",
        ),
        (CRUSADER, "crusader", "Enchant Weapon - Crusader"),
    ] {
        let forever = prepared(&protection_with(11684, enchant), Rules::Forever);
        let heard = chance_spells(effect(&forever, kind, trigger));
        for spell_id in [SHIELD_SLAM, SHIELD_BASH] {
            assert!(
                !heard.contains(&position(&forever, spell_id)),
                "{trigger} rolls on {spell_id}: {heard:?}"
            );
        }
        let others = other_main_hand_specials(&forever);
        assert!(!others.is_empty());
        for other in others {
            assert!(heard.contains(&other), "{trigger} misses spell {other}");
        }
    }
}

/// An equip effect of the main hand weapon still hears both: Dreadfrost Saber's frost hit is
/// the 54 of 80 Shield Slams the main hand rule of 12 November rests on.
#[test]
fn an_equip_effect_still_hears_the_shield_strikes() {
    let forever = prepared(&protection_with(DREADFROST_SABER, 0), Rules::Forever);
    let saber = effect(&forever, "spell_data_damage_proc", "Dreadfrost Saber");
    let heard: Vec<u64> = saber["trigger_spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|spell| spell.as_u64().unwrap())
        .collect();
    for spell_id in [SHIELD_SLAM, SHIELD_BASH] {
        assert!(
            heard.contains(&position(&forever, spell_id)),
            "Dreadfrost Saber misses {spell_id}: {heard:?}"
        );
    }
}

/// In fights the Fiery Blaze hit never follows a Shield Slam or Shield Bash, and it does follow
/// other hits.
#[test]
fn no_fiery_blaze_hit_follows_a_shield_strike() {
    let forever = prepared(&protection_with(11684, FIERY_BLAZE_ENCHANT), Rules::Forever);
    let fiery = format!("{{SpellID: {FIERY_BLAZE_HIT}}}");
    let shield = |line: &str| {
        line.contains(&format!("{{SpellID: {SHIELD_SLAM}}}"))
            || line.contains(&format!("{{SpellID: {SHIELD_BASH}}}"))
    };
    let (mut procs, mut shield_strikes) = (0, 0);
    for seed in 1..=8 {
        let logs = first_fight_log(&forever, seed);
        let mut previous: Option<&str> = None;
        for line in logs.lines().filter(|line| hit_line(line)) {
            if line.contains(&fiery) {
                procs += 1;
                let cause = previous.expect("a hit before the proc");
                assert!(!shield(cause), "seed {seed}: {cause}\n{line}");
            } else {
                if shield(line) && !line.contains(" Miss ") {
                    shield_strikes += 1;
                }
                previous = Some(line);
            }
        }
    }
    assert!(procs > 10, "{procs} Fiery Blaze hits");
    assert!(shield_strikes > 50, "{shield_strikes} shield strikes");
}

/// The Fury reference prepared under Forever's rules, its Flurry effect as given.
fn fury(whirlwind_off_hand_starts: bool) -> Value {
    let mut prepared = prepared(&request("production-warrior", FURY), Rules::Forever);
    effect_mut(&mut prepared, "warrior_flurry")["whirlwind_off_hand_starts"] =
        json!(whirlwind_off_hand_starts);
    prepared
}

/// The moments a Whirlwind's off hand strike crits while nothing else the warrior deals at that
/// moment does, and whether Flurry starts or refills there.
fn off_hand_only_crits(prepared: &Value) -> Vec<bool> {
    let off_hand = format!("{{SpellID: {WHIRLWIND}, Tag: 2}} Crit ");
    let mut found = Vec::new();
    for seed in 1..=12 {
        let logs = first_fight_log(prepared, seed);
        for lines in moments(&logs) {
            let crits: Vec<&&str> = lines
                .iter()
                .filter(|line| hit_line(line) && line.contains("} Crit "))
                .collect();
            if crits.len() != 1 || !crits[0].contains(&off_hand) {
                continue;
            }
            let started = lines.iter().any(|line| {
                line.ends_with(&format!("Aura gained: {{SpellID: {FLURRY}}}"))
                    || line.contains(&format!("{{SpellID: {FLURRY}}} stacks: 0 --> 3"))
                    || line.contains(&format!("{{SpellID: {FLURRY}}} stacks: 1 --> 3"))
                    || line.contains(&format!("{{SpellID: {FLURRY}}} stacks: 2 --> 3"))
            });
            found.push(started);
        }
    }
    found
}

/// Under Forever's rules a crit of Whirlwind's off hand strike alone starts Flurry, or refills
/// it; without the rule, as in the reference, it never does.
#[test]
fn a_whirlwind_off_hand_crit_starts_flurry() {
    let forever = prepared(&request("production-warrior", FURY), Rules::Forever);
    let flurry = forever["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "warrior_flurry")
        .unwrap();
    assert_eq!(flurry["whirlwind_off_hand_starts"], true);
    // The reference takes the fixture's own talents on its tree.
    let mut reference = request("production-warrior", FURY);
    reference["raid"]["parties"][0]["players"][0]["talentsString"] =
        json!("20315013002-25253005102010501-2");
    let reference = prepared(&reference, Rules::GoReference);
    let flurry = reference["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "warrior_flurry")
        .unwrap();
    assert!(flurry.get("whirlwind_off_hand_starts").is_none());

    let with_rule = off_hand_only_crits(&fury(true));
    assert!(with_rule.len() >= 5, "{} off hand crits", with_rule.len());
    assert!(
        with_rule.iter().filter(|started| **started).count() >= with_rule.len() / 2,
        "{with_rule:?}"
    );
    let without = off_hand_only_crits(&fury(false));
    assert!(!without.is_empty());
    assert!(without.iter().all(|started| !started), "{without:?}");
}
