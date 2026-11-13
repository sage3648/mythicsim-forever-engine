//! Forever client items that refused real players' sims on 13 November 2026 ("no item with id
//! N"), and two more rows a run of the item tools over the whole candidate list found, from
//! Wowhead's Forever tooltips and the client rows wago.tools serves (`product=wowf`, build
//! 1.60.1.70938) on 14 November 2026:
//!
//! - Lucky Shovelnose Tuft (280060), trinket, item level 60: +11 Strength, +11 Spirit and "Use:
//!   Increases your chance to Dodge by 5% for 15 sec. (5 Min Cooldown)". ItemEffect 237439 is a
//!   use of Lucky Shovelnose Snout (1315844) on 300000 ms and 15000 ms of category 2554;
//!   SpellEffect 1348362 is a dodge percent aura (49) of 5, SpellMisc 862027 lasts 15 sec
//!   (DurationIndex 8). The use is a survival cooldown of 60 Dodge rating.
//! - Flamebreak Talisman (280071), trinket, item level 61: +17 Fire Resistance and "Use: Increase
//!   your agility by 33 for 30 sec. If there is an elemental nearby, gain an additional 33. (5 Min
//!   Cooldown)". ItemEffect 236650 is a use of Flamebreak Talisman (1315878) on 300000 ms and
//!   30000 ms of category 1141; SpellEffect 1348442 and 1348471 are two Agility auras (29) of 33
//!   on the wearer with no condition in any row, and SpellMisc 862061 lasts 30 sec. The use
//!   grants the first 33 alone: no row states when the second applies.
//! - Tunnelweb Pincer (280085), one-hand dagger, item level 57: 42 to 78, speed 1.60, +7 Agility,
//!   +8 Stamina and "Equip: Improves your chance to get a critical strike by 0.4%." (5.6 crit
//!   rating, 14 a percent).
//! - Egan's Blaster (13289), a quest item of the ranged slot (Item class 12, InventoryType 26),
//!   item level 1: "Use to free Spectral and Ghostly Citizens." (spell 17368, ItemEffect 99465).
//!   No damage, speed or stats: wearing it is wearing nothing in that slot. A Hunter, whose shots
//!   need a ranged weapon, is refused by name.
//! - Matriarch's Prowlers (286616), leather feet, item level 52: 94 Armor, +11 Agility and
//!   "Equip: Increases Feral Combat +1." (ItemEffect 238174, the equip spell 1320726 Spiderchitin
//!   Wrap carries).
//! - Berdun's Trusty Rifle (290333), gun, item level 48: 47 to 87, speed 3.00, +6 Strength.
//!
//! No refused request was at hand, so the tests dress fixture characters.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/rogue_113483.rs"]
mod rogue_113483;

const LUCKY_SHOVELNOSE_TUFT: i64 = 280060;
const FLAMEBREAK_TALISMAN: i64 = 280071;
const TUNNELWEB_PINCER: i64 = 280085;
const EGANS_BLASTER: i64 = 13289;
const MATRIARCHS_PROWLERS: i64 = 286616;
const BERDUNS_TRUSTY_RIFLE: i64 = 290333;

/// The fixtures' equipment lists hold feet at 9 and the first trinket at 12.
const FEET: usize = 9;
const TRINKET_1: usize = 12;
/// The Retribution Paladin's libram and the Survival Hunter's main hand and ranged weapon.
const PALADIN_RANGED: usize = 15;
const HUNTER_MAIN_HAND: usize = 14;
const HUNTER_RANGED: usize = 16;

const SECOND: i64 = 1_000_000_000;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// A fixture's request (a path under `fixtures/`) with these items in these positions and no
/// consumables.
fn wearing(fixture: &str, items: &[(usize, Value)]) -> Value {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/{fixture}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    let player = player(&mut request);
    player["consumables"] = json!({});
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.resize(17, json!({}));
    for (slot, item) in items {
        equipment[*slot] = item.clone();
    }
    request
}

