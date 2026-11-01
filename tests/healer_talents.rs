//! The healer talents past each class's heals, which Forever's rules model from the client rows
//! and tooltips (README, Rust-only mechanics): Riptide's bonus to Chain Heal, Genesis on Wild
//! Growth and Swiftmend, Oath of Light and Boundless Light. Each test puts a reference build's
//! talents on one of the healer fixtures, checks what the export carries and what the spell did
//! in the fights, and that the Go reference's rules do not read it.

use forever_engine::{check_prepared, prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The Restoration shaman reference build's talents (Riptide, Water Shield, Healing Way).
const RESTORATION_SHAMAN: &str = "-055-5531500015513151";
/// The Restoration druid reference build's, in hotfix push 113531's order: Genesis 5.
const RESTORATION_DRUID: &str = "05302001--5053025153113051";
/// The same without Genesis.
const RESTORATION_DRUID_NO_GENESIS: &str = "00302001--5053025153113051";
/// The Holy Paladin reference build's, in hotfix push 113554's order: Oath of Light, Boundless
/// Light 2.
const HOLY_PALADIN: &str = "50320215002121251-503220005002";
/// The same without Oath of Light.
const HOLY_PALADIN_NO_OATH: &str = "50320215002021251-503220005002";

const CHAIN_HEAL: i64 = 10623;
const RIPTIDE: i64 = 1239243;
const HOLY_LIGHT: i64 = 25292;
const HOLY_SHOCK_HEAL: i64 = 25903;
const WILD_GROWTH: i64 = 1238215;
const SWIFTMEND: i64 = 18562;

fn fixture(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// A healer fixture's request on these talents, with `spells` cast in order, each on the player.
fn request(name: &str, talents: &str, spells: &[i64]) -> Value {
    let mut request = fixture(&format!("fixtures/healers/{name}.request.json"));
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!(talents);
    if !spells.is_empty() {
        let list: Vec<Value> = spells
            .iter()
            .map(|id| {
                json!({"action": {"castSpell": {
                    "spellId": {"spellId": id}, "target": {"type": "Self"}
                }}})
            })
            .collect();
        player["rotation"] = json!({"type": "TypeAPL", "priorityList": list});
    }
    request["simOptions"]["iterations"] = json!(40);
    request
}

/// The Holy Paladin fixture on a 113554 talent string and its own rotation (Holy Shock, Holy
/// Light, Flash of Light).
fn paladin(talents: &str) -> Value {
    let mut request = fixture("fixtures/mage/prepared-v2/holy-paladin-healing.request.json");
    request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(talents);
    request["simOptions"]["iterations"] = json!(60);
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules).unwrap()
}

fn effect<'a>(prepared: &'a Value, kind: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

fn spell(prepared: &Value, spell_id: i64) -> &Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| {
            spell["action_id"]["spell_id"] == spell_id && spell["action_id"]["tag"].is_null()
        })
        .unwrap_or_else(|| panic!("no spell {spell_id}"))
}

/// The prepared request passes the gate; the fights' result.
fn simulate(prepared: &Value) -> Value {
    let prepared = serde_json::from_value(prepared.clone()).unwrap();
    check_prepared(&prepared).unwrap();
    simulate_prepared(&prepared).unwrap().result
}

/// What a spell did in every fight together, summed over its targets.
fn sum(result: &Value, spell: i64, key: &str) -> f64 {
    result["raidMetrics"]["parties"][0]["players"][0]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"] == spell)
        .flat_map(|action| action["targets"].as_array().unwrap().iter())
        .map(|target| target[key].as_f64().unwrap_or(0.0))
        .sum()
}

/// What a spell healed a cast, over every fight.
fn healing_a_cast(prepared: &Value, spell: i64) -> f64 {
    let result = simulate(prepared);
    sum(&result, spell, "healing") / sum(&result, spell, "casts")
}

