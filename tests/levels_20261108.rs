//! Levelling characters (docs/levels.md, 8 November 2026).
//!
//! The fixtures are MythicSim's levelling requests for three reference builds, written by the
//! app's `TestDumpLevelledRequests`: the build's gear, its talents trimmed to the level's points,
//! no group buffs, and a character sheet consistent with the gear. They prepare at their level,
//! pass the gate, cast only ranks the level teaches and match their sheet's health and mana.

use forever_engine::contracts::prepared_v2::PreparedV2;
use forever_engine::{
    prepare_json_with, prepared_refusals, simulate_prepared, PrepareError, Rules,
};
use serde_json::Value;
use std::{fs, path::Path};

fn request(name: &str) -> Value {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/levels/{name}.request.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn prepare(request: &Value) -> Result<Value, PrepareError> {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// The DPS a prepared request simulates, after the gate has passed it.
fn dps(prepared: &Value) -> f64 {
    let typed: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    let refusals = prepared_refusals(&typed);
    assert!(refusals.is_empty(), "gate refusals: {refusals:?}");
    let report = simulate_prepared(&typed).unwrap();
    report.result["raidMetrics"]["parties"][0]["players"][0]["dps"]["avg"]
        .as_f64()
        .unwrap_or(0.0)
}

/// The player's spells the rotation can press, by spell id.
fn pressable(prepared: &Value) -> Vec<i64> {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|spell| {
            spell["flags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|flag| flag == "SpellFlagAPL")
        })
        .filter_map(|spell| spell["action_id"]["spell_id"].as_i64())
        .collect()
}

#[test]
fn levelling_characters_sim_at_their_level() {
    for name in ["frost-mage-L20", "combat-rogue-L30", "hunter-L20"] {
        let request = request(name);
        let prepared = prepare(&request).unwrap_or_else(|err| panic!("{name}: {err}"));
        let level = request["raid"]["parties"][0]["players"][0]["level"].as_i64();
        assert_eq!(prepared["player"]["level"].as_i64(), level, "{name}");
        let dps = dps(&prepared);
        assert!(dps > 50.0, "{name} does {dps} DPS");
    }
}

/// Frostbolt rank 4 (7322) is learned at 20 and rank 5 (8406) at 26: a level 20 mage presses
/// rank 4, and the rotation's top rank (25304) is pointed at it.
#[test]
fn a_level_20_mage_presses_the_frostbolt_it_knows() {
    let prepared = prepare(&request("frost-mage-L20")).unwrap();
    let spells = pressable(&prepared);
    assert!(spells.contains(&7322), "{spells:?}");
    for unknown in [8406, 25304] {
        assert!(!spells.contains(&unknown), "{unknown} in {spells:?}");
    }
    let rotation = prepared["player"]["rotation"].to_string();
    assert!(
        !rotation.contains("25304"),
        "the rotation still names 25304"
    );
}

/// The sheet's health and mana are the character's, buffs and all: the calibration solves the
/// level's base health and mana so the engine's sheet is the exported one, less what the sheet's
/// extra attributes give (10 health a point of Stamina, 15 mana a point of Intellect). The
/// sheet was read with buffs up; the engine sims the character solo.
#[test]
fn the_sheet_sets_health_and_mana() {
    let request = request("frost-mage-L20");
    let sheet = request["raid"]["parties"][0]["players"][0]["characterSheet"].clone();
    let prepared = prepare(&request).unwrap();
    let stats = &prepared["player"]["stats"];
    for (stat, field, attribute, index, per_point) in [
        ("Health", "health", "Stamina", 2, 10.0),
        ("Mana", "mana", "Intellect", 3, 15.0),
    ] {
        let engine = stats[stat].as_f64().unwrap();
        let extra =
            sheet["attributes"][index].as_f64().unwrap() - stats[attribute].as_f64().unwrap();
        let want = sheet[field].as_f64().unwrap() - per_point * extra;
        assert!(
            (engine - want).abs() < 1.0,
            "{stat}: {engine} against {want} (the sheet's {} less {per_point} for each of its {extra} extra {attribute})",
            sheet[field]
        );
    }
}

#[test]
fn a_levelling_character_needs_its_sheet_and_ten_levels() {
    let mut without_sheet = request("frost-mage-L20");
    player(&mut without_sheet)
        .as_object_mut()
        .unwrap()
        .remove("characterSheet");
    let mut level_five = request("frost-mage-L20");
    player(&mut level_five)["level"] = 5.into();
    for (request, says) in [(without_sheet, "character sheet"), (level_five, "level 5")] {
        match prepare(&request) {
            Err(PrepareError::Refused(refusal)) => {
                assert_eq!(refusal.code, "level_unsupported");
                assert!(refusal.reason.contains(says), "{}", refusal.reason);
            }
            other => panic!("not refused: {:?}", other.map(|_| ())),
        }
    }
}

/// A Best Gear or Droptimizer set swaps gear into the request while the sheet still describes
/// the character's own. Solved against the swapped gear, the sheet would absorb the swap's own
/// crit; solved against the gear it names (`characterSheet.equipment`), taking off Robe of the
/// Archmage (14152: 1% spell crit and 16 Intellect) costs that crit and the Intellect's.
#[test]
fn a_swapped_item_keeps_its_crit() {
    let own = request("frost-mage-L20");
    let items = own["raid"]["parties"][0]["players"][0]["equipment"]["items"].clone();
    let spell_crit = |request: &Value| {
        prepare(request).unwrap()["player"]["stats"]["SpellCritPercent"]
            .as_f64()
            .unwrap()
    };
    let mut without_robe = own.clone();
    player(&mut without_robe)["equipment"]["items"] = Value::Array(
        items
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["id"] != 14152)
            .cloned()
            .collect(),
    );
    player(&mut without_robe)["characterSheet"]["equipment"] = serde_json::json!({"items": items});
    let lost = spell_crit(&own) - spell_crit(&without_robe);
    assert!(
        lost > 1.2 && lost < 1.6,
        "the robe was worth {lost}% spell crit"
    );
}

/// A class's own raid buff is the rank the levelling character knows: Arcane Brilliance is a
/// level 56 book, so a level 20 mage buffs with Arcane Intellect rank 2 (1460, learned at 14),
/// not the raid's +31 Intellect.
#[test]
fn a_levelling_class_buffs_with_the_rank_it_knows() {
    let prepared = prepare(&request("frost-mage-L20")).unwrap();
    let brilliance = prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == "Arcane Brilliance (External)")
        .expect("the mage's own raid buff");
    assert_eq!(brilliance["action_id"]["spell_id"], 1460);
}