/// The Retribution Paladin, whom no target attacks.
fn paladin(items: &[(usize, Value)]) -> Value {
    wearing(
        "paladin/retribution-paladin-unsealed-wrath-command-echoes",
        items,
    )
}

/// The Combat Rogue. The fixture's talents are read in the trees Forever's rules prepare.
fn rogue(items: &[(usize, Value)]) -> Value {
    let mut request = wearing("forever-gaps/combat-rogue-self-casts-at-player", items);
    rogue_113483::forever_request(&mut request);
    request
}

/// The Survival Hunter.
fn hunter(items: &[(usize, Value)]) -> Value {
    wearing("forever-gaps/survival-hunter-felstriker-pet-hit", items)
}

/// The live bear reference's talents in Forever's order, as `forever_client_items_20261113b`
/// reads them.
const FERAL_BEAR_TALENTS: &str = "-5500033212310212551-055";

/// The production Feral Bear, whom the target attacks, in Forever's talent trees.
fn bear(items: &[(usize, Value)]) -> Value {
    let mut request = wearing("mage/prepared-v2/production-feral-bear-druid", items);
    player(&mut request)["talentsString"] = json!(FERAL_BEAR_TALENTS);
    request
}

fn prepared(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// A prepared input through the gate and run: the whole result.
fn result(value: &Value) -> Value {
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    simulate_prepared(&prepared).unwrap().result
}

/// A prepared input through the gate and run: the player's metrics.
fn simulate(value: &Value) -> Value {
    let player = result(value)["raidMetrics"]["parties"][0]["players"][0].clone();
    assert!(player["dps"]["avg"].as_f64().unwrap() > 0.0);
    player
}

fn refusal(request: &Value, rules: Rules) -> String {
    match prepare_json_with(request.to_string().as_bytes(), "rust", rules) {
        Err(PrepareError::Refused(refusal)) => refusal.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap_or(0.0)
}

fn added(with: &Value, without: &Value, name: &str) -> f64 {
    stat(with, name) - stat(without, name)
}

/// The use an item registers.
fn use_spell(prepared: &Value, item: i64) -> Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["item_id"] == item)
        .expect("the item's use")
        .clone()
}

fn aura(prepared: &Value, label: &str) -> Value {
    prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == label)
        .expect("the use's buff")
        .clone()
}

/// The temporary stats an item's use grants, as preparation describes them.
fn temporary_stats(prepared: &Value, item: i64) -> Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "temporary_stats" && effect["item_id"] == item)
        .expect("the use's stats")
        .clone()
}

/// The kinds of cooldown the item's use is, as the character's cooldown list holds it.
fn cooldown_types(prepared: &Value, item: i64) -> Value {
    prepared["player"]["major_cooldowns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cooldown| cooldown["action_id"]["item_id"] == item)
        .expect("the use among the cooldowns")["type"]
        .clone()
}

/// How often the item's use was pressed, over every iteration.
fn uses(player: &Value, item: i64) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["itemId"].as_i64() == Some(item))
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["casts"].as_f64().unwrap_or(0.0))
        .sum()
}

/// The request with the item's use at the top of its rotation, as a rotation that calls for the
/// item has it.
fn pressing(mut request: Value, item: i64) -> Value {
    player(&mut request)["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            json!({ "action": { "castSpell": { "spellId": { "itemId": item } } } }),
        );
    request
}

/// The main hand's weapon skill above base, as the attack table carries it.
fn main_hand_skill(prepared: &Value) -> f64 {
    prepared["melee"]["main_hand_skill"]["skill"]
        .as_f64()
        .unwrap_or(0.0)
}

/// The prepared input without the hash of the request it came from, which names the item.
fn without_request_hash(mut prepared: Value) -> Value {
    prepared.as_object_mut().unwrap().remove("request_sha256");
    prepared
}

// ---------------------------------------------------------------------------------------------
// Lucky Shovelnose Tuft.
// ---------------------------------------------------------------------------------------------

