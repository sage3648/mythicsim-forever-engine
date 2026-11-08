//! The Orc racial Command on a Warlock's demons since the WoW: Forever hotfixes of 6 November 2026
//! (blue post: "Fixes an issue where some Warlock pet abilities were benefitting too much from the
//! Orc Command racial ability"), under Forever's rules.
//!
//! Command (20575) in client 1.60.1.70862 (SpellEffect and SpellClassOptions, product wowf) has
//! two effects:
//! - SpellEffect 697942: aura 107 (flat modifier), misc 8 (all effect points), base 5, class set 5
//!   mask_0 0x4000000, the Tamed Pet Passive (DND) (18727 to 18730), whose only effect is aura 79
//!   (damage done percent), misc 127 (all schools), base 0. Command lifts it to 5: the demon deals
//!   5% more damage of every kind, melee included. Push 113276 re-pushes it unchanged.
//! - SpellEffect 697943: aura 108 (percent modifier), misc 0 (damage), base 5, class set 5 mask_0
//!   0x3000, Firebolt and Lash of Pain. Hotfix push 113702 (DBCache.bin of build 70862) rewrites
//!   its EffectBasePointsF to 0, so those two spells get nothing on top of the 5%.
//!
//! So an Orc's Succubus melee and Lash of Pain and an Imp's Firebolt each deal 5% more than the
//! same warlock's without its racials, in the same seeded fights, and nothing deals 10% more.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

const LASH_OF_PAIN: i64 = 11780;
const FIREBOLT: i64 = 11763;
const SHADOW_BOLT: i64 = 25307;

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2")
        .join(format!("{name}.request.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// The Affliction fixture as an Orc with `summon` out, casting Shadow Bolt alone (so no racial
/// cooldown is pressed) in a fixed two minute fight of 20 seeded iterations.
fn orc_warlock(summon: &str, racials: bool) -> Value {
    let mut request = fixture("affliction-warlock-lash-of-pain-rounding");
    request["encounter"]["duration"] = json!(120);
    request["encounter"]["durationVariation"] = json!(0);
    request["simOptions"]["iterations"] = json!(20);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    let player = player(&mut request);
    player["race"] = json!("RaceOrc");
    player["disableRacials"] = json!(!racials);
    player["warlock"]["options"]["classOptions"]["summon"] = json!(summon);
    player["rotation"] = json!({
        "type": "TypeAPL",
        "prepullActions": [],
        "priorityList": [{"action": {"castSpell": {"spellId": {"spellId": SHADOW_BOLT}}}}],
    });
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

/// Every pet action's casts and damage over all iterations, by pet name and action id.
fn pet_damage(prepared: Value) -> BTreeMap<(String, String), (f64, f64)> {
    let prepared = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    let player = &report.result["raidMetrics"]["parties"][0]["players"][0];
    let mut totals = BTreeMap::new();
    for pet in player["pets"].as_array().unwrap() {
        let name = pet["name"].as_str().unwrap().to_string();
        for action in pet["actions"].as_array().unwrap() {
            let entry = totals
                .entry((name.clone(), action["id"].to_string()))
                .or_insert((0.0, 0.0));
            for target in action["targets"].as_array().unwrap() {
                entry.0 += target["casts"].as_f64().unwrap_or(0.0);
                entry.1 += target["damage"].as_f64().unwrap_or(0.0);
            }
        }
    }
    totals.retain(|_, (_, damage)| *damage > 0.0);
    totals
}

fn spell(id: i64) -> String {
    format!(r#"{{"spellId":{id}"#)
}

/// The start of the white hits' id, whatever their tag.
fn melee() -> String {
    r#"{"otherId":"OtherActionAttack""#.to_string()
}

/// Each of the demon's damaging actions deals 5% more as an Orc, with the same casts in the
/// same seeded fights; an action whose id starts with each of `expected` must be among them.
fn assert_five_percent_on_everything(summon: &str, expected: &[String]) {
    let orc = pet_damage(prepared(&orc_warlock(summon, true), Rules::Forever));
    let base = pet_damage(prepared(&orc_warlock(summon, false), Rules::Forever));
    assert_eq!(
        orc.keys().collect::<Vec<_>>(),
        base.keys().collect::<Vec<_>>()
    );
    for id in expected {
        assert!(
            orc.keys()
                .any(|(pet, action)| pet == summon && action.starts_with(id.as_str())),
            "{summon} dealt no {id}: {orc:?}"
        );
    }
    for (key, (casts, damage)) in &orc {
        let (base_casts, base_damage) = base[key];
        assert_eq!(*casts, base_casts, "{key:?}");
        let ratio = damage / base_damage;
        // 5%, not the 10% Firebolt and Lash of Pain took before push 113702.
        assert!((ratio - 1.05).abs() < 1e-9, "{key:?}: {ratio}");
    }
}

#[test]
fn command_raises_the_succubus_melee_and_lash_of_pain_by_five_percent() {
    assert_five_percent_on_everything("Succubus", &[melee(), spell(LASH_OF_PAIN)]);
}

#[test]
fn command_raises_the_imps_firebolt_by_five_percent() {
    assert_five_percent_on_everything("Imp", &[spell(FIREBOLT)]);
}

/// The prepared state carries Command as the summoned demon's damage dealt multiplier and no spell
/// modifier: Firebolt's and Lash of Pain's own multipliers are those of a warlock without it.
#[test]
fn command_is_on_the_demons_damage_and_not_on_two_spells() {
    let additive = |prepared: &Value, id: i64| -> f64 {
        let spells: Vec<f64> = prepared["pets"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|pet| pet["spells"].as_array().unwrap().clone())
            .filter(|spell| spell["action_id"]["spell_id"] == id)
            .map(|spell| spell["damage_multiplier_additive"].as_f64().unwrap())
            .collect();
        assert_eq!(spells.len(), 1, "{id}");
        spells[0]
    };
    let multipliers = |prepared: &Value| -> Vec<f64> {
        prepared["pets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pet| {
                pet["pseudo_stats"]["damage_dealt_multiplier"]
                    .as_f64()
                    .unwrap()
            })
            .collect()
    };
    for (summon, id) in [("Succubus", LASH_OF_PAIN), ("Imp", FIREBOLT)] {
        let orc = prepared(&orc_warlock(summon, true), Rules::Forever);
        let base = prepared(&orc_warlock(summon, false), Rules::Forever);
        let (with, without) = (multipliers(&orc), multipliers(&base));
        assert!(!with.is_empty(), "no demon");
        for (with, without) in with.iter().zip(&without) {
            assert!(
                (with / without - 1.05).abs() < 1e-12,
                "{summon}: {with} vs {without}"
            );
        }
        assert_eq!(additive(&orc, id), additive(&base, id), "{summon}");
    }
}
