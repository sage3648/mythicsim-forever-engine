//! Forever client items and set bonuses that refused real players' Quick Sims on 11 and 12
//! November 2026 ("no item with id", "set bonus Krol'dok Battlegear 2P is not prepared yet"), from
//! Wowhead's Forever tooltips and the client rows wago.tools serves (`product=wowf`, build
//! 1.60.1.70888) on 12 November 2026:
//!
//! - Defias Enforcer's Garb, the five leather pieces of a Forever set the reference does not
//!   register: Mask 287019, Shoulderpads 276741, Chest 287020, Legguards 276746, Prowlers 287021.
//!   Its bonuses of plain stats are applied ((2) +10 Fire Resistance, (3) +15 Stamina, (4)
//!   Increased Swords +1); the (5) crit proc states no chance and refuses by name.
//! - Krol'dok Battlegear's (2) +100 Armor and (3) Increased Defense +5 are applied; its (4) heal
//!   on block and (5) Stamina when taking damage still refuse by name.
//! - Heart of the Primeval (274615), a trinket: +18 Frost Resistance and a use of 19 spell damage
//!   and healing for 30 sec on a 6 minute cooldown.
//! - Zealot Plate (274651) and Girdle of Extracted Answers (276742), plate whose equip effect acts
//!   only when the wearer is struck (mana, damage on block). A character the effect cannot reach
//!   wears each as a plain item; one it can reach is refused.
//!
//! No refused request was at hand, so the tests dress the characters of `fixtures/forever-gaps`.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/rogue_113483.rs"]
mod rogue_113483;

const HEART_OF_THE_PRIMEVAL: i64 = 274615;
const ZEALOT_PLATE: i64 = 274651;
const GIRDLE_OF_EXTRACTED_ANSWERS: i64 = 276742;

const DEFIAS_MASK: i64 = 287019;
const DEFIAS_SHOULDERPADS: i64 = 276741;
const DEFIAS_CHEST: i64 = 287020;
const DEFIAS_LEGGUARDS: i64 = 276746;
const DEFIAS_PROWLERS: i64 = 287021;

const KROLDOK_CHEST: i64 = 275005;
const KROLDOK_HELMET: i64 = 275006;
const KROLDOK_GAUNTLETS: i64 = 287012;
const KROLDOK_LEGPLATES: i64 = 287013;
const KROLDOK_PAULDRONS: i64 = 287014;

const HEAD: usize = 0;
const SHOULDER: usize = 2;
const CHEST: usize = 4;
const WAIST: usize = 7;
const HANDS: usize = 6;
const LEGS: usize = 8;
const FEET: usize = 9;
const TRINKET_1: usize = 12;
const MAIN_HAND: usize = 14;
const OFF_HAND: usize = 15;

/// The armor slots the tests fill, emptied first so a difference is the items' alone.
const ARMOR_SLOTS: [usize; 7] = [HEAD, SHOULDER, CHEST, WAIST, HANDS, LEGS, FEET];

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// A fixture's request without buffs, with the armor slots emptied and then these items worn.
fn dressed(fixture: &str, items: &[(usize, i64)]) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/forever-gaps/{fixture}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    request["raid"]["buffs"] = json!({});
    request["raid"]["parties"][0]["buffs"] = json!({});
    let player = player(&mut request);
    player["buffs"] = json!({});
    let equipment = player["equipment"]["items"].as_array_mut().unwrap();
    equipment.resize(17, json!({}));
    for slot in ARMOR_SLOTS {
        equipment[slot] = json!({});
    }
    for &(slot, id) in items {
        equipment[slot] = json!({ "id": id });
    }
    request
}

/// The DPS Warrior of the Pummel request, whom no target attacks.
fn warrior(items: &[(usize, i64)]) -> Value {
    dressed("dps-warrior-pummel", items)
}

/// The Combat Rogue, whom no target attacks. The fixture's talents are read in the trees
/// Forever's rules prepare.
fn rogue(items: &[(usize, i64)]) -> Value {
    let mut request = dressed("combat-rogue-self-casts-at-player", items);
    rogue_113483::forever_request(&mut request);
    request
}

/// The Protection Paladin, who tanks the target with a shield and has a mana bar.
fn paladin(items: &[(usize, i64)]) -> Value {
    dressed("protection-paladin-quel-serrar", items)
}