/// The trinket's row is the tooltip's Strength and Spirit; the dodge is the use's, not the
/// item's.
#[test]
fn the_tuft_is_a_strength_and_spirit_trinket() {
    let with = prepared(&paladin(&[(
        TRINKET_1,
        json!({ "id": LUCKY_SHOVELNOSE_TUFT }),
    )]));
    let without = prepared(&paladin(&[(TRINKET_1, json!({}))]));
    assert!(added(&with, &without, "Strength") >= 11.0);
    assert!(added(&with, &without, "Spirit") >= 11.0);
    assert_eq!(added(&with, &without, "DodgeRating"), 0.0);
    simulate(&with);
}

/// The use is on the item effect's cooldowns, 5 minutes and 15 seconds of category 2554, and its
/// buff is 60 Dodge rating (5%) for 15 seconds.
#[test]
fn the_tufts_use_is_five_percent_dodge_for_fifteen_seconds() {
    let with = prepared(&paladin(&[(
        TRINKET_1,
        json!({ "id": LUCKY_SHOVELNOSE_TUFT }),
    )]));
    let spell = use_spell(&with, LUCKY_SHOVELNOSE_TUFT);
    assert_eq!(spell["cd"]["duration_ns"], 300 * SECOND);
    assert_eq!(spell["shared_cd"]["duration_ns"], 15 * SECOND);
    let buff = aura(&with, "Lucky Shovelnose Snout (1315844)");
    assert_eq!(buff["duration_ns"], 15 * SECOND);
    let effect = temporary_stats(&with, LUCKY_SHOVELNOSE_TUFT);
    let dodge =
        effect["active_stats"]["DodgeRating"].as_f64().unwrap() - stat(&with, "DodgeRating");
    assert!((dodge - 60.0).abs() < 1e-9, "{dodge}");
}

/// A dodge buff is a survival cooldown, which a rotation that does not call for it never presses:
/// the paladin wears the trinket for its stats.
#[test]
fn the_tufts_use_is_a_survival_cooldown() {
    let with = prepared(&paladin(&[(
        TRINKET_1,
        json!({ "id": LUCKY_SHOVELNOSE_TUFT }),
    )]));
    assert_eq!(
        cooldown_types(&with, LUCKY_SHOVELNOSE_TUFT),
        json!(["survival"])
    );
    assert_eq!(uses(&simulate(&with), LUCKY_SHOVELNOSE_TUFT), 0.0);
}

/// A rotation that calls for the tuft presses it once in a 120 second fight.
#[test]
fn a_rotation_that_calls_for_the_tuft_presses_it() {
    let request = pressing(
        paladin(&[(TRINKET_1, json!({ "id": LUCKY_SHOVELNOSE_TUFT }))]),
        LUCKY_SHOVELNOSE_TUFT,
    );
    let iterations = request["simOptions"]["iterations"].as_f64().unwrap();
    assert_eq!(
        uses(&simulate(&prepared(&request)), LUCKY_SHOVELNOSE_TUFT),
        iterations
    );
}

// ---------------------------------------------------------------------------------------------
// Flamebreak Talisman.
// ---------------------------------------------------------------------------------------------

/// The trinket's row is the tooltip's 17 Fire Resistance, and its use is on the item effect's
/// cooldowns: 5 minutes, and 30 seconds of the shared on-use category.
#[test]
fn the_talisman_is_a_fire_resistance_trinket_with_a_use() {
    let with = prepared(&rogue(&[(TRINKET_1, json!({ "id": FLAMEBREAK_TALISMAN }))]));
    let without = prepared(&rogue(&[(TRINKET_1, json!({}))]));
    assert_eq!(added(&with, &without, "FireResistance"), 17.0);
    assert_eq!(added(&with, &without, "Agility"), 0.0);
    let spell = use_spell(&with, FLAMEBREAK_TALISMAN);
    assert_eq!(spell["cd"]["duration_ns"], 300 * SECOND);
    assert_eq!(spell["shared_cd"]["duration_ns"], 30 * SECOND);
    let buff = aura(&with, "Flamebreak Talisman (1315878)");
    assert_eq!(buff["duration_ns"], 30 * SECOND);
    assert_eq!(cooldown_types(&with, FLAMEBREAK_TALISMAN), json!(["dps"]));
}

