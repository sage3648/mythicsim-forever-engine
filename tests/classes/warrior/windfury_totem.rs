//! The party's Windfury Totem on a Warrior under Forever's rules. Every ability's main hand
//! strike rolls the totem since Blizzard's hotfixes of 13 November 2026 ("Windfury Totem is now
//! correctly activated by all melee abilities even if it doesn't involve your main hand
//! weapon"); an off hand strike and a triggered hit such as Sweeping Strikes' do not. Before the
//! hotfix an ability rolled it only when its client row needs a main hand weapon
//! (`Attributes[3]` 0x400) or goes off on the next swing (`Attributes[0]` 0x4): Bloodthirst 0 of
//! 45, Whirlwind 0 of 55 and 0 of 279, Overpower 0 of 97, while Mortal Strike procs it 29 times
//! in 150. From Forever combat logs: a landed hit rolls it without dealing damage (Sunder Armor
//! applications at 15%). The auto that fires the proc spends no charge, so the next off hand
//! auto keeps the attack power (46 of 47), and a missed auto spends one (8 of 8).

use forever_engine::{contracts::prepared_v2::PreparedV2, prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const BATTLE_STANCE: i64 = 2457;
const BERSERKER_STANCE: i64 = 2458;
const BLOODTHIRST: i64 = 23894;
const MORTAL_STRIKE: i64 = 21553;
const WHIRLWIND: i64 = 1680;
const OVERPOWER: i64 = 11585;
const SLAM: i64 = 11605;
/// Sweeping Strikes' extra hit, a triggered hit.
const SWEEPING_STRIKES_HIT: i64 = 12723;
/// The tag of the totem's extra main hand attack.
const TOTEM_SWING: i64 = 25584;
/// The totem's charges of attack power.
const CHARGES: &str = "{SpellID: 10610, Tag: -1} stacks: ";

/// The production request, with its talents read as the rules read them.
fn request(name: &str, rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/mage/prepared-v2/production-{name}.request.json"
    ));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    if rules == Rules::Forever {
        crate::warrior_trees_20261104::forever_request(&mut request);
    }
    request
}

/// A rotation of a stance and these casts only.
fn rotation(stance: i64, casts: &[i64]) -> Value {
    json!({
        "type": "TypeAPL",
        "prepullActions": [{
            "action": {"castSpell": {"spellId": {"spellId": stance}}},
            "doAtValue": {"const": {"val": "-1s"}},
        }],
        "priorityList": casts
            .iter()
            .map(|id| json!({"action": {"castSpell": {"spellId": {"spellId": id}}}}))
            .collect::<Vec<_>>(),
    })
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "warrior", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn totem(prepared: &mut Value) -> &mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "windfury_totem")
        .expect("a windfury_totem effect")
}

/// The action ids of the spells the totem's trigger hears.
fn heard(prepared: &mut Value) -> Vec<Value> {
    let positions = totem(prepared)["trigger_spells"].clone();
    positions
        .as_array()
        .unwrap()
        .iter()
        .map(|position| {
            prepared["player"]["spells"][position.as_u64().unwrap() as usize]["action_id"].clone()
        })
        .collect()
}

fn hears(heard: &[Value], spell_id: i64, tag: i64) -> bool {
    heard.iter().any(|id| {
        id["spell_id"].as_i64() == Some(spell_id) && id["tag"].as_i64().unwrap_or(0) == tag
    })
}

fn hears_auto(heard: &[Value], tag: i64) -> bool {
    heard
        .iter()
        .any(|id| id["other_id"] == "OtherActionAttack" && id["tag"].as_i64() == Some(tag))
}

/// The totem procs every hit it hears, and only the abilities' hits: the main hand autos and
/// extra swings, which roll it under either rules, leave the trigger.
fn abilities_only_at_full_chance(prepared: &mut Value) {
    let autos: Vec<u64> = prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, spell)| {
            spell["proc_mask"]
                .as_array()
                .unwrap()
                .iter()
                .any(|mask| mask == "ProcMaskMeleeMHAuto")
        })
        .map(|(position, _)| position as u64)
        .collect();
    let totem = totem(prepared);
    totem["trigger_proc_chance"] = json!(1.0);
    totem["trigger_spells"]
        .as_array_mut()
        .unwrap()
        .retain(|position| !autos.contains(&position.as_u64().unwrap()));
}

fn simulate(prepared: Value, iterations: u64) -> Value {
    let mut prepared = prepared;
    prepared["sim"]["iterations"] = json!(iterations);
    let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
    forever_engine::simulate_prepared(&typed).unwrap().result
}

