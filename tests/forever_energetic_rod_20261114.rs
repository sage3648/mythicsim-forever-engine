//! Energetic Rod (18321), a main hand mace of item level 59 from Dire Maul East (+5 Intellect, 68
//! spell damage and healing): "Equip: Chance on landing a damaging spell to restore 48 mana to
//! you." A player asked on 14 November 2026 whether it procs off Consecration and what its
//! internal cooldown is; until then the sim wore it for its stats alone.
//!
//! The client rows (wago.tools `product=wowf`, build 1.60.1.70938, 14 November 2026): Demonic
//! Energy (1302190), SpellAuraOptions 246697: ProcChance 100, ProcCategoryRecovery 0, no
//! SpellProcsPerMinute, ProcTypeMask 65536 (harmful spells dealt); it casts 1302191, an Energize
//! of 48 on mana. Read literally under the owner's decision of 13 November 2026, every landed
//! damaging spell restores 48 mana, with no internal cooldown. Forever's Consecration ticks
//! (1280345 to 1280349) carry the treat as periodic attribute, which the mask does not name, and
//! the cast lands on no unit, so Consecration restores nothing.
//!
//! The tests dress fixture characters: the Arcane Mage of `fixtures/dev-notes-20261102`, the
//! Protection Paladin of `fixtures/forever-gaps` (Quel'Serrar) who casts Consecration, and the Combat Rogue of
//! `fixtures/forever-gaps`, who has no mana bar.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/rogue_113483.rs"]
mod rogue_113483;

const ENERGETIC_ROD: i64 = 18321;
const DEMONIC_ENERGY_MANA: i64 = 1302191;
const CONSECRATION_RANK_5: i64 = 20924;
const MAIN_HAND: usize = 14;
/// A spell batch window, the delay of an item proc's handler.
const SPELL_BATCH_WINDOW_NS: i64 = 10_000_000;

const MAGE: &str = "fixtures/dev-notes-20261102/arcane-mage.request.json";
const PALADIN: &str = "fixtures/forever-gaps/protection-paladin-quel-serrar.request.json";
const ROGUE: &str = "fixtures/forever-gaps/combat-rogue-self-casts-at-player.request.json";

/// A fixture's request with the rod in the main hand.
fn wielding_the_rod(fixture: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.resize(17, json!({}));
    equipment[MAIN_HAND] = json!({ "id": ENERGETIC_ROD });
    if fixture == ROGUE {
        rogue_113483::forever_request(&mut request);
    }
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// A prepared input through the gate and run: the player's metrics.
fn simulate(value: &Value) -> Value {
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    let player = result["raidMetrics"]["parties"][0]["players"][0].clone();
    assert!(player["dps"]["avg"].as_f64().unwrap() > 0.0);
    player
}

fn rod_effect(prepared: &Value) -> Option<&Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "mana_proc" && effect["trigger_aura"] == "Energetic Rod")
}

fn has_aura(prepared: &Value, label: &str) -> bool {
    prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .any(|aura| aura["label"] == label)
}

/// The spell ids of the spellbook positions the proc hears.
fn heard_spell_ids(prepared: &Value, effect: &Value) -> Vec<i64> {
    effect["chances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let position = entry["spell"].as_u64().unwrap() as usize;
            prepared["player"]["spells"][position]["action_id"]["spell_id"]
                .as_i64()
                .unwrap_or(0)
        })
        .collect()
}

/// The gains of mana a metrics list holds for an action with this spell id: (count, total).
fn mana_gains(player: &Value, id: i64) -> (f64, f64) {
    let mut count = 0.0;
    let mut total = 0.0;
    for resource in player["resources"].as_array().unwrap() {
        if resource["id"]["spellId"].as_i64() == Some(id) {
            count += resource["events"].as_f64().unwrap_or(0.0);
            total += resource["gain"].as_f64().unwrap_or(0.0);
        }
    }
    (count, total)
}

