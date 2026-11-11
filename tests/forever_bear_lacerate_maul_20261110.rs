//! Lacerate's weapon hit and Maul's Omen of Clarity rate, from bear combat logs at levels 37 to
//! 41 on client 1.60.1.70862 (8 November 2026). Lacerate's hit is its own spell (414647): the
//! rank's cast is what misses, is dodged or parried, the hit is never blocked (0 of 790), and its
//! class flags lie outside Natural Instinct's, so it crits for x2.0 (147 crits) while the bleed's
//! ticks take the talent. Maul's first sample, 14 procs in 342 hits, read as half the chance of
//! every other special; a larger one on the same client (46.5 over 431 rolls, 10.8%) fits the
//! full two a minute, so the half chance is gone. Forever's rules only; the Go reference keeps
//! one Lacerate spell.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The live bear reference's talents in Forever's order, Natural Instinct 2/2.
const FERAL_BEAR: &str = "-5500033212310212551-055";
const LACERATE: i64 = 1235827;
const LACERATE_HIT: i64 = 414647;
const MAUL: i64 = 9881;

fn production(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/mage/prepared-v2/production-{name}.request.json"
    ));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn bear() -> Value {
    let mut request = production("feral-bear-druid");
    request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(FERAL_BEAR);
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn effect<'a>(prepared: &'a Value, kind: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

fn positions(prepared: &Value, spell_id: i64) -> Vec<u64> {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, spell)| {
            let id = &spell["action_id"];
            id["spell_id"] == spell_id && id["tag"].as_i64().unwrap_or(0) == 0
        })
        .map(|(i, _)| i as u64)
        .collect()
}

fn ids(values: &Value) -> Vec<u64> {
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap())
        .collect()
}

#[test]
fn lacerates_hit_is_forever_rules_only_and_maul_rolls_omen_at_the_full_rate() {
    let forever = prepared(&bear(), Rules::Forever);
    let lacerate = effect(&forever, "lacerate");
    assert_eq!(
        lacerate["hit_spell"].as_u64(),
        positions(&forever, LACERATE_HIT).first().copied()
    );
    // Maul is among Omen of Clarity's triggers and rolls as the others do: 46.5 procs over 431
    // rolls (10.8%) on client 1.60.1.70862, where the first sample of 14 in 342 read as half.
    let maul = positions(&forever, MAUL);
    assert!(!maul.is_empty());
    let omen = effect(&forever, "omen_of_clarity");
    let triggers = ids(&omen["trigger_spells"]);
    assert!(maul.iter().all(|spell| triggers.contains(spell)));
    assert!(omen.get("half_chance_spells").is_none());

    let reference = prepared(&production("feral-bear-druid"), Rules::GoReference);
    assert!(effect(&reference, "lacerate").get("hit_spell").is_none());
    assert!(positions(&reference, LACERATE_HIT).is_empty());
    assert!(effect(&reference, "omen_of_clarity")
        .get("half_chance_spells")
        .is_none());
}

fn cast(spell_id: i64, condition: Option<Value>) -> Value {
    let mut action = json!({"castSpell": {"spellId": {"spellId": spell_id}}});
    if let Some(condition) = condition {
        action["condition"] = condition;
    }
    json!({"action": action})
}

fn row(player: &Value, spell_id: i64) -> &Value {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"] == spell_id && action["id"].get("tag").is_none())
        .map(|action| &action["targets"][0])
        .unwrap_or_else(|| panic!("no {spell_id} row"))
}

fn count(target: &Value, field: &str) -> f64 {
    target.get(field).and_then(Value::as_f64).unwrap_or(0.0)
}

/// A bear that only Lacerates, in front of a target that blocks half of what it can block: the
/// hit takes the crits at x2.0 and no blocks, the cast every miss, dodge and parry.
#[test]
fn lacerates_hit_crits_for_double_and_is_never_blocked() {
    let mut request = bear();
    let player = &mut request["raid"]["parties"][0]["players"][0];
    let bear_form = json!({"not": {"val": {"auraIsActive": {"auraId": {"spellId": 9634}}}}});
    player["rotation"] = json!({"type": "TypeAPL", "prepullActions": [], "priorityList": [
        cast(9634, Some(bear_form)),
        cast(LACERATE, None),
    ]});
    request["simOptions"]["iterations"] = json!(200);
    let mut prepared = prepared(&request, Rules::Forever);
    prepared["melee"]["defender_block"] = json!(0.5);
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    let player = &report.result["raidMetrics"]["parties"][0]["players"][0];

    let hit = row(player, LACERATE_HIT);
    let cast = row(player, LACERATE);
    assert!(count(hit, "hits") > 1000.0 && count(hit, "crits") > 100.0);
    for avoided in ["blocks", "misses", "dodges", "parries"] {
        assert_eq!(count(hit, avoided), 0.0, "the hit {avoided}");
    }
    assert!(count(cast, "dodges") + count(cast, "parries") > 0.0);
    assert_eq!(count(cast, "blocks"), 0.0);

    // The same stacks under crits and hits: the average crit is twice the average hit, where
    // Natural Instinct's 2/2 would make it 2.2 times.
    let crit = count(hit, "critDamage") / count(hit, "crits");
    let normal = (count(hit, "damage") - count(hit, "critDamage")) / count(hit, "hits");
    let ratio = crit / normal;
    assert!((1.9..2.1).contains(&ratio), "{ratio}");
    // The ticks still take Natural Instinct.
    let tick_crit = count(cast, "critTickDamage") / count(cast, "critTicks");
    let tick = (count(cast, "tickDamage") - count(cast, "critTickDamage")) / count(cast, "ticks");
    assert!(tick_crit / tick > 2.1, "{}", tick_crit / tick);
}
