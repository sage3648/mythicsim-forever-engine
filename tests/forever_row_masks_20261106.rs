//! Spell-mask gaps closed against the live client rows (wago.tools `product=wowf` and Wowhead's
//! Forever tooltips, 6 November 2026), under Forever's rules only. The reference keeps its masks.

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn request(fixture: &str, talents: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2")
        .join(format!("{fixture}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["raid"]["parties"][0]["players"][0]["talentsString"] = Value::from(talents);
    request["simOptions"]["iterations"] = json!(20);
    serde_json::to_vec(&request).unwrap()
}

fn prepared(fixture: &str, talents: &str, rules: Rules) -> Value {
    prepare_json_with(&request(fixture, talents), "rust", rules)
        .unwrap_or_else(|err| panic!("{talents}: {err:?}"))
}

fn spell_named(spells: &Value, spell_id: i64) -> &Value {
    spells
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| {
            spell["action_id"]["spell_id"] == spell_id && spell["action_id"]["tag"].is_null()
        })
        .unwrap_or_else(|| panic!("no spell {spell_id}"))
}

const AFFLICTION: &str = "production-affliction-warlock";
const HOLY: &str = "holy-paladin-healing";
const LASH_OF_PAIN: i64 = 11780;
const LAY_ON_HANDS: i64 = 10310;
const HOLY_LIGHT: i64 = 25292;

/// Affliction 0552032013520005 holds one point of Malevolence.
const MALEVOLENCE_BUILD: &str = "0552032013520005--0550005003";
/// The same tree with no Malevolence point.
const NO_MALEVOLENCE_BUILD: &str = "0552032013020005--0550005003";
/// Holy to Holy Power 5/5 in push 113554's tree (the first four rows).
const HOLY_POWER_BUILD: &str = "0502521022102125";
/// Holy to Holy Power 5/5 in the reference's tree.
const REFERENCE_HOLY_BUILD: &str = "25320213225131051-50323";

/// Malevolence (wowf SpellEffect on 1310949): EffectSpellClassMask word 0 is 550027, bit 13 of
/// which (8192) is the SpellClassMask of every Lash of Pain rank (family 5, 11780). Under
/// Forever's rules the Succubus' Lash of Pain takes the crit; the reference's does not.
#[test]
fn malevolence_adds_crit_to_the_succubus_lash_of_pain() {
    let lash = |rules, talents| {
        let prepared = prepared(AFFLICTION, talents, rules);
        let bonus = spell_named(&prepared["pets"][0]["spells"], LASH_OF_PAIN)["bonus_crit_percent"]
            .as_f64()
            .unwrap();
        bonus
    };
    assert_eq!(lash(Rules::Forever, MALEVOLENCE_BUILD), 5.0);
    assert_eq!(lash(Rules::GoReference, MALEVOLENCE_BUILD), 0.0);
    assert_eq!(lash(Rules::Forever, NO_MALEVOLENCE_BUILD), 0.0);
}

/// The warlock's own shadow spells keep the same bonus under both rules: the new mod rides on
/// the demon and changes nothing the warlock casts.
#[test]
fn malevolence_still_reaches_the_warlocks_shadow_spells() {
    let shadow_bolt = |rules| {
        let prepared = prepared(AFFLICTION, MALEVOLENCE_BUILD, rules);
        spell_named(&prepared["player"]["spells"], 25307)["bonus_crit_percent"]
            .as_f64()
            .unwrap()
    };
    assert_eq!(shadow_bolt(Rules::Forever), shadow_bolt(Rules::GoReference));
    assert!(shadow_bolt(Rules::Forever) >= 5.0);
}

/// Holy Power (wowf SpellEffect 683158) keeps its class mask: word 0 is 33612840, whose bit 15
/// (32768) is the SpellClassMask of every Lay on Hands rank (family 10, 633). Under Forever's
/// rules the heal crit bonus reaches Lay on Hands like Holy Light.
#[test]
fn holy_power_heal_crit_reaches_lay_on_hands() {
    let forever = prepared(HOLY, HOLY_POWER_BUILD, Rules::Forever);
    let spells = &forever["player"]["spells"];
    assert_eq!(spell_named(spells, HOLY_LIGHT)["crit_multiplier_pct"], 1.5);
    assert_eq!(
        spell_named(spells, LAY_ON_HANDS)["crit_multiplier_pct"],
        1.5
    );
    assert_eq!(spell_named(spells, LAY_ON_HANDS)["bonus_crit_percent"], 0.0);
}

/// The reference's Holy Power is crit chance and already named Lay on Hands; nothing moves.
#[test]
fn the_reference_holy_power_is_unchanged() {
    let reference = prepared(HOLY, REFERENCE_HOLY_BUILD, Rules::GoReference);
    let spells = &reference["player"]["spells"];
    assert_eq!(spell_named(spells, LAY_ON_HANDS)["bonus_crit_percent"], 5.0);
    assert_eq!(
        spell_named(spells, LAY_ON_HANDS)["crit_multiplier_pct"],
        1.0
    );
}
