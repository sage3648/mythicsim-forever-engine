//! Forever client items that refused real players' Quick Sims on 12 November 2026 ("no item with
//! id N"), from Wowhead's Forever tooltips and the client rows wago.tools serves (`product=wowf`,
//! build 1.60.1.70888) on 13 November 2026, none of them encrypted:
//!
//! - Lightcrest Hammer (276419), one-hand mace, item level 60, 66 to 123, speed 2.40: "Chance on
//!   hit: Smites a target for 105 Holy damage. Deals 2 times as much damage to Undead." ItemEffect
//!   234878 is a chance on hit (trigger type 2) of Smite (1299631) with no rate and no cooldown: a
//!   School Damage of 105, Holy, on the magic table (SpellEffect 1323965, SpellMisc 846916,
//!   SpellCategories 185137), and a Dummy of 2 (SpellEffect 1323966), the doubling against
//!   Undead. Two Combat Rogue refusals.
//! - Spiderchitin Wrap (280016), leather chest, item level 63: 176 Armor, +18 Strength, +24
//!   Stamina, "Equip: Increases Feral Combat +1." ItemEffect 238248 is an equip of Increased Feral
//!   Combat (1320726), a skill aura (30) of 1 on skill line 3014, Feral Combat (SpellEffect
//!   1355535): the Druid's own weapon skill line (SkillLine 3014, the Weapon Skills category;
//!   SkillRaceClassInfo 2554, Druids only). Three Feral Bear refusals.
//! - Fingerbreaker Stamp (276743), trinket, item level 53: "Equip: Increased Defense +6." and
//!   "Equip: Increases Strength by 38 while affected by a Bleed." ItemEffect 237304 is an equip of
//!   Fingerbreaker Stamp (1314784), a periodic dummy aura (226) of 38 every 1000 ms (SpellEffect
//!   1346731). Nothing in a fight here bleeds the wearer, so the Strength never applies. One Feral
//!   Bear refusal.
//!
//! No refused request was at hand, so the tests dress fixture characters: the Combat Rogue of
//! `fixtures/forever-gaps/combat-rogue-self-casts-at-player` and the production Feral Bear of
//! `fixtures/mage/prepared-v2/production-feral-bear-druid`, whom the target attacks.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/rogue_113483.rs"]
mod rogue_113483;

const LIGHTCREST_HAMMER: i64 = 276419;
const SMITE: i64 = 1299631;
const SPIDERCHITIN_WRAP: i64 = 280016;
const FINGERBREAKER_STAMP: i64 = 276743;

const CHEST: usize = 4;
const MAIN_HAND: usize = 14;
/// A position the bear's list leaves empty (it holds 14 items, its weapon at 12 and its idol at
/// 13). Preparation places an item by its type, so the stamp goes in a trinket slot from here.
const SPARE: usize = 16;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// A fixture's request (a path under `fixtures/`) with these items in these slots and no
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

/// The Combat Rogue, who has no Feral Combat skill. The fixture's talents are read in the trees
/// Forever's rules prepare.
fn rogue(items: &[(usize, Value)]) -> Value {
    let mut request = wearing("forever-gaps/combat-rogue-self-casts-at-player", items);
    rogue_113483::forever_request(&mut request);
    request
}

/// The live bear reference's talents in Forever's order, as `forever_bear_lacerate_maul_20261110`
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

