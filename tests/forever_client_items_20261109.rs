//! Items the Forever client adds whose ItemSparse rows wago.tools cannot read (the client ships
//! them encrypted), rerun from Wowhead's Forever tooltips of 9 November 2026
//! (`tools/rust_client_items.py`, `data/forever-client-items.jsonl`): 71 more items whose
//! tooltips Wowhead decoded since 8 November, 14 of them with an "Attack Power against
//! <creature type>" line (`data/forever-client-item-bonuses.jsonl`).
//!
//! Nama's Cherished Necklace (279893), a tooltip of 9 November: Neck, Item Level 57, +9 Stamina,
//! "Equip: +12 Attack Power." and "Equip: +19 Attack Power against Humanoids."

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const NAMAS_NECKLACE: i64 = 279893;
const NECK: usize = 1;
const MAIN_HAND: usize = 14;
const OFF_HAND: usize = 15;
const RANGED: usize = 16;

fn read(name: &str) -> Vec<u8> {
    fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(name)).unwrap()
}

/// The level 40 Enhancement Shaman of the Ceremonial Mallet's refused request, without buffs and
/// with 20 iterations.
fn shaman() -> Value {
    let mut request: Value = serde_json::from_slice(&read(
        "fixtures/forever-gaps/enhancement-shaman-ceremonial-mallet.request.json",
    ))
    .unwrap();
    request["simOptions"]["iterations"] = json!(20);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["buffs"] = json!({});
    let items = player["equipment"]["items"].as_array_mut().unwrap();
    items.resize(RANGED + 1, json!({}));
    request["raid"]["buffs"] = json!({});
    request["raid"]["parties"][0]["buffs"] = json!({});
    request
}

fn items(request: &mut Value) -> &mut Vec<Value> {
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap()
}

/// The slot of the request's equipment a client row is worn in.
fn slot(row: &Value) -> usize {
    match row["type"].as_str().unwrap() {
        "ItemTypeHead" => 0,
        "ItemTypeNeck" => NECK,
        "ItemTypeShoulder" => 2,
        "ItemTypeBack" => 3,
        "ItemTypeChest" => 4,
        "ItemTypeWrist" => 5,
        "ItemTypeHands" => 6,
        "ItemTypeWaist" => 7,
        "ItemTypeLegs" => 8,
        "ItemTypeFeet" => 9,
        "ItemTypeFinger" => 10,
        "ItemTypeTrinket" => 12,
        "ItemTypeRanged" => RANGED,
        "ItemTypeWeapon" if row["hand_type"] == "HandTypeOffHand" => OFF_HAND,
        "ItemTypeWeapon" => MAIN_HAND,
        other => panic!("item type {other}"),
    }
}

/// The shaman wearing the item in its slot: an off hand item with the main hand empty, a main
/// hand weapon with the off hand empty.
fn wearing(row: &Value) -> Value {
    let mut request = shaman();
    let slot = slot(row);
    let items = items(&mut request);
    match slot {
        OFF_HAND => items[MAIN_HAND] = json!({}),
        MAIN_HAND => items[OFF_HAND] = json!({}),
        _ => {}
    }
    items[slot] = json!({ "id": row["id"] });
    request
}

fn prepared(request: &Value) -> Result<PreparedV2, String> {
    let value = prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .map_err(|err| err.to_string())?;
    let prepared: PreparedV2 = serde_json::from_value(value).map_err(|err| err.to_string())?;
    check_prepared(&prepared).map_err(|err| format!("{err:?}"))?;
    Ok(prepared)
}

/// Items whose effect is refused by name on the shaman, a character with a mana bar: Farraki
/// Kris's chance on harmful spell cast has no stated rate
/// (`forever_effect_items::SPELL_CAST_EFFECTS_NOT_PREPARED`).
const REFUSED_ON_THE_SHAMAN: [i64; 1] = [274647];