/// The Elemental Shaman, a caster whom no target attacks.
fn shaman(items: &[(usize, i64)]) -> Value {
    dressed("elemental-shaman-the-elements", items)
}

fn prepare(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

/// Prepared, through the gate and run: the prepared input and the player's metrics.
fn run(request: &Value) -> (Value, Value) {
    let value = prepare(request);
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    let player = result["raidMetrics"]["parties"][0]["players"][0].clone();
    assert!(player["dps"]["avg"].as_f64().unwrap() > 0.0);
    (value, player)
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap_or(0.0)
}

/// The difference between two prepared characters' stat.
fn added(with: &Value, without: &Value, name: &str) -> f64 {
    stat(&prepare(with), name) - stat(&prepare(without), name)
}

fn refusal(request: &Value, rules: Rules) -> String {
    match prepare_json_with(request.to_string().as_bytes(), "rust", rules) {
        Err(PrepareError::Refused(refusal)) => refusal.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn has_aura(prepared: &Value, label: &str) -> bool {
    prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .any(|aura| aura["label"] == label)
}

// ---------------------------------------------------------------------------------------------
// Krol'dok Battlegear.
// ---------------------------------------------------------------------------------------------

/// The two pieces that refused players (Gauntlets and Pauldrons) now prepare and run, and the
/// second piece adds the bonus's 100 Armor over the pieces' own.
#[test]
fn two_kroldok_pieces_add_a_hundred_armor() {
    let one = warrior(&[(HANDS, KROLDOK_GAUNTLETS)]);
    let two = warrior(&[(HANDS, KROLDOK_GAUNTLETS), (SHOULDER, KROLDOK_PAULDRONS)]);
    let pauldrons = warrior(&[(SHOULDER, KROLDOK_PAULDRONS)]);
    let (prepared, _) = run(&two);
    assert!(has_aura(&prepared, "Krol'dok Battlegear 2P"));
    assert!(!has_aura(&prepared, "Krol'dok Battlegear 3P"));
    assert!(!has_aura(&prepare(&one), "Krol'dok Battlegear 2P"));
    let alone = added(&pauldrons, &warrior(&[]), "Armor");
    assert_eq!(added(&two, &one, "Armor"), alone + 100.0);
    assert_eq!(
        added(&two, &one, "DefenseRating"),
        added(&pauldrons, &warrior(&[]), "DefenseRating")
    );
}

/// The third piece adds Increased Defense +5 over its own stats, and the 100 Armor stays.
#[test]
fn three_kroldok_pieces_add_five_defense() {
    let two = warrior(&[(HANDS, KROLDOK_GAUNTLETS), (SHOULDER, KROLDOK_PAULDRONS)]);
    let three = warrior(&[
        (HANDS, KROLDOK_GAUNTLETS),
        (SHOULDER, KROLDOK_PAULDRONS),
        (HEAD, KROLDOK_HELMET),
    ]);
    let helmet = warrior(&[(HEAD, KROLDOK_HELMET)]);
    let (prepared, _) = run(&three);
    assert!(has_aura(&prepared, "Krol'dok Battlegear 2P"));
    assert!(has_aura(&prepared, "Krol'dok Battlegear 3P"));
    let alone = added(&helmet, &warrior(&[]), "DefenseRating");
    assert_eq!(added(&three, &two, "DefenseRating"), alone + 5.0);
    assert_eq!(
        added(&three, &two, "Armor"),
        added(&helmet, &warrior(&[]), "Armor")
    );
}

/// The heal on block and the Stamina when taking damage are not prepared: the fourth piece
/// refuses and names the bonus.
#[test]
fn the_fourth_kroldok_piece_is_refused_by_name() {
    let four = warrior(&[
        (HEAD, KROLDOK_HELMET),
        (SHOULDER, KROLDOK_PAULDRONS),
        (CHEST, KROLDOK_CHEST),
        (HANDS, KROLDOK_GAUNTLETS),
    ]);
    let reason = refusal(&four, Rules::Forever);
    assert!(
        reason.contains("set bonus Krol'dok Battlegear 4P is not prepared yet (item 287012)"),
        "{reason}"
    );
    let five = warrior(&[
        (HEAD, KROLDOK_HELMET),
        (SHOULDER, KROLDOK_PAULDRONS),
        (CHEST, KROLDOK_CHEST),
        (HANDS, KROLDOK_GAUNTLETS),
        (LEGS, KROLDOK_LEGPLATES),
    ]);
    let reason = refusal(&five, Rules::Forever);
    assert!(reason.contains("Krol'dok Battlegear 4P"), "{reason}");
}

// ---------------------------------------------------------------------------------------------
// Defias Enforcer's Garb.
// ---------------------------------------------------------------------------------------------

/// Each piece alone is a plain item of its tooltip's stats, with no set bonus.
#[test]
fn one_defias_enforcer_piece_is_a_plain_item() {
    let none = rogue(&[]);
    for (slot, id) in [
        (HEAD, DEFIAS_MASK),
        (SHOULDER, DEFIAS_SHOULDERPADS),
        (CHEST, DEFIAS_CHEST),
        (LEGS, DEFIAS_LEGGUARDS),
        (FEET, DEFIAS_PROWLERS),
    ] {
        let (prepared, _) = run(&rogue(&[(slot, id)]));
        assert!(!has_aura(&prepared, "Defias Enforcer's Garb 2P"), "{id}");
    }
    // Mask: 124 Armor, +20 Stamina, 1% less dodged or parried, +30 Attack Power.
    let mask = rogue(&[(HEAD, DEFIAS_MASK)]);
    assert_eq!(added(&mask, &none, "Armor"), 124.0);
    assert_eq!(added(&mask, &none, "ExpertiseRating"), 10.0);
    assert_eq!(added(&mask, &none, "AttackPower"), 30.0);
    assert!(added(&mask, &none, "Stamina") >= 20.0);
    // Shoulderpads: 115 Armor, +13 Stamina, 0.5% crit, +28 Attack Power.
    let shoulderpads = rogue(&[(SHOULDER, DEFIAS_SHOULDERPADS)]);
    assert_eq!(added(&shoulderpads, &none, "Armor"), 115.0);
    assert_eq!(added(&shoulderpads, &none, "AttackPower"), 28.0);
    // Legguards: 138 Armor and +20 Agility (2 Armor a point), +16 Fire Resistance, 1% hit.
    let legguards = rogue(&[(LEGS, DEFIAS_LEGGUARDS)]);
    assert_eq!(added(&legguards, &none, "FireResistance"), 16.0);
    assert!(added(&legguards, &none, "Agility") >= 20.0);
    assert!(added(&legguards, &none, "Armor") >= 138.0 + 40.0);
    // Prowlers: 105 Armor, +16 Stamina, +28 Attack Power.
    let prowlers = rogue(&[(FEET, DEFIAS_PROWLERS)]);
    assert_eq!(added(&prowlers, &none, "Armor"), 105.0);
    assert_eq!(added(&prowlers, &none, "AttackPower"), 28.0);
    // Chest: 156 Armor, +14 Strength, +22 Agility, +11 Stamina.
    let chest = rogue(&[(CHEST, DEFIAS_CHEST)]);
    assert!(added(&chest, &none, "Strength") >= 14.0);
    assert!(added(&chest, &none, "Agility") >= 22.0);
}

/// The Mask and the Chest, the pair that refused Combat Rogues: the second piece adds 10 Fire
/// Resistance.
#[test]
fn two_defias_enforcer_pieces_add_ten_fire_resistance() {
    let one = rogue(&[(HEAD, DEFIAS_MASK)]);
    let two = rogue(&[(HEAD, DEFIAS_MASK), (CHEST, DEFIAS_CHEST)]);
    let (prepared, _) = run(&two);
    assert!(has_aura(&prepared, "Defias Enforcer's Garb 2P"));
    assert!(!has_aura(&prepared, "Defias Enforcer's Garb 3P"));
    assert_eq!(added(&two, &one, "FireResistance"), 10.0);
}

/// The third piece adds 15 Stamina over its own 13.
#[test]
fn three_defias_enforcer_pieces_add_fifteen_stamina() {
    let none = rogue(&[]);
    let two = rogue(&[(HEAD, DEFIAS_MASK), (CHEST, DEFIAS_CHEST)]);
    let three = rogue(&[
        (HEAD, DEFIAS_MASK),
        (CHEST, DEFIAS_CHEST),
        (SHOULDER, DEFIAS_SHOULDERPADS),
    ]);
    let shoulderpads = rogue(&[(SHOULDER, DEFIAS_SHOULDERPADS)]);
    let (prepared, _) = run(&three);
    assert!(has_aura(&prepared, "Defias Enforcer's Garb 3P"));
    let alone = added(&shoulderpads, &none, "Stamina");
    let third = added(&three, &two, "Stamina");
    // Stamina takes the character's own multipliers, so the bonus is 15 at the piece's ratio.
    let ratio = alone / 13.0;
    assert!(
        (third - alone - 15.0 * ratio).abs() < 1e-6,
        "{third} {alone}"
    );
    assert_eq!(added(&three, &two, "FireResistance"), 0.0);
}

/// The fourth piece adds one to the skill of a sword in the main hand, and nothing to a weapon of
/// another kind.
#[test]
fn four_defias_enforcer_pieces_add_one_sword_skill() {
    let skill = |request: &Value| {
        let prepared = prepare(request);
        prepared["melee"]["main_hand_skill"]["skill"]
            .as_f64()
            .unwrap_or(0.0)
    };
    let three = [
        (HEAD, DEFIAS_MASK),
        (CHEST, DEFIAS_CHEST),
        (SHOULDER, DEFIAS_SHOULDERPADS),
    ];
    let four = [
        (HEAD, DEFIAS_MASK),
        (CHEST, DEFIAS_CHEST),
        (SHOULDER, DEFIAS_SHOULDERPADS),
        (LEGS, DEFIAS_LEGGUARDS),
    ];
    // Krol Blade (2244), a one-handed sword of the reference database, in the main hand.
    let with_sword = |items: &[(usize, i64)]| {
        let mut all = items.to_vec();
        all.push((MAIN_HAND, 2244));
        rogue(&all)
    };
    let (prepared, _) = run(&with_sword(&four));
    assert!(has_aura(&prepared, "Defias Enforcer's Garb 4P"));
    assert_eq!(skill(&with_sword(&four)) - skill(&with_sword(&three)), 1.0);
    assert_eq!(skill(&rogue(&four)), skill(&rogue(&three)));
}

/// The crit proc of five pieces states no chance and is not prepared: the fifth piece refuses and
/// names the bonus.
#[test]
fn the_fifth_defias_enforcer_piece_is_refused_by_name() {
    let five = rogue(&[
        (HEAD, DEFIAS_MASK),
        (SHOULDER, DEFIAS_SHOULDERPADS),
        (CHEST, DEFIAS_CHEST),
        (LEGS, DEFIAS_LEGGUARDS),
        (FEET, DEFIAS_PROWLERS),
    ]);
    let reason = refusal(&five, Rules::Forever);
    assert!(
        reason.contains("set bonus Defias Enforcer's Garb 5P is not prepared yet (item 287021)"),
        "{reason}"
    );
}

// ---------------------------------------------------------------------------------------------
// Heart of the Primeval.
// ---------------------------------------------------------------------------------------------

fn heart_spell(prepared: &Value) -> Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["item_id"] == HEART_OF_THE_PRIMEVAL)
        .expect("the trinket's use")
        .clone()
}

/// The trinket's row is the tooltip's 18 Frost Resistance, and its use is on the client's
/// cooldowns: 6 minutes, and 30 seconds of the shared on-use category.
#[test]
fn the_heart_of_the_primeval_is_a_frost_resistance_trinket_with_a_use() {
    let with = shaman(&[(TRINKET_1, HEART_OF_THE_PRIMEVAL)]);
    let mut without = shaman(&[]);
    player(&mut without)["equipment"]["items"][TRINKET_1] = json!({});
    assert_eq!(added(&with, &without, "FrostResistance"), 18.0);
    // The buff is the use's, not the item's.
    assert_eq!(added(&with, &without, "SpellDamage"), 0.0);
    let prepared = prepare(&with);
    let spell = heart_spell(&prepared);
    assert_eq!(spell["cd"]["duration_ns"], 360_000_000_000_i64);
    assert_eq!(spell["shared_cd"]["duration_ns"], 30_000_000_000_i64);
    let aura = prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == "Heart of the Primeval (1295233)")
        .expect("the use's buff");
    assert_eq!(aura["duration_ns"], 30_000_000_000_i64);
    // The buff is 19 spell damage and 19 healing over the character's own.
    let effect = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["kind"] == "temporary_stats" && effect["item_id"] == HEART_OF_THE_PRIMEVAL
        })
        .expect("the use's stats");
    assert_eq!(
        effect["active_stats"]["SpellDamage"].as_f64().unwrap() - stat(&prepared, "SpellDamage"),
        19.0
    );
    assert_eq!(
        effect["active_stats"]["HealingPower"].as_f64().unwrap() - stat(&prepared, "HealingPower"),
        19.0
    );
}