/// The main hand's weapon skill above base, as the attack table carries it (0 where none moves
/// the table).
fn main_hand_skill(prepared: &Value) -> f64 {
    prepared["melee"]["main_hand_skill"]["skill"]
        .as_f64()
        .unwrap_or(0.0)
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

/// The rogue with the hammer in the main hand, against a target of this creature type.
fn hammer_against(mob_type: &str) -> Value {
    let mut request = rogue(&[(MAIN_HAND, json!({ "id": LIGHTCREST_HAMMER }))]);
    for target in request["encounter"]["targets"].as_array_mut().unwrap() {
        target["mobType"] = json!(mob_type);
    }
    request
}

// ---------------------------------------------------------------------------------------------
// Lightcrest Hammer.
// ---------------------------------------------------------------------------------------------

/// The hammer prepares, passes the gate and runs in the Combat Rogue's main hand, and Smite lands.
#[test]
fn the_hammer_prepares_and_runs() {
    let player = simulate(&prepared(&rogue(&[(
        MAIN_HAND,
        json!({ "id": LIGHTCREST_HAMMER }),
    )])));
    assert!(total(&player, SMITE, "damage") > 0.0);
}

/// The row is the tooltip's mace: 66 to 123 at 2.40, with no stats.
#[test]
fn the_hammer_row_is_the_tooltips_mace() {
    let prepared = prepared(&rogue(&[(MAIN_HAND, json!({ "id": LIGHTCREST_HAMMER }))]));
    let main_hand = &prepared["melee"]["main_hand"];
    assert_eq!(main_hand["base_damage_min"], 66.0);
    assert_eq!(main_hand["base_damage_max"], 123.0);
    assert_eq!(main_hand["swing_speed"], 2.4);
}

/// Smite is the client row's: a Holy hit of 105 on the magic table, with no variance, no damage
/// over time and no coefficient. No rate is stated: 4.32% a swing at 2.40.
#[test]
fn smite_is_a_holy_hit_on_the_weapons_proc_rate() {
    let prepared = prepared(&hammer_against("MobTypeHumanoid"));
    let proc = proc(&prepared, "Lightcrest Hammer");
    assert_eq!(proc["landed_only"], true);
    assert_eq!(proc["average"], 105.0);
    assert_eq!(proc["variance"], 0.0);
    assert!(proc.get("periodic").is_none());
    assert!(proc.get("area").is_none());
    let chances = proc["chances"].as_array().unwrap();
    assert!(!chances.is_empty());
    for chance in chances {
        let chance = chance["chance"].as_f64().unwrap();
        assert!((chance - 0.018 * 2.4).abs() < 1e-9, "{chance}");
    }
    let spell = proc_spell(&prepared, "Lightcrest Hammer");
    assert_eq!(spell["action_id"]["spell_id"], SMITE);
    assert_eq!(spell["school"], 2);
    assert_eq!(spell["defense_type"], "DefenseTypeMagic");
    assert_eq!(spell["bonus_coefficient"], 0.0);
    assert_eq!(spell["damage_multiplier"], 1.0);
}

/// "Deals 2 times as much damage to Undead": against an Undead target the smite's damage
/// multiplier is 2, and nothing else about the proc or its spell differs.
#[test]
fn smite_is_doubled_against_undead() {
    let undead = prepared(&hammer_against("MobTypeUndead"));
    let humanoid = prepared(&hammer_against("MobTypeHumanoid"));
    assert_eq!(
        proc_spell(&undead, "Lightcrest Hammer")["damage_multiplier"],
        2.0
    );
    assert_eq!(
        proc(&undead, "Lightcrest Hammer"),
        proc(&humanoid, "Lightcrest Hammer")
    );
    let mut same = proc_spell(&undead, "Lightcrest Hammer").clone();
    same["damage_multiplier"] = json!(1.0);
    assert_eq!(&same, proc_spell(&humanoid, "Lightcrest Hammer"));
}

/// In the fight a smite that does not crit is the row's 105 against no resistance.
#[test]
fn smite_hits_for_a_hundred_and_five() {
    let mut request = hammer_against("MobTypeHumanoid");
    request["raid"]["debuffs"] = json!({});
    let player = simulate(&prepared(&request));
    let target = &player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"].as_i64() == Some(SMITE))
        .unwrap()["targets"][0];
    assert!(target["hits"].as_f64().unwrap() > 0.0);
    assert_eq!(target["hitRange"]["max"], 105.0);
}

// ---------------------------------------------------------------------------------------------
// Spiderchitin Wrap.
// ---------------------------------------------------------------------------------------------

/// The wrap prepares and runs on the Feral Bear and raises the skill the bear's attacks take by
/// one; the bear in no chest has none above base.
#[test]
fn the_wrap_raises_a_druids_feral_combat_skill() {
    let with = prepared(&bear(&[(CHEST, json!({ "id": SPIDERCHITIN_WRAP }))]));
    let without = prepared(&bear(&[(CHEST, json!({}))]));
    simulate(&with);
    assert_eq!(main_hand_skill(&without), 0.0);
    assert_eq!(main_hand_skill(&with), 1.0);
    // The table the bear's attacks read moves by the one point.
    assert_ne!(
        with["melee"]["main_hand_skill"]["miss_chance"],
        with["melee"]["base_miss_chance"]
    );
}

/// The row is the tooltip's leather chest: 176 Armor, +18 Strength and +24 Stamina, before the
/// bear's own multipliers.
#[test]
fn the_wrap_row_is_the_tooltips_chest() {
    let with = prepared(&rogue(&[(CHEST, json!({ "id": SPIDERCHITIN_WRAP }))]));
    let without = prepared(&rogue(&[(CHEST, json!({}))]));
    assert_eq!(stat(&with, "Armor") - stat(&without, "Armor"), 176.0);
    assert!(stat(&with, "Strength") - stat(&without, "Strength") >= 18.0);
    assert!(stat(&with, "Stamina") - stat(&without, "Stamina") >= 24.0);
}

/// On anyone but a Druid the Feral Combat skill is a line they do not have: the wrap is a plain
/// chest, and the rogue's main hand keeps its skill.
#[test]
fn the_wrap_is_a_plain_chest_on_a_rogue() {
    let with = prepared(&rogue(&[(CHEST, json!({ "id": SPIDERCHITIN_WRAP }))]));
    let without = prepared(&rogue(&[(CHEST, json!({}))]));
    simulate(&with);
    assert_eq!(main_hand_skill(&with), main_hand_skill(&without));
    assert_eq!(
        with["melee"]["main_hand_skill"],
        without["melee"]["main_hand_skill"]
    );
}

// ---------------------------------------------------------------------------------------------
// Fingerbreaker Stamp.
// ---------------------------------------------------------------------------------------------

/// The stamp prepares and runs on the Feral Bear, whom the target attacks: it adds its 6 Defense
/// and no Strength, since nothing bleeds the bear.
#[test]
fn the_stamp_is_six_defense_on_the_bear() {
    let with = prepared(&bear(&[(SPARE, json!({ "id": FINGERBREAKER_STAMP }))]));
    let without = prepared(&bear(&[(SPARE, json!({}))]));
    simulate(&with);
    assert_eq!(
        stat(&with, "DefenseRating") - stat(&without, "DefenseRating"),
        6.0
    );
    assert_eq!(stat(&with, "Strength"), stat(&without, "Strength"));
}

// ---------------------------------------------------------------------------------------------
// The reference's rules.
// ---------------------------------------------------------------------------------------------

/// The reference's rules have no row for any of the three and refuse each as unknown.
#[test]
fn the_reference_rules_still_refuse_the_items() {
    for (slot, id) in [
        (MAIN_HAND, LIGHTCREST_HAMMER),
        (CHEST, SPIDERCHITIN_WRAP),
        (SPARE, FINGERBREAKER_STAMP),
    ] {
        let reason = refusal(&bear(&[(slot, json!({ "id": id }))]), Rules::GoReference);
        assert!(
            reason.contains(&format!("no item with id {id}")),
            "{reason}"
        );
    }
}
