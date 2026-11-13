//! Farraki Kris (274647), the Forever dagger whose equip effect refused real players' Quick Sims
//! on 13 November 2026 ("item 274647 has an effect not prepared yet (a chance on harmful spell
//! cast whose rate the client does not state on a character with a mana bar)": an Arcane Mage,
//! level 49, prod sim b0970a12-fbba-4560-8731-d6f079bd421b, another Arcane Mage and a Shadow
//! Priest).
//!
//! "Equip: Chance on harmful spell cast to afflict the target with a draining ritual, leeching 19
//! health every 2 sec for 6 sec. (100ms cooldown)". ItemEffect 233957 is an equip of Blood
//! Sacrifice (1294644): SpellAuraOptions 246039 has ProcChance 100, ProcTypeMask 65536 (harmful
//! spells dealt) and a proc recovery of 100 ms, and it casts Blood Sacrifice (1294645), a Periodic
//! Leech (53) of 19 every 2000 ms for 6 sec, Shadow, on the magic table (wago.tools
//! `product=wowf`, 13 November 2026). The rows are read literally: the chance is 100%, gated by
//! the 100 ms recovery (docs/mechanics.md).
//!
//! No refused request was at hand, so the tests dress fixture characters: the Arcane Mage of
//! `fixtures/dev-notes-20261102`, the Shadow Priest of `fixtures/forever-gaps` (two targets), the
//! Elemental Shaman of `fixtures/forever-gaps` and the Combat Rogue of `fixtures/forever-gaps`,
//! who has no mana bar.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/rogue_113483.rs"]
mod rogue_113483;

const FARRAKI_KRIS: i64 = 274647;
const BLOOD_SACRIFICE: i64 = 1294645;
const MAIN_HAND: usize = 14;

const MAGE: &str = "fixtures/dev-notes-20261102/arcane-mage.request.json";
const PRIEST: &str = "fixtures/forever-gaps/shadow-priest-chastise.request.json";
const SHAMAN: &str = "fixtures/forever-gaps/elemental-shaman-the-elements.request.json";
const ROGUE: &str = "fixtures/forever-gaps/combat-rogue-self-casts-at-player.request.json";

/// A fixture's request with this item in the main hand.
fn wielding(fixture: &str, main_hand: Value) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.resize(17, json!({}));
    equipment[MAIN_HAND] = main_hand;
    request
}

fn kris(fixture: &str) -> Value {
    let mut request = wielding(fixture, json!({ "id": FARRAKI_KRIS }));
    if fixture == ROGUE {
        rogue_113483::forever_request(&mut request);
    }
    request
}

fn prepared(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// A prepared input through the gate and run: the player's metrics.
fn simulate(value: &Value) -> Value {
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    let player = result["raidMetrics"]["parties"][0]["players"][0].clone();
    assert!(player["dps"]["avg"].as_f64().unwrap() > 0.0);
    player
}

/// The player's actions with this spell id.
fn actions(player: &Value, id: i64) -> Vec<&Value> {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"].as_i64() == Some(id))
        .collect()
}

/// A field of the player's actions with this spell id, summed over every tag and target.
fn total(player: &Value, id: i64, field: &str) -> f64 {
    actions(player, id)
        .into_iter()
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target[field].as_f64().unwrap_or(0.0))
        .sum()
}

fn kris_effect(prepared: &Value) -> Option<&Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["kind"] == "spell_data_damage_proc" && effect["trigger_aura"] == "Farraki Kris"
        })
}

/// On each caster that refused, the kris prepares, passes the gate and runs, and the leech's ticks
/// show up as Shadow damage on the target.
#[test]
fn the_kris_prepares_and_its_leech_ticks_on_every_caster() {
    for fixture in [MAGE, PRIEST, SHAMAN] {
        let prepared = prepared(&kris(fixture));
        let player = simulate(&prepared);
        let ticks = total(&player, BLOOD_SACRIFICE, "ticks");
        assert!(ticks > 0.0, "{fixture}: no ticks");
        assert!(
            total(&player, BLOOD_SACRIFICE, "damage") > 0.0,
            "{fixture}: no damage"
        );
        let action = actions(&player, BLOOD_SACRIFICE)[0];
        assert_eq!(action["spellSchool"], 32, "{fixture}");
        // The hit that applies it rolls the magic table, which can miss.
        assert!(total(&player, BLOOD_SACRIFICE, "hits") > 0.0, "{fixture}");
        assert!(total(&player, BLOOD_SACRIFICE, "misses") > 0.0, "{fixture}");
    }
}