/// Over a 120 second fight the trinket is used once an iteration, and the shaman deals more with
/// it than with an empty slot.
#[test]
fn the_heart_of_the_primeval_is_used_once_in_a_two_minute_fight() {
    let with = shaman(&[(TRINKET_1, HEART_OF_THE_PRIMEVAL)]);
    let mut without = shaman(&[]);
    player(&mut without)["equipment"]["items"][TRINKET_1] = json!({});
    let iterations = with["simOptions"]["iterations"].as_f64().unwrap();
    let (_, used) = run(&with);
    let (_, plain) = run(&without);
    let casts: f64 = used["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["itemId"].as_i64() == Some(HEART_OF_THE_PRIMEVAL))
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["casts"].as_f64().unwrap_or(0.0))
        .sum();
    assert_eq!(casts, iterations);
    assert!(used["dps"]["avg"].as_f64().unwrap() > plain["dps"]["avg"].as_f64().unwrap());
}

// ---------------------------------------------------------------------------------------------
// Zealot Plate and Girdle of Extracted Answers.
// ---------------------------------------------------------------------------------------------

/// On the DPS Warrior, whom no target attacks, both are plain plate of their tooltip's stats.
#[test]
fn the_struck_effect_items_are_plain_on_a_character_no_target_attacks() {
    let none = warrior(&[]);
    let plate = warrior(&[(CHEST, ZEALOT_PLATE)]);
    run(&plate);
    assert_eq!(added(&plate, &none, "Armor"), 509.0);
    assert!(added(&plate, &none, "Strength") >= 13.0);
    let girdle = warrior(&[(WAIST, GIRDLE_OF_EXTRACTED_ANSWERS)]);
    run(&girdle);
    assert_eq!(added(&girdle, &none, "Armor"), 313.0);
    assert!(added(&girdle, &none, "Strength") >= 12.0);
    assert!(added(&girdle, &none, "Stamina") >= 15.0);
}

