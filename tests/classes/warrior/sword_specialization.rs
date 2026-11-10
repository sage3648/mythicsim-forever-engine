//! Weaponmaster's sword branch, Sword Specialization, under Forever's rules, from Forever combat
//! logs with a two-hand sword at 5/5: a Windfury Totem swing's roll brings no extra swing (0 of
//! 6), Sweeping Strikes' hits never roll it (0 of 336 over two logs), and Whirlwind and Cleave
//! roll it once a cast, not once a target (Whirlwind 7 procs on 100 casts and 283 landed hits).

use forever_engine::{contracts::prepared_v2::PreparedV2, prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const BATTLE_STANCE: i64 = 2457;
const BERSERKER_STANCE: i64 = 2458;
const SWEEPING_STRIKES: i64 = 12723;
const WHIRLWIND: i64 = 1680;
/// The tags of Sword Specialization's extra swing and the party Windfury Totem's.
const SWORD_SWING: i64 = 1290261;
const TOTEM_SWING: i64 = 25584;

/// The production Arms request (a two-hand sword), with its talents read as the rules read them.
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

fn cast(spell_id: i64) -> Value {
    json!({"castSpell": {"spellId": {"spellId": spell_id}}})
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "warrior", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn effect<'a>(prepared: &'a mut Value, kind: &str) -> &'a mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

/// Sword Specialization and the party's Windfury Totem both proc on every hit they hear.
fn every_hit_procs(prepared: &mut Value) {
    effect(prepared, "weaponmaster_sword")["proc_chance"] = json!(1.0);
    effect(prepared, "windfury_totem")["trigger_proc_chance"] = json!(1.0);
}

fn simulate(mut prepared: Value, iterations: u64, log: bool) -> Value {
    prepared["sim"]["iterations"] = json!(iterations);
    prepared["sim"]["debug_first_iteration"] = json!(log);
    let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
    forever_engine::simulate_prepared(&typed).unwrap().result
}

/// An auto attack's landed swings over a run, by tag.
fn landed_swings(result: &Value, tag: i64) -> u64 {
    let player = &result["raidMetrics"]["parties"][0]["players"][0];
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| {
            action["id"]["otherId"] == "OtherActionAttack"
                && action["id"]["tag"].as_i64() == Some(tag)
        })
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| {
            let count = |key: &str| target[key].as_u64().unwrap_or(0);
            count("casts") - count("misses") - count("dodges") - count("parries")
        })
        .sum()
}

