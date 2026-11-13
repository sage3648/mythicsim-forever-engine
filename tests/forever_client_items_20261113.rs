//! Forever client items that refused real players' Quick Sims on 12 November 2026 ("no item with
//! id N"), from Wowhead's Forever tooltips and the client rows wago.tools serves (`product=wowf`,
//! build 1.60.1.70888) on 13 November 2026, none of them encrypted:
//!
//! - Pyromancer's Baton (286650), one-hand mace, item level 40, 46 to 86, speed 2.40: "Chance on
//!   hit: Burns the target for 30 Fire damage every 3 sec for 9 sec." ItemEffect 238191 is a chance
//!   on hit (trigger type 2) of Ablaze (1320925) with no rate and no cooldown: a Periodic Damage
//!   aura of 30 every 3000 ms for 9 sec, Fire, on the magic table (SpellEffect 1355874, SpellMisc
//!   866630, SpellCategories 189172). Five Combat Rogue refusals.
//! - Sword of Dismemberment (287005), two-hand sword, item level 42, 110 to 166, speed 3.70:
//!   "Chance on hit: Dismember the target, inflicting 335 to 1005 Physical damage." ItemEffect
//!   238815 is a chance on hit of Dismember (1324687) with no rate and no cooldown: a School Damage
//!   of 670 with a variance of 1, Physical, on the melee table (SpellEffect 1361623, SpellMisc
//!   870298, SpellCategories 189951). Two Retribution Paladin refusals.
//! - Farraki Kris (274647), main hand dagger, item level 47, 23 to 44, speed 1.60, 54 spell damage
//!   and healing, 2 spell penetration, and "Equip: Chance on harmful spell cast to afflict the
//!   target with a draining ritual, leeching 19 health every 2 sec for 6 sec. (100ms cooldown)".
//!   ItemEffect 233957 is an equip of Blood Sacrifice (1294644): SpellAuraOptions 246039 has
//!   ProcChance 100 beside the tooltip's "Chance", read as the rate it states
//!   (`tests/forever_client_items_20261113c.rs`). Three Arcane Mage refusals.
//!
//! The two weapons roll the rate MythicSim assumes for a chance on hit without one
//! (`shared_items::CHANCE_ON_HIT_PPM`, 1.8% per second of the weapon's speed), as Toofbreaker and
//! the Rock on Stick weapons do. The kris casts Blood Sacrifice on every harmful spell: a character
//! with no mana bar casts none and wears it as a plain dagger.
//!
//! No refused request was at hand, so the tests dress fixture characters: the Combat Rogue of
//! `fixtures/forever-gaps/combat-rogue-self-casts-at-player`, the Retribution Paladin of
//! `fixtures/paladin/retribution-paladin-unsealed-wrath-command-echoes` and the Elemental Shaman of
//! `fixtures/forever-gaps/elemental-shaman-the-elements`.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/rogue_113483.rs"]
mod rogue_113483;

const PYROMANCERS_BATON: i64 = 286650;
const ABLAZE: i64 = 1320925;
const SWORD_OF_DISMEMBERMENT: i64 = 287005;
const DISMEMBER: i64 = 1324687;
const FARRAKI_KRIS: i64 = 274647;

const MAIN_HAND: usize = 14;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// A fixture's request (a path under `fixtures/`) with this item in the main hand and no
/// consumables, whose sharpening stone would add weapon damage.
fn wielding(fixture: &str, main_hand: Value) -> Value {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/{fixture}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    let player = player(&mut request);
    player["consumables"] = json!({});
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.resize(17, json!({}));
    equipment[MAIN_HAND] = main_hand;
    request
}

/// The Combat Rogue, who has no mana bar. The fixture's talents are read in the trees Forever's
/// rules prepare.
fn rogue(main_hand: Value) -> Value {
    let mut request = wielding("forever-gaps/combat-rogue-self-casts-at-player", main_hand);
    rogue_113483::forever_request(&mut request);
    request
}

/// The Retribution Paladin, who wields a two-hand weapon, without the fixture's item swap, which
/// would take the weapon out of the hand.
fn paladin(main_hand: Value) -> Value {
    let mut request = wielding(
        "paladin/retribution-paladin-unsealed-wrath-command-echoes",
        main_hand,
    );
    let player = player(&mut request).as_object_mut().unwrap();
    player.remove("itemSwap");
    player.insert("enableItemSwap".to_string(), json!(false));
    request
}

/// The Elemental Shaman, a caster with a mana bar.
fn shaman(main_hand: Value) -> Value {
    wielding("forever-gaps/elemental-shaman-the-elements", main_hand)
}

fn prepared(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
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

fn refusal(request: &Value, rules: Rules) -> String {
    match prepare_json_with(request.to_string().as_bytes(), "rust", rules) {
        Err(PrepareError::Refused(refusal)) => refusal.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A field of the player's actions with this spell id, summed over every tag and target.
fn total(player: &Value, id: i64, field: &str) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"].as_i64() == Some(id))
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target[field].as_f64().unwrap_or(0.0))
        .sum()
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap_or(0.0)
}

fn proc<'a>(prepared: &'a Value, item: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "spell_data_damage_proc" && effect["trigger_aura"] == item)
        .unwrap_or_else(|| panic!("{item}'s chance on hit"))
}

