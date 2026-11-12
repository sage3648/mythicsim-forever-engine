//! An Enhancement Shaman the boss attacks, under Forever's rules (Redfall, 12 November 2026: the
//! shaman tanks asked for a sim with the boss swinging at them).
//!
//! Two of the shaman's auras hear the hits it takes, and the gate refused the request for both:
//!
//! - Lightning Shield. Every rank's client row (324 to 10432) has ProcFlags 139944 (the melee and
//!   ranged hits the caster takes), ProcChance 100, 3 charges and a 3.5 sec internal cooldown: a
//!   landed swing spends a charge and the rank's orb strikes the attacker. The reference's trigger
//!   hears only the shield self proc, so its orbs never fire; under Forever's rules they do.
//! - Improved Stormstrike (1223031, ProcFlags 40: melee hits taken). A dodge or parry of a swing
//!   rolls the talent's chance to finish Stormstrike's cooldown, as the reference's reset does.
//!
//! The request is MythicSim's Enhancement reference build as the app builds it (12 November
//! 2026), which does not cast Lightning Shield; the tests add the line the app adds for a shaman
//! the boss attacks.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// Lightning Shield's highest rank, which the rotation names, and that rank's orb.
const LIGHTNING_SHIELD: i64 = 10432;
const ORB: i64 = 26363;
const STORMSTRIKE: i64 = 17364;
const ITERATIONS: f64 = 300.0;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// The reference, attacked or not, with or without the app's Lightning Shield line.
fn request(attacked: bool, shield: bool) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/boss-attacks/enhancement-shaman.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"] = json!({ "iterations": ITERATIONS, "randomSeed": 7 });
    if attacked {
        player(&mut request)["inFrontOfTarget"] = json!(true);
        request["raid"]["tanks"] = json!([{ "type": "Player", "index": 0 }]);
    }
    if shield {
        let cast = json!({ "castSpell": { "spellId": { "spellId": LIGHTNING_SHIELD } } });
        let mut recast = cast.clone();
        recast["condition"] = json!({ "not": { "val": { "auraIsActive": {
            "auraId": { "spellId": LIGHTNING_SHIELD } } } } });
        let rotation = &mut player(&mut request)["rotation"];
        rotation["prepullActions"].as_array_mut().unwrap().insert(
            0,
            json!({ "action": cast, "doAtValue": { "const": { "val": "-6s" } } }),
        );
        rotation["priorityList"]
            .as_array_mut()
            .unwrap()
            .insert(0, json!({ "action": recast }));
    }
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// The request prepared under Forever's rules, through the gate and run: the player's metrics.
fn simulate(request: &Value) -> Value {
    let value = prepared(request, Rules::Forever);
    let prepared: PreparedV2 = serde_json::from_value(value).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0].clone()
}

/// A field of the player's actions with this spell id and no tag, summed over the targets, per
/// iteration.
fn per_fight(player: &Value, id: i64, field: &str) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| {
            action["id"]["spellId"].as_i64() == Some(id) && action["id"].get("tag").is_none()
        })
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target[field].as_f64().unwrap_or(0.0))
        .sum::<f64>()
        / ITERATIONS
}

fn lightning_shield(prepared: &Value) -> &Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "lightning_shield")
        .expect("Lightning Shield")
}

#[test]
fn forever_rules_export_the_orb_and_the_reference_does_not() {
    let forever = prepared(&request(true, true), Rules::Forever);
    let orb = &lightning_shield(&forever)["orb"];
    assert_eq!(orb["spell_id"], ORB);
    assert_eq!(orb["icd_ns"], 3_500_000_000_i64);
    assert_eq!(orb["can_crit"], false);
    assert!(orb["base_damage"].as_f64().unwrap() > 0.0);

    let reference = prepared(&request(true, true), Rules::GoReference);
    assert!(lightning_shield(&reference).get("orb").is_none());
}

#[test]
fn a_struck_shaman_spends_charges_on_the_boss() {
    let attacked = simulate(&request(true, true));
    let orb_damage = per_fight(&attacked, ORB, "damage");
    let orb_hits = per_fight(&attacked, ORB, "hits");
    let shields = per_fight(&attacked, LIGHTNING_SHIELD, "casts");
    assert!(orb_damage > 0.0, "no orb damage");
    // One orb per 3.5 sec at most over a fight of at most 135 sec, and three a cast.
    assert!(orb_hits <= 135.0 / 3.5 + 1.0, "{orb_hits} orbs a fight");
    assert!(
        orb_hits <= shields * 3.0 + 1e-9,
        "{orb_hits} orbs from {shields} casts"
    );
    assert!(
        shields > 2.0,
        "{shields} casts: the charges are never spent"
    );

    // Nothing strikes a shaman the boss does not attack: one cast, no orb.
    let behind = simulate(&request(false, true));
    assert_eq!(per_fight(&behind, ORB, "damage"), 0.0);
    assert_eq!(per_fight(&behind, LIGHTNING_SHIELD, "casts"), 1.0);
}

#[test]
fn dodges_and_parries_reset_stormstrike() {
    let value = prepared(&request(true, false), Rules::Forever);
    let cooldown_ns = value["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["class_spell"] == "stormstrike_cast")
        .and_then(|spell| spell["cd"]["duration_ns"].as_i64())
        .expect("Stormstrike's cooldown") as f64;
    // Without a reset no fight, at most 135 sec, holds more casts than this.
    let cap = (135e9 / cooldown_ns).floor() + 1.0;
    let attacked = simulate(&request(true, false));
    let casts = per_fight(&attacked, STORMSTRIKE, "casts");
    assert!(
        casts > cap,
        "{casts} Stormstrikes a fight, cap without resets {cap}"
    );
    let behind = simulate(&request(false, false));
    assert!(per_fight(&behind, STORMSTRIKE, "casts") <= cap);
}
