//! The shaman's shields against two refusals of 14 November 2026, under Forever's rules.
//!
//! - Water Shield on a shaman the boss attacks. The gate refused it (`player aura "Water Shield"
//!   reacts to the target's swings`): the aura hears the hits the shaman takes and nothing ran.
//!   The client row (408510, build 1.60.1.70938): SpellAuraOptions 223904 has ProcTypeMask
//!   221864, whose 0x8 bit is a melee hit taken, ProcChance 100, ProcCharges 3 and
//!   ProcCategoryRecovery 3500; SpellEffect 1072686 triggers 408511, whose SpellEffect 1072688
//!   restores 2% of maximum mana. So a landed swing spends a globe for 2% of maximum mana, one
//!   globe every 3.5 sec at most, on the cooldown the critical spells share.
//! - Lightning Shield below level 56. The gate knew the cast by rank 7's id (10432) alone, so a
//!   levelling shaman's rank (rank 5, 8134, from level 40; rank 6, 10431, from 48) was refused
//!   (`rotation reaches 8134 without a known behavior`) though the engine registers and runs the
//!   rank the level knows.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, prepared_refusals,
    simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const WATER_SHIELD: i64 = 408510;
const WATER_SHIELD_MANA: i64 = 408511;
const ITERATIONS: f64 = 300.0;

fn fixture(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

fn recast_when_down(spell: i64) -> Value {
    json!({ "action": {
        "castSpell": { "spellId": { "spellId": spell } },
        "condition": { "not": { "val": { "auraIsActive": { "auraId": { "spellId": spell } } } } },
    } })
}

/// MythicSim's Enhancement reference with a point in Water Shield, attacked or not. Its rotation
/// is the shield alone, kept up: no spell of the shaman's can crit, so only swings spend globes.
fn water_shield_request(attacked: bool) -> Value {
    let mut request = fixture("fixtures/boss-attacks/enhancement-shaman.request.json");
    request["simOptions"] = json!({ "iterations": ITERATIONS, "randomSeed": 7 });
    let shaman = player(&mut request);
    // Restoration: Water Shield is the tree's ninth talent.
    shaman["talentsString"] = json!("053303102-053030310051002251-000000001");
    shaman["rotation"] = json!({
        "type": "TypeAPL",
        "prepullActions": [],
        "priorityList": [recast_when_down(WATER_SHIELD)],
    });
    if attacked {
        shaman["inFrontOfTarget"] = json!(true);
        request["raid"]["tanks"] = json!([{ "type": "Player", "index": 0 }]);
    }
    request
}

fn prepared(request: &Value) -> PreparedV2 {
    let value = prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"));
    serde_json::from_value(value).unwrap()
}

/// The request through the gate and run: the player's metrics.
fn simulate(request: &Value) -> Value {
    let prepared = prepared(request);
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0].clone()
}

/// The casts of a spell per fight.
fn casts(player: &Value, id: i64) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| {
            action["id"]["spellId"].as_i64() == Some(id) && action["id"].get("tag").is_none()
        })
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["casts"].as_f64().unwrap_or(0.0))
        .sum::<f64>()
        / ITERATIONS
}

/// The globes spent per fight and the mana each gave.
fn globes(player: &Value) -> (f64, f64) {
    let rows: Vec<&Value> = player["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|resource| resource["id"]["spellId"].as_i64() == Some(WATER_SHIELD_MANA))
        .collect();
    let events: f64 = rows
        .iter()
        .map(|row| row["events"].as_f64().unwrap_or(0.0))
        .sum();
    let gain: f64 = rows
        .iter()
        .map(|row| row["gain"].as_f64().unwrap_or(0.0))
        .sum();
    if events == 0.0 {
        (0.0, 0.0)
    } else {
        (events / ITERATIONS, gain / events)
    }
}

#[test]
fn a_struck_shaman_spends_water_shield_globes_for_mana() {
    let request = water_shield_request(true);
    let typed = prepared(&request);
    let refusals = prepared_refusals(&typed);
    assert!(refusals.is_empty(), "gate refusals: {refusals:?}");
    let max_mana = serde_json::to_value(&typed.player.stats).unwrap()["Mana"]
        .as_f64()
        .unwrap();

    let attacked = simulate(&request);
    let (spent, mana_each) = globes(&attacked);
    let shields = casts(&attacked, WATER_SHIELD);
    assert!(spent > 0.0, "no globe spent");
    // 2% of maximum mana a globe (SpellEffect 1072688).
    assert!(
        (mana_each - max_mana * 0.02).abs() < 1e-6,
        "{mana_each} mana a globe of {max_mana}"
    );
    // One globe per 3.5 sec at most over a fight of at most 135 sec, and three a cast.
    assert!(spent <= 135.0 / 3.5 + 1.0, "{spent} globes a fight");
    assert!(
        spent <= shields * 3.0 + 1e-9,
        "{spent} globes from {shields} casts"
    );
    assert!(shields > 2.0, "{shields} casts: the globes are never spent");
}

#[test]
fn nothing_spends_the_globes_of_a_shaman_the_boss_does_not_attack() {
    let behind = simulate(&water_shield_request(false));
    assert_eq!(globes(&behind).0, 0.0);
    assert_eq!(casts(&behind, WATER_SHIELD), 1.0);
}

/// A levelling Elemental shaman (MythicSim's level 30 request) raised to a level, casting the
/// Lightning Shield rank that level knows.
fn lightning_shield_request(level: i64, rank: i64) -> Value {
    let mut request = fixture("fixtures/levels/elemental-shaman-L30.request.json");
    request["simOptions"] = json!({ "iterations": ITERATIONS, "randomSeed": 7 });
    // A levelling character's target is two levels above it.
    request["encounter"]["targets"][0]["level"] = (level + 2).into();
    let shaman = player(&mut request);
    shaman["level"] = level.into();
    shaman["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .insert(0, recast_when_down(rank));
    request
}

#[test]
fn a_levelling_shaman_casts_the_lightning_shield_rank_it_knows() {
    // Rank 4 (945) is learned at 32, rank 5 (8134) at 40, rank 6 (10431) at 48 and rank 7
    // (10432) at 56.
    for (level, rank) in [(30, 905), (42, 8134), (50, 10431)] {
        let request = lightning_shield_request(level, rank);
        let typed = prepared(&request);
        let refusals = prepared_refusals(&typed);
        assert!(refusals.is_empty(), "level {level}: {refusals:?}");
        let shaman = simulate(&request);
        assert_eq!(casts(&shaman, rank), 1.0, "level {level} rank {rank}");
    }
}