/// Riptide (1239243): a Chain Heal cast directly on its target heals 25% more (the talent's
/// tooltip, the Riptide rows' third effect). The first player of a Chain Heal is read as the
/// Riptide holder while one is up; its jumps keep their falloff of the unboosted heal.
#[test]
fn riptide_raises_the_first_chain_heal_player() {
    let with_riptide = prepared(
        &request(
            "restoration-shaman",
            RESTORATION_SHAMAN,
            &[RIPTIDE, CHAIN_HEAL],
        ),
        Rules::Forever,
    );
    let without = prepared(
        &request("restoration-shaman", RESTORATION_SHAMAN, &[CHAIN_HEAL]),
        Rules::Forever,
    );
    let heals = effect(&with_riptide, "heals")["heals"].as_array().unwrap();
    let chain = heals
        .iter()
        .find(|heal| heal["spell_id"] == CHAIN_HEAL)
        .unwrap();
    assert_eq!(chain["hot_bonus"]["multiplier"], 1.25);
    assert_eq!(chain["hot_bonus"]["first_target_only"], true);
    assert!(chain["hot_bonus"]["spell_ids"]
        .as_array()
        .unwrap()
        .contains(&json!(RIPTIDE)));
    // Every Chain Heal rank takes it and no other heal does; without the talent no Chain Heal
    // does.
    for heal in heals {
        let chain_rank = [1064, 10622, CHAIN_HEAL].contains(&heal["spell_id"].as_i64().unwrap());
        assert_eq!(heal.get("hot_bonus").is_some(), chain_rank, "{heal}");
    }
    let no_talent = prepared(
        &request("restoration-shaman", "-055-5531500015513150", &[CHAIN_HEAL]),
        Rules::Forever,
    );
    assert!(effect(&no_talent, "heals")["heals"]
        .as_array()
        .unwrap()
        .iter()
        .all(|heal| heal.get("hot_bonus").is_none()));
    // The reference's rules, with no party and no Chain Heal reach, state none.
    let reference = prepared(
        &request(
            "restoration-shaman",
            RESTORATION_SHAMAN,
            &[RIPTIDE, CHAIN_HEAL],
        ),
        Rules::GoReference,
    );
    assert!(effect(&reference, "heals")["heals"]
        .as_array()
        .unwrap()
        .iter()
        .all(|heal| heal.get("hot_bonus").is_none()));

    // The first player is a share of a cast's players (1, then half and a quarter of it when
    // the party's damage reaches them): 25% on it is about 15% on the cast.
    let ratio = healing_a_cast(&with_riptide, CHAIN_HEAL) / healing_a_cast(&without, CHAIN_HEAL);
    assert!((1.10..1.20).contains(&ratio), "Chain Heal ratio {ratio}");
}

/// Genesis (1223081): 2% a rank since push 113531, on the periodic healing of Rejuvenation,
/// Regrowth and Wild Growth (its first effect) and on Swiftmend (its second). The reference
/// reads it on Rejuvenation and Regrowth alone.
#[test]
fn genesis_reaches_wild_growth_and_swiftmend() {
    let spells = [WILD_GROWTH, SWIFTMEND];
    let with = prepared(
        &request("restoration-druid", RESTORATION_DRUID, &spells),
        Rules::Forever,
    );
    let without = prepared(
        &request("restoration-druid", RESTORATION_DRUID_NO_GENESIS, &spells),
        Rules::Forever,
    );
    let growth = |prepared: &Value| {
        spell(prepared, WILD_GROWTH)["dot"]["periodic_damage_multiplier"]
            .as_f64()
            .unwrap()
    };
    let swiftmend = |prepared: &Value| {
        spell(prepared, SWIFTMEND)["damage_multiplier_additive"]
            .as_f64()
            .unwrap()
    };
    // Five ranks of 2% a rank, on top of Gift of Nature's 10%.
    assert!((growth(&with) / growth(&without) - 1.1).abs() < 1e-9);
    assert!((swiftmend(&with) - swiftmend(&without) - 0.1).abs() < 1e-9);
    // Regrowth and Rejuvenation keep reading it.
    let rejuvenation = |prepared: &Value| {
        spell(prepared, 25299)["dot"]["periodic_damage_multiplier"]
            .as_f64()
            .unwrap()
    };
    assert!((rejuvenation(&with) / rejuvenation(&without) - 1.1).abs() < 1e-9);
    // Wild Growth heals more with it in the fights.
    assert!(healing_a_cast(&with, WILD_GROWTH) > healing_a_cast(&without, WILD_GROWTH));

    // Under the reference's rules the druid's reference string is read in its own order, and
    // Wild Growth and Swiftmend take no Genesis.
    let reference = prepared(
        &request("restoration-druid", "05002001--5050035153113051", &spells),
        Rules::GoReference,
    );
    let reference_without = prepared(
        &request("restoration-druid", "00002001--5050035153113051", &spells),
        Rules::GoReference,
    );
    assert_eq!(growth(&reference), growth(&reference_without));
    assert_eq!(swiftmend(&reference), swiftmend(&reference_without));
}

