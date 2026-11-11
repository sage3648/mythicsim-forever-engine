//! Forever client items whose tooltips carry a line the import read as unknown until 10 November
//! 2026 (`tools/rust_client_items.py`, Wowhead's Forever tooltips of 9 November):
//!
//! - Lines that change nothing in a fight are left out of the row: mount and swim speed, a zone,
//!   falling, stealth detection, a profession, quest text, uses and procs that only control an
//!   enemy. The Desert Rose (274648): "Equip: Increases mount speed by 7% in Desert areas."
//! - Expertise, weapon damage, weapon skill and all resistances, read as the reference rows hold
//!   the same lines: Spaulders of Containment (287022), "Reduces chance to be Dodged or Parried by
//!   1.0%"; Knucklebone Ring (274614), "+2 Weapon Damage"; Bloody Claws of Attack (280068) and Big
//!   Basher's Belt (286619), "Increased Fist Weapons +1"; Titanosaur Scale Pendant (280009), "+4
//!   to All Resistances".
//! - Spell damage against a creature type, as Mark of the Champion's: Courier's Clasp (279833),
//!   "Increases damage done to Humanoids by magical spells and effects by up to 10."

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const DESERT_ROSE: i64 = 274648;
const SPAULDERS_OF_CONTAINMENT: i64 = 287022;
const KNUCKLEBONE_RING: i64 = 274614;
const BLOODY_CLAWS: i64 = 280068;
const BIG_BASHERS_BELT: i64 = 286619;
const TITANOSAUR_PENDANT: i64 = 280009;
const COURIERS_CLASP: i64 = 279833;

const NECK: usize = 1;
const SHOULDER: usize = 2;
const WAIST: usize = 7;
const FINGER: usize = 10;
const TRINKET: usize = 12;
const MAIN_HAND: usize = 14;
const OFF_HAND: usize = 15;

