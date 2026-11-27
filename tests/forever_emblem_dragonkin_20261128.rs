//! Draconic Infused Emblem against Dragonkin (a player report, 28 November 2026).
//!
//! The tooltip reads "Chance on harmful spellcast to increase your spell damage and healing by up
//! to 35 for 10 sec. This spell damage increase is doubled against Dragonkin." The buff row
//! (1318930) has three effects: 35 spell damage (aura 13), 35 healing (aura 135) and 35 flat spell
//! damage against Dragonkin (aura 180, mask 2). The export described only the stat aura, so a
//! wearer simming a Dragonkin got 35, not 70, while the buff held.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const MAGE: &str = "fixtures/dev-notes-20261102/arcane-mage.request.json";
const EMBLEM: i64 = 22268;
const BUFF: &str = "Draconic Infused Emblem Proc";

fn request(mob_type: Option<&str>) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(MAGE);
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"] = json!({"iterations": 2000, "randomSeed": "42"});
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": EMBLEM}));
    if let Some(mob_type) = mob_type {
        request["encounter"]["targets"][0]["mobType"] = json!(mob_type);
    }
    request
}

fn prepared(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

fn bonuses(prepared: &Value) -> Vec<&Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == "mob_type_bonus" && effect["aura"] == BUFF)
        .collect()
}

fn dps(prepared: &Value) -> f64 {
    let prepared: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0]["dps"]["avg"]
        .as_f64()
        .unwrap()
}

#[test]
fn the_emblems_buff_adds_35_spell_damage_against_a_dragonkin() {
    let dragonkin = prepared(&request(Some("MobTypeDragonkin")));
    let found = bonuses(&dragonkin);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["spell_damage"], 35.0);
    assert_eq!(found[0]["attack_power"], 0.0);
    assert_eq!(found[0]["melee_attack_power"], 0.0);
}

#[test]
fn another_creature_type_or_none_keeps_the_stat_aura_alone() {
    for mob_type in [None, Some("MobTypeUndead"), Some("MobTypeDemon")] {
        assert!(
            bonuses(&prepared(&request(mob_type))).is_empty(),
            "{mob_type:?}"
        );
    }
}

#[test]
fn a_dragonkin_gains_from_the_emblem_by_the_extra_35_alone() {
    let unknown = prepared(&request(None));
    let mut dragonkin = prepared(&request(Some("MobTypeDragonkin")));
    let with_bonus = dps(&dragonkin);
    assert!(
        with_bonus > dps(&unknown),
        "the Dragonkin's extra 35 raises the damage"
    );
    // Without the extra 35 the two fights are the same one.
    for effect in dragonkin["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "mob_type_bonus" && effect["aura"] == BUFF {
            effect["spell_damage"] = json!(0.0);
        }
    }
    assert_eq!(dps(&dragonkin), dps(&unknown));
}