/// The spell a chance on hit casts, by its spellbook position.
fn proc_spell<'a>(prepared: &'a Value, item: &str) -> &'a Value {
    let position = proc(prepared, item)["spell"].as_u64().unwrap() as usize;
    &prepared["player"]["spells"][position]
}

/// Every chance the proc rolls, which are the weapon's: 1.8% per second of its speed.
fn assert_assumed_rate(proc: &Value, speed: f64) {
    let chances = proc["chances"].as_array().unwrap();
    assert!(!chances.is_empty());
    for chance in chances {
        let chance = chance["chance"].as_f64().unwrap();
        assert!((chance - 0.018 * speed).abs() < 1e-9, "{chance}");
    }
}

// ---------------------------------------------------------------------------------------------
// Pyromancer's Baton.
// ---------------------------------------------------------------------------------------------

/// The baton prepares, passes the gate and runs in the Combat Rogue's main hand, and Ablaze ticks
/// on the target.
#[test]
fn the_baton_prepares_and_runs() {
    let player = simulate(&prepared(&rogue(json!({ "id": PYROMANCERS_BATON }))));
    assert!(total(&player, ABLAZE, "damage") > 0.0);
}

/// The row is the tooltip's mace: 46 to 86 at 2.40, with no stats.
#[test]
fn the_baton_row_is_the_tooltips_mace() {
    let prepared = prepared(&rogue(json!({ "id": PYROMANCERS_BATON })));
    let main_hand = &prepared["melee"]["main_hand"];
    assert_eq!(main_hand["base_damage_min"], 46.0);
    assert_eq!(main_hand["base_damage_max"], 86.0);
    assert_eq!(main_hand["swing_speed"], 2.4);
}

/// Ablaze is the client row's: three ticks of 30 Fire every 3 sec on the unit hit, with no hit of
/// its own and no coefficient. It is a magic spell, so it rolls the magic hit as it goes on and its
/// ticks may crit (the row's `PERIODIC_CAN_CRIT`), as Flaming Incinerator's Incinerator does. No
/// rate is stated: 4.32% a swing at 2.40.
#[test]
fn ablaze_is_a_fire_damage_over_time_on_the_weapons_proc_rate() {
    let prepared = prepared(&rogue(json!({ "id": PYROMANCERS_BATON })));
    let proc = proc(&prepared, "Pyromancer's Baton");
    assert_eq!(proc["landed_only"], true);
    assert_eq!(proc["average"], 0.0);
    let periodic = &proc["periodic"];
    assert_eq!(periodic["tick_base"], 30.0);
    assert_eq!(periodic["tick_outcome"], "tick_magic_hit_and_crit");
    assert_eq!(periodic["with_direct"], false);
    assert_eq!(periodic["application"], "magic_hit");
    assert_assumed_rate(proc, 2.4);
    let spell = proc_spell(&prepared, "Pyromancer's Baton");
    assert_eq!(spell["action_id"]["spell_id"], ABLAZE);
    assert_eq!(spell["school"], 4);
    assert_eq!(spell["defense_type"], "DefenseTypeMagic");
    assert_eq!(spell["dot"]["base_tick_count"], 3);
    assert_eq!(spell["dot"]["base_tick_length_ns"], 3_000_000_000_i64);
    assert_eq!(spell["dot"]["bonus_coefficient"], 0.0);
}

/// In the fight every tick that does not crit is the row's 30 and every crit 45, and the
/// application can miss, as a magic spell's does.
#[test]
fn ablaze_ticks_for_thirty() {
    let mut request = rogue(json!({ "id": PYROMANCERS_BATON }));
    request["raid"]["debuffs"] = json!({});
    let player = simulate(&prepared(&request));
    let target = &player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"].as_i64() == Some(ABLAZE))
        .unwrap()["targets"][0];
    assert!(target["ticks"].as_f64().unwrap() > 0.0);
    assert_eq!(target["tickRange"]["min"], 30.0);
    assert_eq!(target["tickRange"]["max"], 30.0);
    if target["critTicks"].as_f64().unwrap_or(0.0) > 0.0 {
        assert_eq!(target["critTickRange"]["min"], 45.0);
        assert_eq!(target["critTickRange"]["max"], 45.0);
    }
}

// ---------------------------------------------------------------------------------------------
// Sword of Dismemberment.
// ---------------------------------------------------------------------------------------------

/// The sword prepares, passes the gate and runs in the Retribution Paladin's hands, and Dismember
/// lands.
#[test]
fn the_sword_prepares_and_runs() {
    let player = simulate(&prepared(&paladin(json!({ "id": SWORD_OF_DISMEMBERMENT }))));
    assert!(total(&player, DISMEMBER, "damage") > 0.0);
    assert!(total(&player, DISMEMBER, "hits") > 0.0);
}