/// Every Forever item read from its tooltip alone (ids from 270000) prepares in its slot and
/// passes the gate, the 71 added on 9 November among them, but for those refused by name on the
/// shaman, which are refused for their effect and nothing else.
#[test]
fn every_forever_client_item_prepares_in_its_slot() {
    let text = String::from_utf8(read("data/forever-client-items.jsonl")).unwrap();
    let rows: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|row: &Value| row["id"].as_i64().unwrap() >= 270_000)
        .collect();
    assert!(rows.len() >= 230, "{} rows", rows.len());
    for id in REFUSED_ON_THE_SHAMAN {
        let row = rows.iter().find(|row| row["id"] == id).unwrap();
        let err = prepared(&wearing(row)).err().unwrap_or_default();
        assert!(
            err.contains(&format!("item {id} has an effect not prepared yet")),
            "{id}: {err}"
        );
    }
    let refused: Vec<String> = rows
        .iter()
        .filter(|row| !REFUSED_ON_THE_SHAMAN.contains(&row["id"].as_i64().unwrap()))
        .filter_map(|row| {
            prepared(&wearing(row))
                .err()
                .map(|err| format!("{} {}: {err}", row["id"], row["name"]))
        })
        .collect();
    assert!(refused.is_empty(), "{refused:#?}");
}

fn with_target_type(mut request: Value, mob_type: &str) -> Value {
    request["encounter"]["targets"][0]["mobType"] = json!(mob_type);
    request
}

fn with_neck(id: i64) -> Value {
    let mut request = shaman();
    items(&mut request)[NECK] = if id == 0 {
        json!({})
    } else {
        json!({ "id": id })
    };
    request
}

/// The necklace's creature type bonuses (the shaman's Ceremonial Mallet has its own, against
/// Undead).
fn necklace_bonuses(prepared: &PreparedV2) -> Vec<Value> {
    let value = serde_json::to_value(prepared).unwrap();
    value["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| {
            effect["kind"] == "mob_type_bonus" && effect["aura"] == "Nama's Cherished Necklace"
        })
        .cloned()
        .collect()
}

/// The tooltip's plain stats are the row's: 9 Stamina and 12 attack power.
#[test]
fn namas_necklace_carries_the_tooltips_stats() {
    let stats = |request: Value| {
        let value = prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
            .unwrap_or_else(|err| panic!("{err}"));
        let stat = |name: &str| value["player"]["stats"][name].as_f64().unwrap();
        (stat("Stamina"), stat("AttackPower"))
    };
    let (worn, bare) = (stats(with_neck(NAMAS_NECKLACE)), stats(with_neck(0)));
    assert_eq!(worn.0 - bare.0, 9.0, "Stamina");
    assert_eq!(worn.1 - bare.1, 12.0, "Attack Power");
}

/// "+19 Attack Power against Humanoids" is melee attack power on a Humanoid target alone, as the
/// Ceremonial Mallet's line against Undead; a target of another type or of none takes nothing.
#[test]
fn namas_necklace_bonus_applies_against_humanoids_only() {
    let humanoid = prepared(&with_target_type(
        with_neck(NAMAS_NECKLACE),
        "MobTypeHumanoid",
    ))
    .unwrap_or_else(|err| panic!("{err}"));
    let bonus = necklace_bonuses(&humanoid);
    assert_eq!(bonus.len(), 1, "{bonus:?}");
    assert_eq!(bonus[0]["melee_attack_power"], 19.0);
    simulate_prepared(&humanoid).unwrap();

    for other in [
        with_target_type(with_neck(NAMAS_NECKLACE), "MobTypeUndead"),
        with_neck(NAMAS_NECKLACE),
    ] {
        let prepared = prepared(&other).unwrap_or_else(|err| panic!("{err}"));
        assert!(necklace_bonuses(&prepared).is_empty());
    }
}

/// The shaman with its other Forever client items taken off (in its equipment and its character
/// sheet's), so the necklace is the one item the reference lacks.
#[test]
fn the_reference_rules_still_refuse_namas_necklace() {
    let mut request = with_neck(NAMAS_NECKLACE);
    for pointer in ["/equipment/items", "/characterSheet/equipment/items"] {
        let player = &mut request["raid"]["parties"][0]["players"][0];
        for item in player.pointer_mut(pointer).unwrap().as_array_mut().unwrap() {
            if item["id"].as_i64().is_some_and(|id| id >= 270_000) {
                *item = json!({});
            }
        }
    }
    items(&mut request)[NECK] = json!({ "id": NAMAS_NECKLACE });
    match prepare_json_with(request.to_string().as_bytes(), "rust", Rules::GoReference) {
        Err(PrepareError::Refused(refusal)) => {
            let reason = refusal.to_string();
            assert!(reason.contains("no item with id 279893"), "{reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}