fn read(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// A request without buffs, 20 iterations, with the given items in their slots.
fn wearing(mut request: Value, items: &[(usize, i64)]) -> Value {
    request["simOptions"]["iterations"] = json!(20);
    request["raid"]["buffs"] = json!({});
    request["raid"]["parties"][0]["buffs"] = json!({});
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["buffs"] = json!({});
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.resize(17, json!({}));
    for &(slot, id) in items {
        equipment[slot] = if id == 0 {
            json!({})
        } else {
            json!({ "id": id })
        };
    }
    request
}

/// The level 40 Enhancement Shaman of the Ceremonial Mallet's refused request.
fn shaman(items: &[(usize, i64)]) -> Value {
    wearing(
        read("fixtures/forever-gaps/enhancement-shaman-ceremonial-mallet.request.json"),
        items,
    )
}

fn prepare(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// Prepared, through the gate and run.
fn run(request: &Value) -> PreparedV2 {
    let prepared: PreparedV2 = serde_json::from_value(prepare(request)).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    simulate_prepared(&prepared).unwrap();
    prepared
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap()
}

/// The difference an item makes to the shaman's prepared stats.
fn added(slot: usize, id: i64, name: &str) -> f64 {
    stat(&prepare(&shaman(&[(slot, id)])), name) - stat(&prepare(&shaman(&[(slot, 0)])), name)
}

#[test]
fn a_line_with_no_fight_effect_leaves_the_item_without_one() {
    let with = prepare(&shaman(&[(TRINKET, DESERT_ROSE)]));
    let without = prepare(&shaman(&[(TRINKET, 0)]));
    assert_eq!(with["player"]["stats"], without["player"]["stats"]);
    assert_eq!(with["effects"], without["effects"]);
    run(&shaman(&[(TRINKET, DESERT_ROSE)]));
}

#[test]
fn dodged_or_parried_is_expertise_rating() {
    assert_eq!(
        added(SHOULDER, SPAULDERS_OF_CONTAINMENT, "ExpertiseRating"),
        10.0
    );
}

#[test]
fn weapon_damage_is_bonus_physical_damage() {
    assert_eq!(added(FINGER, KNUCKLEBONE_RING, "PhysicalDamage"), 2.0);
}

#[test]
fn all_resistances_is_each_resistance() {
    for school in ["Arcane", "Fire", "Frost", "Nature", "Shadow"] {
        let name = format!("{school}Resistance");
        assert_eq!(added(NECK, TITANOSAUR_PENDANT, &name), 4.0, "{name}");
    }
}

/// Bloody Claws of Attack, a fist weapon, and Big Basher's Belt each add one to the fist weapon
/// skill the main hand's attack table reads.
#[test]
fn fist_weapon_skill_adds_to_the_main_hands_skill() {
    let skill = |items: &[(usize, i64)]| {
        let prepared = prepare(&shaman(items));
        prepared["melee"]["main_hand_skill"]["skill"]
            .as_f64()
            .unwrap_or(0.0)
    };
    let claws = [(MAIN_HAND, BLOODY_CLAWS), (OFF_HAND, 0)];
    let both = [
        (MAIN_HAND, BLOODY_CLAWS),
        (OFF_HAND, 0),
        (WAIST, BIG_BASHERS_BELT),
    ];
    assert_eq!(skill(&claws), 1.0);
    assert_eq!(skill(&both), 2.0);
    run(&shaman(&both));
}

fn with_target_type(mut request: Value, mob_type: &str) -> Value {
    request["encounter"]["targets"][0]["mobType"] = json!(mob_type);
    request
}

fn clasp_bonuses(prepared: &PreparedV2) -> Vec<Value> {
    let value = serde_json::to_value(prepared).unwrap();
    value["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == "mob_type_bonus" && effect["aura"] == "Courier's Clasp")
        .cloned()
        .collect()
}

/// An Elemental Shaman wearing Courier's Clasp: the clasp's 10 spell damage holds against a Humanoid
/// target alone, as Mark of the Champion's against Undead and Demons, and raises the shaman's DPS
/// there.
#[test]
fn couriers_clasp_adds_spell_damage_against_humanoids_only() {
    let elemental = |mob_type: Option<&str>, neck: i64| {
        let request = wearing(
            read("fixtures/forever-gaps/elemental-shaman-the-elements.request.json"),
            &[(NECK, neck)],
        );
        match mob_type {
            Some(mob_type) => with_target_type(request, mob_type),
            None => request,
        }
    };
    let humanoid = run(&elemental(Some("MobTypeHumanoid"), COURIERS_CLASP));
    let bonus = clasp_bonuses(&humanoid);
    assert_eq!(bonus.len(), 1, "{bonus:?}");
    assert_eq!(bonus[0]["spell_damage"], 10.0);
    // The prepared input leaves a zero bonus out.
    assert_eq!(bonus[0]["melee_attack_power"].as_f64().unwrap_or(0.0), 0.0);
    for other in [
        elemental(Some("MobTypeUndead"), COURIERS_CLASP),
        elemental(None, COURIERS_CLASP),
    ] {
        assert!(clasp_bonuses(&run(&other)).is_empty());
    }

    let dps = |request: Value| {
        let prepared: PreparedV2 = serde_json::from_value(prepare(&request)).unwrap();
        let result = simulate_prepared(&prepared).unwrap().result;
        result["raidMetrics"]["parties"][0]["players"][0]["dps"]["avg"]
            .as_f64()
            .unwrap()
    };
    let against_humanoid = dps(elemental(Some("MobTypeHumanoid"), COURIERS_CLASP));
    let against_undead = dps(elemental(Some("MobTypeUndead"), COURIERS_CLASP));
    assert!(
        against_humanoid > against_undead,
        "Humanoid {against_humanoid} against Undead {against_undead}"
    );
}

#[test]
fn the_reference_rules_still_refuse_the_items() {
    let mut request = shaman(&[(SHOULDER, SPAULDERS_OF_CONTAINMENT)]);
    for pointer in ["/equipment/items", "/characterSheet/equipment/items"] {
        let player = &mut request["raid"]["parties"][0]["players"][0];
        for item in player.pointer_mut(pointer).unwrap().as_array_mut().unwrap() {
            if item["id"]
                .as_i64()
                .is_some_and(|id| id >= 270_000 && id != SPAULDERS_OF_CONTAINMENT)
            {
                *item = json!({});
            }
        }
    }
    match prepare_json_with(request.to_string().as_bytes(), "rust", Rules::GoReference) {
        Err(PrepareError::Refused(refusal)) => {
            let reason = refusal.to_string();
            assert!(reason.contains("no item with id 287022"), "{reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}