/// The row is the tooltip's sword: 110 to 166 at 3.70, with no stats.
#[test]
fn the_sword_row_is_the_tooltips_two_hand_sword() {
    let prepared = prepared(&paladin(json!({ "id": SWORD_OF_DISMEMBERMENT })));
    let main_hand = &prepared["melee"]["main_hand"];
    assert_eq!(main_hand["base_damage_min"], 110.0);
    assert_eq!(main_hand["base_damage_max"], 166.0);
    assert_eq!(main_hand["swing_speed"], 3.7);
}

/// Dismember is the client row's: a Physical hit of 670 with a variance of 1 (335 to 1005) on the
/// melee table, with no damage over time and no coefficient. No rate is stated: 6.66% a swing at
/// 3.70.
#[test]
fn dismember_is_a_physical_hit_on_the_weapons_proc_rate() {
    let prepared = prepared(&paladin(json!({ "id": SWORD_OF_DISMEMBERMENT })));
    let proc = proc(&prepared, "Sword of Dismemberment");
    assert_eq!(proc["landed_only"], true);
    assert_eq!(proc["average"], 670.0);
    assert_eq!(proc["variance"], 1.0);
    assert_eq!(proc["outcome"], "melee_special_hit_and_crit");
    assert!(proc.get("periodic").is_none());
    assert!(proc.get("area").is_none());
    assert_assumed_rate(proc, 3.7);
    let spell = proc_spell(&prepared, "Sword of Dismemberment");
    assert_eq!(spell["action_id"]["spell_id"], DISMEMBER);
    assert_eq!(spell["school"], 1);
    assert_eq!(spell["defense_type"], "DefenseTypeMelee");
    assert_eq!(spell["bonus_coefficient"], 0.0);
}

/// Against no armor the hits fall in the row's 335 to 1005, crits aside, raised at most by the
/// paladin's Vengeance (15% physical damage after a crit).
#[test]
fn dismember_hits_for_three_hundred_thirty_five_to_a_thousand_and_five() {
    let mut request = paladin(json!({ "id": SWORD_OF_DISMEMBERMENT }));
    request["raid"]["debuffs"] = json!({});
    request["encounter"]["targets"][0]["stats"][30] = json!(0.0);
    let mut value = prepared(&request);
    let position = proc(&value, "Sword of Dismemberment")["spell"]
        .as_u64()
        .unwrap() as usize;
    // The paladin's talents raise every damage spell's multiplier; the row's range is read at 1.
    value["player"]["spells"][position]["damage_multiplier"] = json!(1.0);
    let player = simulate(&value);
    let hit = &player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"].as_i64() == Some(DISMEMBER))
        .unwrap()["targets"][0]["hitRange"];
    let (min, max) = (hit["min"].as_f64().unwrap(), hit["max"].as_f64().unwrap());
    assert!(min >= 335.0 && max <= 1005.0 * 1.15 + 1e-9, "{min} {max}");
    // The row's range is three to one, whatever multiplies it.
    assert!(max / min <= 3.0 + 1e-9, "{min} {max}");
}

// ---------------------------------------------------------------------------------------------
// Farraki Kris.
// ---------------------------------------------------------------------------------------------

/// On the Combat Rogue, who has no mana bar and casts no harmful spell, the kris is a plain dagger
/// of the tooltip's weapon and stats: 54 spell damage, 54 healing and 2 spell penetration.
#[test]
fn the_kris_is_a_plain_dagger_on_a_character_with_no_mana_bar() {
    let with = prepared(&rogue(json!({ "id": FARRAKI_KRIS })));
    let without = prepared(&rogue(json!({})));
    simulate(&with);
    let main_hand = &with["melee"]["main_hand"];
    assert_eq!(main_hand["base_damage_min"], 23.0);
    assert_eq!(main_hand["base_damage_max"], 44.0);
    assert_eq!(main_hand["swing_speed"], 1.6);
    assert_eq!(
        stat(&with, "SpellDamage") - stat(&without, "SpellDamage"),
        54.0
    );
    assert_eq!(
        stat(&with, "HealingPower") - stat(&without, "HealingPower"),
        54.0
    );
    assert_eq!(
        stat(&with, "SpellPiercing") - stat(&without, "SpellPiercing"),
        2.0
    );
}

/// On a caster with a mana bar the kris prepares: its effect is read from the client rows
/// (`tests/forever_client_items_20261113c.rs`).
#[test]
fn the_kris_prepares_on_a_character_with_a_mana_bar() {
    prepared(&shaman(json!({ "id": FARRAKI_KRIS })));
}

// ---------------------------------------------------------------------------------------------
// The reference's rules.
// ---------------------------------------------------------------------------------------------

/// The reference's rules have no row for any of the three and refuse each as unknown.
#[test]
fn the_reference_rules_still_refuse_the_items() {
    assert!(prepare_json_with(
        shaman(json!({})).to_string().as_bytes(),
        "rust",
        Rules::GoReference
    )
    .is_ok());
    for id in [PYROMANCERS_BATON, SWORD_OF_DISMEMBERMENT, FARRAKI_KRIS] {
        let reason = refusal(&shaman(json!({ "id": id })), Rules::GoReference);
        assert!(
            reason.contains(&format!("no item with id {id}")),
            "{reason}"
        );
    }
}
