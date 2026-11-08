//! Physical damage measurements at levels 20 to 24 (in game, 7 November 2026), under Forever's
//! rules, which move every class with a physical row. The Go reference keeps its own numbers, so
//! every check also reads the reference.
//!
//! A block takes off a whole number, and a physical row that deals school damage takes the flat
//! physical damage bonus at its bonus coefficient column. The roll of such a row lands on a whole
//! number, which `a_physical_row_rolls_whole_numbers_with_its_ends_rarer` in
//! `src/classes/warrior/spells/revenge.rs` pins.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/warrior_trees_20261104.rs"]
mod warrior_trees_20261104;

const BATTLE_STANCE: i64 = 2457;
const HAMSTRING_RANK_3: i64 = 7373;
const REND_RANK_7: i64 = 11574;
const THUNDER_CLAP_RANK_6: i64 = 11581;
/// Forever's rules leave out the ranks an Ahn'Qiraj 20 book teaches, so its warrior's highest
/// Revenge is rank 5 and its Heroic Strike rank 8.
const REVENGE_RANK_5: i64 = 11601;
const SHIELD_SLAM_RANK_4: i64 = 23925;
const VICTORY_RUSH: i64 = 402927;
const HEROIC_STRIKE_RANK_8: i64 = 11567;

fn recorded(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/mage/prepared-v2/production-{name}.request.json"
    ));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The production request with its talents in the client's order, which Forever's rules read.
fn production(name: &str) -> Value {
    let mut request = recorded(name);
    warrior_trees_20261104::forever_request(&mut request);
    request
}

/// The request as the rules read its talents: the recorded string is the reference's.
fn production_for(name: &str, rules: Rules) -> Value {
    match rules {
        Rules::Forever => production(name),
        Rules::GoReference => recorded(name),
    }
}

fn player_mut(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "warrior", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

/// The exported spell with this spell id and no tag.
fn spell(prepared: &Value, spell_id: i64) -> &Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| {
            spell["action_id"]["spell_id"].as_i64() == Some(spell_id)
                && spell["action_id"]["tag"].as_i64().unwrap_or(0) == 0
        })
        .unwrap_or_else(|| panic!("spell {spell_id} is not exported"))
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

/// The player's metrics of a run of the prepared request.
fn run(prepared: &Value) -> Value {
    let typed: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    check_prepared(&typed).unwrap_or_else(|error| panic!("{error:?}"));
    let result = simulate_prepared(&typed).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0].clone()
}

/// A rotation that casts only these (a stance first), so nothing else gains rage or deals damage.
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

/// Damage a run's action with this spell id (any tag) dealt.
fn damage(player: &Value, spell_id: i64) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"].as_i64() == Some(spell_id))
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["damage"].as_f64().unwrap_or(0.0))
        .sum()
}

/// A block takes off the whole number below the block value, in both the target's swing at a
/// tank and Shield Slam's bonus; the reference takes off the fraction.
/// (`a_block_takes_off_a_whole_number` in `src/prepare/enemy/tests.rs` pins 7 for a 6 shield at
/// 59 Strength.)
#[test]
fn a_blocked_hit_loses_a_whole_number() {
    let reductions = |rules| -> Vec<f64> {
        let prepared = prepared(&production_for("protection-warrior", rules), rules);
        let enemy = &prepared["enemy"];
        ["rolls", "reduced_avoidance_rolls"]
            .into_iter()
            .flat_map(|key| enemy[key].as_array().unwrap().clone())
            .map(|rolls| number(&rolls["block_reduction"]))
            .collect()
    };
    let forever = reductions(Rules::Forever);
    assert!(!forever.is_empty());
    assert!(
        forever.iter().all(|reduction| reduction.fract() == 0.0),
        "{forever:?}"
    );
    assert!(
        forever.iter().any(|reduction| *reduction > 20.0),
        "{forever:?}"
    );
    let reference = reductions(Rules::GoReference);
    assert!(
        reference.iter().any(|reduction| reduction.fract() != 0.0),
        "{reference:?}"
    );
}

/// The bonus coefficient column a physical school damage row states is the share of the caster's
/// flat physical damage bonus it takes: Revenge, Hamstring and Shield Slam state 1, Thunder Clap,
/// Victory Rush and Shield Bash 0. A weapon strike takes the bonus whole and has no share.
#[test]
fn a_school_damage_row_carries_its_bonus_coefficient() {
    let mut request = production("protection-warrior");
    player_mut(&mut request)["talentsString"] = json!("--250033120310021251");
    let forever = prepared(&request, Rules::Forever);
    for (id, coefficient) in [
        (REVENGE_RANK_5, 1.0),
        (HAMSTRING_RANK_3, 1.0),
        (SHIELD_SLAM_RANK_4, 1.0),
        (THUNDER_CLAP_RANK_6, 0.0),
        (VICTORY_RUSH, 0.0),
    ] {
        assert_eq!(
            forever["player"]["spells"]
                .as_array()
                .unwrap()
                .iter()
                .find(|spell| {
                    spell["action_id"]["spell_id"].as_i64() == Some(id)
                        && spell["action_id"]["tag"].as_i64().unwrap_or(0) == 0
                })
                .map(|spell| spell["physical_school_coefficient"].clone()),
            Some(json!(coefficient)),
            "spell {id}"
        );
    }
    assert!(spell(&forever, HEROIC_STRIKE_RANK_8)
        .get("physical_school_coefficient")
        .is_none());

    let reference = prepared(&request, Rules::GoReference);
    for spell in reference["player"]["spells"].as_array().unwrap() {
        assert!(spell.get("physical_school_coefficient").is_none());
    }
    assert!(reference["player"]
        .get("physical_rolls_are_whole")
        .is_none());
    assert_eq!(forever["player"]["physical_rolls_are_whole"], json!(true));
}

/// What a fight that casts only this spell in Battle Stance dealt with it, with this much flat
/// physical damage bonus on the sheet. The bonus changes no draw, so the fights match hit for hit.
fn damage_with_flat_bonus(rules: Rules, bonus: f64, spell_id: i64) -> f64 {
    let mut request = production_for("arms-warrior", rules);
    player_mut(&mut request)["rotation"] = rotation(BATTLE_STANCE, &[spell_id]);
    request["simOptions"]["iterations"] = json!(50);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    let mut value = prepared(&request, rules);
    value["player"]["stats"]["PhysicalDamage"] = json!(bonus);
    damage(&run(&value), spell_id)
}

/// Hamstring takes the flat physical damage bonus and Thunder Clap and Rend's ticks do not (their
/// coefficient columns state 1, 0 and nothing). The reference gives every physical spell the
/// whole bonus except Rend's ticks, which never took it.
#[test]
fn flat_physical_damage_follows_the_rows_coefficient() {
    for (rules, takes) in [
        (Rules::Forever, [false, false, true]),
        (Rules::GoReference, [false, true, true]),
    ] {
        for (spell_id, takes) in [REND_RANK_7, THUNDER_CLAP_RANK_6, HAMSTRING_RANK_3]
            .into_iter()
            .zip(takes)
        {
            let without = damage_with_flat_bonus(rules, 0.0, spell_id);
            let with = damage_with_flat_bonus(rules, 100.0, spell_id);
            assert!(without > 0.0, "{rules:?}: spell {spell_id} deals nothing");
            if takes {
                assert!(with > without + 100.0, "{rules:?}: spell {spell_id}");
            } else {
                assert_eq!(with, without, "{rules:?}: spell {spell_id}");
            }
        }
    }
}
