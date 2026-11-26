//! The armor debuff weapon procs under Forever's rules: Puncture Armor (Bashguuder and
//! Rivenspike's 17315, Digmaster 5000 and Vibroblade's 1293005) and Annihilator's Armor Shatter
//! (16928) share the target's minor armor reduction with Faerie Fire and Curse of Recklessness.
//! Public Forever logs (forever.warcraftlogs.com, 11 to 26 November 2026) show none of them
//! landing on a target under either, Faerie Fire landing taking Puncture Armor off at the same
//! timestamp, and Sunder Armor beside them (docs/mechanics.md). The Go reference stacks them
//! beside both.

use forever_engine::contracts::prepared_v2::PreparedV2;
use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// Bashguuder (Puncture Armor 17315), Digmaster 5000 (Puncture Armor 1293005) and Annihilator
/// (Armor Shatter 16928): the item, the prepared debuff's label and its spell.
const WEAPONS: [(i64, &str, i64); 3] = [
    (13204, "Puncture Armor", 17315),
    (9465, "Puncture Armor 1293005", 1293005),
    (12798, "Armor Shatter 16928", 16928),
];

/// The off hand's position in the Fury board request's compact equipment list, where it wears
/// Bashguuder.
const OFF_HAND: usize = 13;

/// The Fury board request of 26 November 2026 with the weapon in its off hand, under the raid's
/// Faerie Fire, Curse of Recklessness and Sunder Armor unless `minor` is false.
fn request(weapon: i64, minor: bool) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/board-warrior-20261126.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let off_hand = &mut request["raid"]["parties"][0]["players"][0]["equipment"]["items"][OFF_HAND];
    assert_eq!(off_hand["id"], 13204);
    *off_hand = json!({ "id": weapon });
    request["raid"]["debuffs"]["faerieFire"] = json!(minor);
    request["raid"]["debuffs"]["curseOfRecklessness"] = json!(minor);
    assert_eq!(request["raid"]["debuffs"]["sunderArmor"], true);
    request["simOptions"]["iterations"] = json!(100);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

/// The weapon's armor debuff proc.
fn armor_debuff<'a>(prepared: &'a Value, aura: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "armor_debuff_proc" && effect["aura"] == aura)
        .unwrap_or_else(|| panic!("the {aura} proc"))
}

fn blocked_by(prepared: &Value, aura: &str) -> Vec<String> {
    armor_debuff(prepared, aura)["blocked_by"]
        .as_array()
        .map(|labels| {
            labels
                .iter()
                .map(|label| label.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// The seconds a fight the target carried the debuff.
fn uptime(prepared: Value, spell_id: i64) -> f64 {
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let result = forever_engine::simulate_prepared(&prepared).unwrap().result;
    result["encounterMetrics"]["targets"][0]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|aura| aura["id"]["spellId"] == spell_id)
        .map(|aura| aura["uptimeSecondsAvg"].as_f64().unwrap_or(0.0))
        .fold(0.0, f64::max)
}

/// Under Faerie Fire and Curse of Recklessness none of the three debuffs ever lands.
#[test]
fn faerie_fire_and_curse_of_recklessness_block_the_armor_debuff_procs() {
    for (weapon, aura, spell_id) in WEAPONS {
        let prepared = prepared(&request(weapon, true), Rules::Forever);
        let blockers = blocked_by(&prepared, aura);
        assert!(
            blockers
                .iter()
                .any(|label| label.starts_with("Faerie Fire")),
            "{aura}: {blockers:?}"
        );
        assert!(
            blockers
                .iter()
                .any(|label| label.starts_with("Curse of Recklessness")),
            "{aura}: {blockers:?}"
        );
        assert!(!blockers.iter().any(|label| label.starts_with("Sunder")));
        assert_eq!(uptime(prepared, spell_id), 0.0, "{aura}");
    }
}

/// Without either one, each debuff lands beside Sunder Armor as before.
#[test]
fn the_armor_debuff_procs_land_beside_sunder_armor() {
    for (weapon, aura, spell_id) in WEAPONS {
        let prepared = prepared(&request(weapon, false), Rules::Forever);
        assert!(blocked_by(&prepared, aura).is_empty(), "{aura}");
        let seconds = uptime(prepared, spell_id);
        assert!(seconds > 10.0, "{aura}: {seconds}");
    }
}