/// A field of the player's actions with these spell ids, summed over every tag and target.
fn total(player: &Value, ids: &[i64], field: &str) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| {
            action["id"]["spellId"]
                .as_i64()
                .is_some_and(|id| ids.contains(&id))
        })
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target[field].as_f64().unwrap_or(0.0))
        .sum()
}

/// The proc is the row's: every landed hit of a damaging spell, a chance of 1, 48 mana a spell
/// batch window later, and no internal cooldown.
#[test]
fn the_rod_restores_48_mana_on_every_landed_damaging_spell() {
    let request = wielding_the_rod(MAGE);
    let prepared = prepared(&request, Rules::Forever);
    let effect = rod_effect(&prepared).expect("the rod's mana proc");
    assert_eq!(effect["mana"], 48.0);
    assert_eq!(effect["metrics_action_id"]["spell_id"], DEMONIC_ENERGY_MANA);
    assert_eq!(effect["delay_ns"], SPELL_BATCH_WINDOW_NS);
    assert!(effect.get("struck").is_none());
    let chances = effect["chances"].as_array().unwrap();
    assert!(!chances.is_empty());
    assert!(
        chances.iter().all(|entry| entry["chance"] == 1.0),
        "{effect}"
    );
    let trigger = prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == "Energetic Rod")
        .expect("the rod's trigger aura");
    assert!(trigger["icd"].is_null(), "{trigger}");

    let player = simulate(&prepared);
    let (count, mana) = mana_gains(&player, DEMONIC_ENERGY_MANA);
    assert!(count > 0.0);
    assert_eq!(mana, 48.0 * count);
    // One gain for every landed hit of a spell it hears, and none for a miss. The metrics count an
    // Arcane Missiles missile when it is fired, and the last one or two of a fight are still in
    // flight when it ends: they never land and restore nothing.
    let heard = heard_spell_ids(&prepared, effect);
    let landed = total(&player, &heard, "hits") + total(&player, &heard, "crits");
    assert!(total(&player, &heard, "misses") > 0.0);
    let iterations = request["simOptions"]["iterations"].as_f64().unwrap();
    assert!(
        count <= landed && count >= landed - 2.0 * iterations,
        "{count} gains, {landed} landed"
    );

    // Without the rod the mage gains none of it.
    let mut plain = request.clone();
    plain["raid"]["parties"][0]["players"][0]["equipment"]["items"][MAIN_HAND] = json!({});
    let plain = simulate(&self::prepared(&plain, Rules::Forever));
    assert_eq!(mana_gains(&plain, DEMONIC_ENERGY_MANA), (0.0, 0.0));
}

/// Consecration's cast lands on no unit and its ticks are periodic damage: the proc does not
/// hear it, nor a judgement, which is a ranged attack under Forever's rules.
#[test]
fn consecration_restores_no_mana() {
    let prepared = prepared(&wielding_the_rod(PALADIN), Rules::Forever);
    let effect = rod_effect(&prepared).expect("the rod's mana proc");
    let heard = heard_spell_ids(&prepared, effect);
    assert!(!heard.contains(&CONSECRATION_RANK_5), "{heard:?}");
    let spells = prepared["player"]["spells"].as_array().unwrap();
    assert!(
        spells
            .iter()
            .any(|spell| spell["action_id"]["spell_id"] == CONSECRATION_RANK_5),
        "the paladin casts Consecration"
    );
    let player = simulate(&prepared);
    assert!(total(&player, &[CONSECRATION_RANK_5], "ticks") > 0.0);
    let (count, _) = mana_gains(&player, DEMONIC_ENERGY_MANA);
    let landed = total(&player, &heard, "hits") + total(&player, &heard, "crits");
    assert_eq!(count, landed, "{heard:?}");
}

/// A wearer with no mana bar gains nothing and wears the rod as a plain mace.
#[test]
fn the_rod_is_plain_without_a_mana_bar() {
    let prepared = prepared(&wielding_the_rod(ROGUE), Rules::Forever);
    assert!(!has_aura(&prepared, "Energetic Rod"));
    assert!(rod_effect(&prepared).is_none());
    simulate(&prepared);
}