/// The casts of the totem's extra swing over a run.
fn totem_swings(result: &Value) -> u64 {
    result["raidMetrics"]["parties"][0]["players"][0]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| {
            action["id"]["otherId"] == "OtherActionAttack"
                && action["id"]["tag"].as_i64() == Some(TOTEM_SWING)
        })
        .map(|action| action["targets"][0]["casts"].as_u64().unwrap_or(0))
        .sum()
}

fn first_fight_log(mut prepared: Value) -> String {
    prepared["sim"]["debug_first_iteration"] = json!(true);
    simulate(prepared, 1)["logs"].as_str().unwrap().to_string()
}

/// A line's charge change, `(before, after)`.
fn charge_change(line: &str) -> Option<(i32, i32)> {
    let (_, change) = line.split_once(CHARGES)?;
    let (before, after) = change.split_once(" --> ")?;
    Some((before.trim().parse().ok()?, after.trim().parse().ok()?))
}

fn time_of(line: &str) -> &str {
    line.split_once(']').map_or("", |(time, _)| time)
}

#[test]
fn every_ability_main_hand_strike_rolls_the_totem() {
    let mut fury = prepared(&request("warrior", Rules::Forever), Rules::Forever);
    let fury_heard = heard(&mut fury);
    assert!(hears_auto(&fury_heard, 1), "the main hand auto");
    assert!(hears_auto(&fury_heard, TOTEM_SWING), "the extra swing");
    assert!(!hears_auto(&fury_heard, 2), "the off hand auto");
    for (spell, tag) in [(BLOODTHIRST, 0), (WHIRLWIND, 1), (OVERPOWER, 0)] {
        assert!(hears(&fury_heard, spell, tag), "{spell} tag {tag}");
    }
    assert!(
        !hears(&fury_heard, WHIRLWIND, 2),
        "Whirlwind's off hand strike"
    );
    let mut arms = prepared(&request("arms-warrior", Rules::Forever), Rules::Forever);
    let arms_heard = heard(&mut arms);
    for spell in [MORTAL_STRIKE, SLAM, OVERPOWER] {
        assert!(hears(&arms_heard, spell, 0), "{spell}");
    }
    for tag in [0, 1] {
        assert!(
            !hears(&arms_heard, SWEEPING_STRIKES_HIT, tag),
            "Sweeping Strikes' hit, tag {tag}"
        );
    }
    let effect = totem(&mut fury).clone();
    assert_eq!(effect["trigger_proc_chance"], json!(0.2));
    assert_eq!(effect["trigger_require_damage"], json!(false));
    assert_eq!(effect["trigger_spends_no_charge"], json!(true));
    assert_eq!(effect["any_outcome_spends"], json!(true));

    // The reference hears every main hand melee spell and needs damage.
    let mut reference = prepared(&request("warrior", Rules::GoReference), Rules::GoReference);
    assert!(hears(&heard(&mut reference), BLOODTHIRST, 0));
    let effect = totem(&mut reference).clone();
    assert_eq!(effect["trigger_require_damage"], json!(true));
    assert!(effect.get("trigger_spends_no_charge").is_none());
    assert!(effect.get("any_outcome_spends").is_none());
}

/// A warrior that casts nothing but Bloodthirst gets totem procs off it under Forever's rules
/// since the hotfixes of 13 November 2026, as one that casts Mortal Strike does and as the
/// reference does.
#[test]
fn bloodthirst_rolls_the_totem_as_mortal_strike_does() {
    let procs = |name: &str, stance: i64, spell: i64, rules: Rules| {
        let mut request = request(name, rules);
        request["raid"]["parties"][0]["players"][0]["rotation"] = rotation(stance, &[spell]);
        let mut prepared = prepared(&request, rules);
        abilities_only_at_full_chance(&mut prepared);
        totem_swings(&simulate(prepared, 20))
    };
    assert!(procs("warrior", BERSERKER_STANCE, BLOODTHIRST, Rules::Forever) > 100);
    assert!(procs("warrior", BERSERKER_STANCE, BLOODTHIRST, Rules::GoReference) > 100);
    assert!(procs("arms-warrior", BATTLE_STANCE, MORTAL_STRIKE, Rules::Forever) > 100);
}