/// A levelling Retribution Paladin's Best Gear set with a two-hander from the bags opens with the
/// Manual Crowd Pummeler and swaps back to that two-hander, while the sheet was read in a
/// one-hander and shield (Outlaw Sabre 16886, Commander's Crest 6320). The calibration prepares
/// the request in the sheet's gear; with the swap set still on, the two-hander swap would take
/// the sheet's shield off too and the request was refused ("an item swap of a slot other than
/// the main hand alone", nine Best Gear and Droptimizer sims on 8 November 2026). The set sims
/// with the opener: one Pummeler swap back to the two-hander.
#[test]
fn a_two_hander_set_opens_with_the_pummeler_beside_a_sheet_shield() {
    let request = request("retribution-paladin-L29-two-hander");
    let sheet_gear = &request["raid"]["parties"][0]["players"][0]["characterSheet"]["equipment"];
    assert!(
        sheet_gear.to_string().contains("6320"),
        "the sheet holds the shield"
    );
    assert_eq!(
        request["raid"]["parties"][0]["players"][0]["itemSwap"]["items"][14]["id"], 251533,
        "the set's two-hander, Forsaken Greataxe, is the swap set's main hand"
    );
    let prepared = prepare(&request).unwrap();
    assert!(dps(&prepared) > 0.0);
}