/// The Protection Paladin tanks the target, has mana and holds a shield, so both effects would
/// act: each item refuses and names its effect.
#[test]
fn the_struck_effect_items_refuse_a_character_they_reach() {
    let reason = refusal(&paladin(&[(CHEST, ZEALOT_PLATE)]), Rules::Forever);
    assert!(
        reason.contains("item 274651 has an effect not prepared yet (mana when struck"),
        "{reason}"
    );
    let reason = refusal(
        &paladin(&[(WAIST, GIRDLE_OF_EXTRACTED_ANSWERS)]),
        Rules::Forever,
    );
    assert!(
        reason.contains("item 276742 has an effect not prepared yet (damage on block"),
        "{reason}"
    );
}

/// The same paladin when no target attacks them is never struck: both items are plain. So is the
/// girdle on a tank who holds no shield, who cannot block.
#[test]
fn the_struck_effect_items_are_plain_where_the_effect_cannot_act() {
    for (slot, id) in [(CHEST, ZEALOT_PLATE), (WAIST, GIRDLE_OF_EXTRACTED_ANSWERS)] {
        let mut request = paladin(&[(slot, id)]);
        request["raid"].as_object_mut().unwrap().remove("tanks");
        run(&request);
    }
    let mut no_shield = paladin(&[(WAIST, GIRDLE_OF_EXTRACTED_ANSWERS)]);
    player(&mut no_shield)["equipment"]["items"][OFF_HAND] = json!({});
    run(&no_shield);
}

// ---------------------------------------------------------------------------------------------
// The reference's rules.
// ---------------------------------------------------------------------------------------------

#[test]
fn the_reference_rules_still_refuse_the_items() {
    for (slot, id) in [
        (CHEST, ZEALOT_PLATE),
        (WAIST, GIRDLE_OF_EXTRACTED_ANSWERS),
        (HEAD, DEFIAS_MASK),
        (TRINKET_1, HEART_OF_THE_PRIMEVAL),
    ] {
        let reason = refusal(&warrior(&[(slot, id)]), Rules::GoReference);
        assert!(
            reason.contains(&format!("no item with id {id}")),
            "{reason}"
        );
    }
}