/// The use grants 33 Agility, before the character's own multipliers.
#[test]
fn the_talismans_use_is_thirty_three_agility() {
    let with = prepared(&rogue(&[(TRINKET_1, json!({ "id": FLAMEBREAK_TALISMAN }))]));
    let effect = temporary_stats(&with, FLAMEBREAK_TALISMAN);
    assert_eq!(
        effect["gain_log"],
        "Gained {\"Agility\": 33.000,} from {ItemID: 280071}."
    );
    let agility = effect["active_stats"]["Agility"].as_f64().unwrap() - stat(&with, "Agility");
    assert!((33.0..66.0).contains(&agility), "{agility}");
}

/// "If there is an elemental nearby, gain an additional 33": no client row states when that
/// holds, so the use grants the same 33 against an Elemental target.
#[test]
fn the_talisman_grants_no_second_half_against_an_elemental() {
    let mut request = rogue(&[(TRINKET_1, json!({ "id": FLAMEBREAK_TALISMAN }))]);
    let plain = temporary_stats(&prepared(&request), FLAMEBREAK_TALISMAN);
    for target in request["encounter"]["targets"].as_array_mut().unwrap() {
        target["mobType"] = json!("MobTypeElemental");
    }
    let elemental = prepared(&request);
    assert_eq!(elemental["target"]["mob_type"], "MobTypeElemental");
    assert_eq!(
        temporary_stats(&elemental, FLAMEBREAK_TALISMAN)["gain_log"],
        plain["gain_log"]
    );
}

/// Over a 120 second fight a rotation that calls for the trinket uses it once an iteration, and
/// the rogue deals more with it than with an empty slot.
#[test]
fn the_talisman_is_used_once_in_a_two_minute_fight() {
    let mut with = pressing(
        rogue(&[(TRINKET_1, json!({ "id": FLAMEBREAK_TALISMAN }))]),
        FLAMEBREAK_TALISMAN,
    );
    let mut without = rogue(&[(TRINKET_1, json!({}))]);
    for request in [&mut with, &mut without] {
        request["simOptions"]["iterations"] = json!(200);
    }
    let used = simulate(&prepared(&with));
    let plain = simulate(&prepared(&without));
    assert_eq!(uses(&used, FLAMEBREAK_TALISMAN), 200.0);
    assert!(used["dps"]["avg"].as_f64().unwrap() > plain["dps"]["avg"].as_f64().unwrap());
}

// ---------------------------------------------------------------------------------------------
// Tunnelweb Pincer.
// ---------------------------------------------------------------------------------------------

/// The dagger's row is the tooltip's: 42 to 78 at 1.60, +7 Agility, +8 Stamina and 0.4% crit,
/// which is 5.6 crit rating at 14 a percent.
#[test]
fn the_pincer_is_the_tooltips_dagger() {
    let with = prepared(&hunter(&[(
        HUNTER_MAIN_HAND,
        json!({ "id": TUNNELWEB_PINCER }),
    )]));
    let without = prepared(&hunter(&[(HUNTER_MAIN_HAND, json!({}))]));
    let main_hand = &with["melee"]["main_hand"];
    assert_eq!(main_hand["swing_speed"], 1.6);
    assert_eq!(main_hand["normalized_swing_speed"], 1.7);
    let spread = main_hand["base_damage_max"].as_f64().unwrap()
        - main_hand["base_damage_min"].as_f64().unwrap();
    assert_eq!(spread, 78.0 - 42.0);
    let crit = added(&with, &without, "MeleeCritRating");
    assert!((crit - 5.6).abs() < 1e-9, "{crit}");
    assert!(added(&with, &without, "Agility") >= 7.0);
    assert!(added(&with, &without, "Stamina") >= 8.0);
    simulate(&with);
}

// ---------------------------------------------------------------------------------------------
// Egan's Blaster.
// ---------------------------------------------------------------------------------------------