/// With both procs certain and nothing but autos, every landed main hand swing brings one Sword
/// Specialization swing under Forever's rules: the totem's extra swing does not roll it. The
/// reference rolls it off the totem's swing too, so a landed totem swing brings another.
#[test]
fn a_windfury_totem_swing_does_not_roll_sword_specialization() {
    let swings = |rules: Rules| {
        let mut request = request("arms-warrior", rules);
        request["raid"]["parties"][0]["players"][0]["rotation"] = json!({
            "type": "TypeAPL",
            "prepullActions": [{"action": cast(BATTLE_STANCE), "doAtValue": {"const": {"val": "-1s"}}}],
            "priorityList": [],
        });
        let mut prepared = prepared(&request, rules);
        every_hit_procs(&mut prepared);
        let result = simulate(prepared, 20, false);
        let sword = result["raidMetrics"]["parties"][0]["players"][0]["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|action| action["id"]["tag"].as_i64() == Some(SWORD_SWING))
            .map(|action| action["targets"][0]["casts"].as_u64().unwrap_or(0))
            .sum::<u64>();
        (
            sword,
            landed_swings(&result, 1),
            landed_swings(&result, TOTEM_SWING),
        )
    };
    let (sword, autos, totem) = swings(Rules::Forever);
    assert!(
        autos > 500 && totem > 500,
        "{autos} autos, {totem} totem swings"
    );
    assert_eq!(sword, autos);
    let (sword, autos, totem) = swings(Rules::GoReference);
    assert_eq!(sword, autos + totem);
}

/// The Sword Specialization swings in each instant a Whirlwind goes off with no auto beside it.
fn sword_swings_per_whirlwind(logs: &str) -> Vec<usize> {
    let lines: Vec<&str> = logs.lines().collect();
    let time = |line: &str| {
        line.split_once(']')
            .map_or("", |(time, _)| time)
            .to_string()
    };
    let mut counts = Vec::new();
    let mut start = 0;
    while start < lines.len() {
        let now = time(lines[start]);
        let end = (start..lines.len())
            .find(|&index| time(lines[index]) != now)
            .unwrap_or(lines.len());
        let instant = &lines[start..end];
        let has = |text: &str| instant.iter().any(|line| line.contains(text));
        if has(&format!("Casting {{SpellID: {WHIRLWIND}, Tag: 1}}"))
            && !has("Casting {OtherID: 3, Tag: 1}")
        {
            let sword = format!("Casting {{OtherID: 3, Tag: {SWORD_SWING}}}");
            counts.push(instant.iter().filter(|line| line.contains(&sword)).count());
        }
        start = end;
    }
    counts
}

/// Three targets and Sweeping Strikes up: a Whirlwind strikes the three and Sweeping Strikes
/// adds its own hit, and with Sword Specialization certain the cast brings one extra swing under
/// Forever's rules. The reference rolls each target's hit and the Sweeping Strikes hit.
#[test]
fn one_whirlwind_brings_one_sword_specialization_swing() {
    let counts = |rules: Rules| {
        let mut request = request("arms-warrior-3-targets", rules);
        let player = &mut request["raid"]["parties"][0]["players"][0];
        player["dpsWarrior"]["options"]["classOptions"]["defaultStance"] =
            json!("WarriorStanceBattle");
        player["rotation"] = json!({
            "type": "TypeAPL",
            "prepullActions": [
                {"action": cast(SWEEPING_STRIKES), "doAtValue": {"const": {"val": "-2s"}}},
                {"action": cast(BERSERKER_STANCE), "doAtValue": {"const": {"val": "-1s"}}},
            ],
            "priorityList": [{"action": {"castSpell": {"spellId": {"spellId": WHIRLWIND, "tag": 1}}}}],
        });
        let mut prepared = prepared(&request, rules);
        every_hit_procs(&mut prepared);
        // Rage for Sweeping Strikes and the first Whirlwind before the pull.
        effect(&mut prepared, "rage_bar")["starting_rage"] = json!(100.0);
        let logs = simulate(prepared, 1, true)["logs"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            logs.contains("{SpellID: 12723, Tag: 1}"),
            "Sweeping Strikes strikes beside the Whirlwind"
        );
        sword_swings_per_whirlwind(&logs)
    };
    let forever = counts(Rules::Forever);
    assert!(forever.len() > 3, "{forever:?}");
    assert!(forever.iter().all(|&swings| swings <= 1), "{forever:?}");
    assert!(forever.contains(&1), "{forever:?}");
    let reference = counts(Rules::GoReference);
    assert!(reference.iter().any(|&swings| swings > 1), "{reference:?}");
}

#[test]
fn only_forever_rules_narrow_sword_specialization() {
    let mut forever = prepared(&request("arms-warrior", Rules::Forever), Rules::Forever);
    let effect_forever = effect(&mut forever, "weaponmaster_sword").clone();
    assert_eq!(effect_forever["skips_windfury_totem_swing"], json!(true));
    assert_eq!(
        effect_forever["excluded_class_spells"],
        json!([
            "shield_bash",
            "shield_slam",
            "sweeping_strikes_hit",
            "sweeping_strikes_normalized_hit"
        ])
    );
    assert_eq!(
        effect_forever["once_a_cast_class_spells"],
        json!(["whirlwind", "whirlwind_off_hand", "cleave"])
    );
    let mut reference = prepared(
        &request("arms-warrior", Rules::GoReference),
        Rules::GoReference,
    );
    let effect_reference = effect(&mut reference, "weaponmaster_sword").clone();
    assert!(effect_reference.get("skips_windfury_totem_swing").is_none());
    assert!(effect_reference.get("once_a_cast_class_spells").is_none());
    assert_eq!(
        effect_reference["excluded_class_spells"],
        json!(["shield_bash", "shield_slam"])
    );
}