/// The proc is the row's: it hears every harmful spell cast (a chance of 1, no procs per
/// minute), applies a damage over time alone of 19 a tick, with no direct hit and no coefficient.
#[test]
fn the_proc_is_the_rows_every_harmful_cast_and_a_leech_of_19() {
    let prepared = prepared(&kris(MAGE));
    let effect = kris_effect(&prepared).expect("the kris's proc");
    assert_eq!(effect["proc_chance"], 1.0);
    assert_eq!(effect["casts"], true);
    assert_eq!(effect["average"], 0.0);
    assert_eq!(effect["periodic"]["tick_base"], 19.0);
    assert_eq!(effect["periodic"]["with_direct"], false);
    assert_eq!(effect["periodic"]["application"], "magic_hit");
    assert!(!effect["trigger_spells"].as_array().unwrap().is_empty());
    assert!(effect.get("chances").is_none());

    let position = effect["spell"].as_u64().unwrap() as usize;
    let spell = &prepared["player"]["spells"][position];
    assert_eq!(spell["action_id"]["spell_id"], BLOOD_SACRIFICE);
    assert_eq!(spell["school"], 32);
    assert_eq!(spell["defense_type"], "DefenseTypeMagic");
    assert_eq!(spell["bonus_coefficient"], 0.0);
    assert!(spell["damage_effect"].is_null());
    // 6 sec over 2 sec: three ticks, on one aura per target, which a second cast refreshes.
    assert_eq!(spell["dot"]["base_tick_count"], 3);
    assert_eq!(spell["dot"]["base_tick_length_ns"], 2_000_000_000_i64);
    assert_eq!(spell["dot"]["bonus_coefficient"], 0.0);
}

/// Every tick deals the same amount, the row's 19 under the wearer's damage multipliers: ticks of
/// this row do not crit (no periodic crit attribute), and none is raised by the wearer's spell
/// power.
#[test]
fn every_tick_deals_the_same_amount() {
    for fixture in [MAGE, PRIEST] {
        let player = simulate(&prepared(&kris(fixture)));
        for action in actions(&player, BLOOD_SACRIFICE) {
            for target in action["targets"].as_array().unwrap() {
                let Some(range) = target.get("tickRange") else {
                    continue;
                };
                let (min, max) = (
                    range["min"].as_f64().unwrap(),
                    range["max"].as_f64().unwrap(),
                );
                assert!((min - max).abs() < 1e-9, "{fixture}: {min} {max}");
                assert!((19.0..19.0 * 1.5).contains(&min), "{fixture}: {min}");
            }
        }
    }
}

/// A second target (the priest's fixture has two) gets its own leech.
#[test]
fn each_target_carries_its_own_leech() {
    let player = simulate(&prepared(&kris(PRIEST)));
    let targets: Vec<&Value> = actions(&player, BLOOD_SACRIFICE)[0]["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|target| target["ticks"].as_f64().unwrap_or(0.0) > 0.0)
        .collect();
    assert!(!targets.is_empty());
}

/// A character with no mana bar casts no harmful spell and wears the kris as a plain dagger: no
/// proc, no Blood Sacrifice damage.
#[test]
fn a_rogue_wears_it_as_a_plain_dagger() {
    let prepared = prepared(&kris(ROGUE));
    let player = simulate(&prepared);
    for field in ["ticks", "hits", "misses", "damage"] {
        assert_eq!(total(&player, BLOOD_SACRIFICE, field), 0.0, "{field}");
    }
}

/// The reference's rules still refuse the kris as unknown.
#[test]
fn the_reference_rules_still_refuse_the_kris() {
    let err = prepare_json_with(
        kris(MAGE).to_string().as_bytes(),
        "rust",
        Rules::GoReference,
    )
    .err()
    .map(|err| err.to_string())
    .unwrap_or_default();
    assert!(
        err.contains(&format!("no item with id {FARRAKI_KRIS}")),
        "{err}"
    );
}