/// It has no damage, no stats and no effect: on the paladin the fight is the fight of the same
/// character with nothing in the ranged slot, roll for roll.
#[test]
fn wearing_the_blaster_is_wearing_nothing_in_the_ranged_slot() {
    let with = prepared(&paladin(&[(
        PALADIN_RANGED,
        json!({ "id": EGANS_BLASTER }),
    )]));
    let without = prepared(&paladin(&[(PALADIN_RANGED, json!({}))]));
    let with_result = result(&with);
    let without_result = result(&without);
    assert_eq!(without_request_hash(with), without_request_hash(without));
    assert_eq!(with_result, without_result);
    // The slot's item is read: one with an id no row holds is refused.
    let unknown = paladin(&[(PALADIN_RANGED, json!({ "id": 999_999_999 }))]);
    assert!(refusal(&unknown, Rules::Forever).contains("no item with id 999999999"));
}

/// A Hunter's shots need a ranged weapon, which the blaster is not: the Hunter is refused, and
/// the refusal names the item.
#[test]
fn a_hunter_holding_the_blaster_is_refused_by_name() {
    let reason = refusal(
        &hunter(&[(HUNTER_RANGED, json!({ "id": EGANS_BLASTER }))]),
        Rules::Forever,
    );
    assert!(
        reason.contains("item 13289 (Egan's Blaster) in the ranged slot is no ranged weapon"),
        "{reason}"
    );
}

// ---------------------------------------------------------------------------------------------
// Matriarch's Prowlers and Berdun's Trusty Rifle.
// ---------------------------------------------------------------------------------------------

/// The Prowlers raise the skill the bear's attacks take by one, as Spiderchitin Wrap does; a
/// rogue has no Feral Combat line and wears them as plain feet.
#[test]
fn the_prowlers_raise_a_druids_feral_combat_skill() {
    let with = prepared(&bear(&[(FEET, json!({ "id": MATRIARCHS_PROWLERS }))]));
    let without = prepared(&bear(&[(FEET, json!({}))]));
    simulate(&with);
    assert_eq!(main_hand_skill(&without), 0.0);
    assert_eq!(main_hand_skill(&with), 1.0);

    let with = prepared(&rogue(&[(FEET, json!({ "id": MATRIARCHS_PROWLERS }))]));
    let without = prepared(&rogue(&[(FEET, json!({}))]));
    simulate(&with);
    assert_eq!(
        with["melee"]["main_hand_skill"],
        without["melee"]["main_hand_skill"]
    );
    // 94 Armor and +11 Agility, which is 2 Armor a point.
    assert!(added(&with, &without, "Agility") >= 11.0);
    assert!(added(&with, &without, "Armor") >= 94.0 + 22.0);
}

/// The rifle prepares and runs in the hunter's ranged slot with its 6 Strength.
#[test]
fn the_rifle_is_a_gun_a_hunter_shoots_with() {
    let with = prepared(&hunter(&[(
        HUNTER_RANGED,
        json!({ "id": BERDUNS_TRUSTY_RIFLE }),
    )]));
    let fixture = prepared(&hunter(&[]));
    simulate(&with);
    // The fixture's own ranged weapon has no Strength.
    assert!(added(&with, &fixture, "Strength") >= 6.0);
}

// ---------------------------------------------------------------------------------------------
// The reference's rules.
// ---------------------------------------------------------------------------------------------

#[test]
fn the_reference_rules_still_refuse_the_items() {
    for (slot, id) in [
        (TRINKET_1, LUCKY_SHOVELNOSE_TUFT),
        (TRINKET_1, FLAMEBREAK_TALISMAN),
        (HUNTER_MAIN_HAND, TUNNELWEB_PINCER),
        (HUNTER_RANGED, EGANS_BLASTER),
        (FEET, MATRIARCHS_PROWLERS),
        (HUNTER_RANGED, BERDUNS_TRUSTY_RIFLE),
    ] {
        let reason = refusal(&hunter(&[(slot, json!({ "id": id }))]), Rules::GoReference);
        assert!(
            reason.contains(&format!("no item with id {id}")),
            "{reason}"
        );
    }
}