/// The Fury warrior's first fight, ten minutes long, with the totem procing on every landed main
/// hand auto, out of Bloodthirst only, so no Heroic Strike takes the extra swing.
fn fury_log(rules: Rules) -> String {
    let mut request = request("warrior", rules);
    request["encounter"]["duration"] = json!(600);
    request["raid"]["parties"][0]["players"][0]["rotation"] =
        rotation(BERSERKER_STANCE, &[BLOODTHIRST]);
    let mut prepared = prepared(&request, rules);
    totem(&mut prepared)["trigger_proc_chance"] = json!(1.0);
    first_fight_log(prepared)
}

/// The charges granted and the charges left at the end of each instant in which a main hand
/// auto procs the totem and nothing but its extra swing swings with it.
fn charges_after_auto_procs(logs: &str) -> Vec<(i32, i32)> {
    let lines: Vec<&str> = logs.lines().collect();
    let mut left = Vec::new();
    let mut start = 0;
    while start < lines.len() {
        let time = time_of(lines[start]);
        let end = (start..lines.len())
            .find(|&index| time_of(lines[index]) != time)
            .unwrap_or(lines.len());
        let instant = &lines[start..end];
        let swings: Vec<&&str> = instant
            .iter()
            .filter(|line| line.contains("] Casting {OtherID: 3, Tag: "))
            .collect();
        let auto_then_totem = swings.len() == 2
            && swings[0].contains("Tag: 1}")
            && swings[1].contains(&format!("Tag: {TOTEM_SWING}}}"));
        let changes: Vec<(i32, i32)> = instant
            .iter()
            .filter_map(|line| charge_change(line))
            .collect();
        if let (true, Some(&(_, granted))) = (auto_then_totem, changes.first()) {
            if granted >= 1 {
                left.push((granted, changes.last().unwrap().1));
            }
        }
        start = end;
    }
    left
}

/// After a main hand auto procs the totem and its extra swing goes off, one charge is left
/// under Forever's rules: the proc grants both and the auto spends none. The reference takes
/// one off for the auto, and the extra swing spends the other.
#[test]
fn an_auto_proc_leaves_one_charge_after_its_extra_swing() {
    let forever = charges_after_auto_procs(&fury_log(Rules::Forever));
    assert!(forever.len() > 5, "{forever:?}");
    assert!(
        forever.iter().all(|&charges| charges == (2, 1)),
        "{forever:?}"
    );
    // The reference grants one charge fewer, which a landed extra swing spends.
    let reference = charges_after_auto_procs(&fury_log(Rules::GoReference));
    assert!(reference.len() > 5, "{reference:?}");
    assert!(
        reference.iter().all(|&(granted, _)| granted == 1),
        "{reference:?}"
    );
    assert!(reference.contains(&(1, 0)), "{reference:?}");
}

/// Of the hands' autos that miss while charges are up, how many spend one. The totem's own
/// extra swing is left out: the spend that follows its miss can be the triggering auto's.
fn missed_autos_spending(logs: &str) -> (usize, usize) {
    let lines: Vec<&str> = logs.lines().collect();
    let (mut charges, mut misses, mut spent) = (0, 0, 0);
    for (index, line) in lines.iter().enumerate() {
        if let Some((_, after)) = charge_change(line) {
            charges = after;
        }
        // The charges fade with the aura, a second after the proc.
        if line.contains("Aura faded: {SpellID: 10610, Tag: -1}") {
            charges = 0;
        }
        let auto = line.contains("{OtherID: 3, Tag: 1} Miss (")
            || line.contains("{OtherID: 3, Tag: 2} Miss (");
        if !auto || charges == 0 {
            continue;
        }
        misses += 1;
        // The spender acts on the miss before the next swing goes off.
        let next = lines[index + 1..]
            .iter()
            .take_while(|next| {
                time_of(next) == time_of(line) && !next.contains("] Casting {OtherID: 3, Tag: ")
            })
            .find_map(|next| charge_change(next));
        if next == Some((charges, charges - 1)) {
            spent += 1;
        }
    }
    (misses, spent)
}

/// A missed auto spends a charge under Forever's rules (8 of 8 missed swings in the logs). The
/// reference spends charges on landed autos only.
#[test]
fn a_missed_auto_spends_a_charge() {
    let (misses, spent) = missed_autos_spending(&fury_log(Rules::Forever));
    assert!(misses > 3, "{misses} misses with charges up");
    assert_eq!(spent, misses);
    let (misses, spent) = missed_autos_spending(&fury_log(Rules::GoReference));
    assert!(misses > 0, "{misses} misses with charges up");
    assert_eq!(spent, 0);
}
