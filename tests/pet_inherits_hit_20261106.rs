//! A dynamic pet follows its owner's physical hit (prod Survival Hunter sims of 5 and 6
//! November 2026, refused with "pet inherits changes of PhysicalHitPercent ... which is
//! unsupported"). Felstriker's proc is a stat aura that raises the owner's hit, and Go hands
//! that change to the hunter's pet at its next stat inheritance heartbeat as the pet's physical
//! hit (coefficient 1) and spell hit (2.125). The runtime reads both in the pet's hit rolls.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const CASE: &str = "fixtures/forever-gaps/survival-hunter-felstriker-pet-hit.request.json";

/// The Felstriker Proc aura's bit in the stat aura combinations: the last of the effect's
/// eight auras.
const FELSTRIKER_PROC_BIT: usize = 7;

fn prepared() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(CASE);
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(40);
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever).unwrap()
}

fn stat_auras(prepared: &mut Value) -> &mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "stat_auras")
        .expect("a stat aura effect")
}

fn accepted(prepared: &Value) -> PreparedV2 {
    let prepared: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    assert_eq!(check_prepared(&prepared), Ok(()));
    prepared
}

/// The pet's landed and missed white hits, over every iteration.
fn pet_misses(prepared: &Value) -> (f64, f64) {
    let prepared = accepted(prepared);
    let result = simulate_prepared(&prepared).unwrap().result;
    let pet = &result["raidMetrics"]["parties"][0]["players"][0]["pets"][0];
    let mut hits = 0.0;
    let mut misses = 0.0;
    for action in pet["actions"].as_array().unwrap() {
        for target in action["targets"].as_array().unwrap() {
            hits +=
                target["hits"].as_f64().unwrap_or(0.0) + target["crits"].as_f64().unwrap_or(0.0);
            misses += target["misses"].as_f64().unwrap_or(0.0);
        }
    }
    (hits, misses)
}

/// Felstriker's request lists PhysicalHitPercent among the stats the owner's auras change and
/// the Cat inherits it: the gate accepts it and the fight runs.
#[test]
fn a_hunter_pet_under_an_owner_hit_aura_is_accepted() {
    let prepared = prepared();
    let changed = stat_auras(&mut prepared.clone())["changed"].clone();
    assert!(
        changed
            .as_array()
            .unwrap()
            .contains(&json!("PhysicalHitPercent")),
        "{changed}"
    );
    let (hits, _) = pet_misses(&prepared);
    assert!(hits > 0.0);
}

/// The pet's hit follows the owner's: the Cat misses nothing with the owner's real hit, but
/// with the owner's hit cut by 200 while the Felstriker proc is up the Cat's hit drops with it
/// at the next heartbeat and its white hits miss. With no coefficient it never follows.
#[test]
fn the_pets_hit_follows_the_owners() {
    let base = prepared();
    let (hits, real_misses) = pet_misses(&base);
    assert!(hits > 0.0);
    assert_eq!(real_misses, 0.0, "the Cat starts past the hit cap");

    // Every combination with the proc up takes 200 hit off the owner (the proc itself gives 100).
    let mut dropped = base.clone();
    for (mask, combo) in stat_auras(&mut dropped)["combos"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        if mask & (1 << FELSTRIKER_PROC_BIT) != 0 {
            let hit = combo["PhysicalHitPercent"].as_f64().unwrap();
            combo["PhysicalHitPercent"] = json!(hit - 200.0);
        }
    }
    let (_, dropped_misses) = pet_misses(&dropped);
    assert!(dropped_misses > 0.0, "dropped {dropped_misses}");

    // The same drop with no coefficient: the owner's hit never reaches the pet.
    let mut cut = dropped.clone();
    for pet in cut["pets"].as_array_mut().unwrap() {
        for term in pet["inheritance"].as_array_mut().unwrap() {
            if term["owner"] == "PhysicalHitPercent" {
                term["coefficient"] = json!(0.0);
            }
        }
    }
    let (_, cut_misses) = pet_misses(&cut);
    assert_eq!(cut_misses, 0.0);
}

/// A stat the runtime does not track is still refused: RangedHitPercent, which the Cat also
/// inherits, as a stat the owner's auras change.
#[test]
fn an_untracked_inherited_change_is_still_refused() {
    let mut value = prepared();
    stat_auras(&mut value)["changed"]
        .as_array_mut()
        .unwrap()
        .push(json!("RangedHitPercent"));
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    match check_prepared(&prepared) {
        Err(forever_engine::PreparedError::Unsupported(reasons)) => assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("inherits changes of RangedHitPercent")),
            "{reasons:?}"
        ),
        other => panic!("expected unsupported, got {other:?}"),
    }
}