/// Oath of Light (1328548, push 113554), in the Holy Paladin healer sim: one ally carries it for
/// the fight, and a Holy Light, Flash of Light or Holy Shock heal reaches that ally with the
/// tank's share of the party's hits. The holder's heal gains the oath's flat healing (400 for
/// Holy Light, 115, 229 for Holy Shock); a Holy Shock heal on anyone else has Boundless Light's 30%
/// crit, which the oath-less build has on every Holy Shock heal.
#[test]
fn oath_of_light_raises_the_holders_heals_and_leaves_boundless_light_the_rest() {
    let sworn = prepared(&paladin(HOLY_PALADIN), Rules::Forever);
    let oath = effect(&sworn, "paladin_heals")["oath_of_light"].clone();
    assert!((oath["holder_share"].as_f64().unwrap() - 5.0 / 9.0).abs() < 1e-12);
    assert_eq!(oath["holy_light"], 400.0);
    assert_eq!(oath["flash_of_light"], 115.0);
    assert_eq!(oath["holy_shock"], 229.0);
    assert_eq!(oath["boundless_light_crit"], 30.0);
    // The runtime applies Boundless Light to the heals the oath's holder does not take.
    assert_eq!(spell(&sworn, HOLY_SHOCK_HEAL)["bonus_crit_percent"], 0.0);

    let unsworn = prepared(&paladin(HOLY_PALADIN_NO_OATH), Rules::Forever);
    assert!(effect(&unsworn, "paladin_heals")
        .get("oath_of_light")
        .is_none());
    assert_eq!(spell(&unsworn, HOLY_SHOCK_HEAL)["bonus_crit_percent"], 30.0);

    // Holy Light heals more a cast: 5 in 9 of them gain 400 before the multipliers, on a base of
    // about 2,000 to 2,500 a cast.
    let gain = healing_a_cast(&sworn, HOLY_LIGHT) / healing_a_cast(&unsworn, HOLY_LIGHT);
    assert!((1.05..1.15).contains(&gain), "Holy Light gain {gain}");

    // Healer-only: a Retribution paladin who took the oath, and the reference's rules, read none
    // (the oath's holder is among the heals a healer's party reaches).
    let mut ret = fixture("fixtures/mage/prepared-v2/production-retribution-paladin.request.json");
    ret["raid"]["parties"][0]["players"][0]["talentsString"] =
        json!("050050002001-503-05233301001301351");
    let ret = prepared(&ret, Rules::Forever);
    assert!(ret["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == "paladin_heals")
        .all(|effect| effect.get("oath_of_light").is_none()));
    let reference = prepared(&paladin("25320213225131051-50323"), Rules::GoReference);
    assert!(effect(&reference, "paladin_heals")
        .get("oath_of_light")
        .is_none());
}
