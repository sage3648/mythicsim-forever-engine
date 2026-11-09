//! Four item gaps that refused real players' sims on 9 November 2026, each read from a
//! production request (fixtures/forever-gaps, names replaced, 50 iterations):
//!
//! - Ceremonial Mallet (279885), a Forever item whose ItemSparse row the client ships encrypted:
//!   Wowhead's Forever tooltip reads Two-Hand Mace, item level 42, 107 - 161 damage, speed 3.60,
//!   +15 Stamina, "Equip: +22 Attack Power" and "Equip: +33 Attack Power against Undead".
//! - Defias Leather (set 161), four pieces: "Grants your melee attacks a 5% chance when striking
//!   from behind to hit a vital point, causing the target to Bleed for 68 to 83 Physical damage
//!   over 5 sec" (client set spell 1292028, bleed 1292029).
//! - Guardian's Dualblade (273046): "Equip: When damaged, has a chance to grant you 1080 armor
//!   for 12 sec. (Proc chance: 33%, 1m cooldown)" (trigger 1293342, buff 1293345).
//! - Sword of Zeal (6622) as the weapon of an item swap.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, prepared_refusals,
    simulate_prepared, PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn request(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/forever-gaps/{name}.request.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

fn prepare(request: &Value, rules: Rules) -> Result<Value, PrepareError> {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
}

/// The request under Forever's rules, prepared, through the gate and run: the prepared input and
/// the player's metrics.
fn run(request: &Value) -> (Value, Value) {
    let value = prepare(request, Rules::Forever).unwrap_or_else(|err| panic!("{err}"));
    let prepared: PreparedV2 = serde_json::from_value(value.clone()).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    let result = simulate_prepared(&prepared).unwrap().result;
    let player = result["raidMetrics"]["parties"][0]["players"][0].clone();
    assert!(player["dps"]["avg"].as_f64().unwrap() > 0.0);
    (value, player)
}

fn effects<'a>(prepared: &'a Value, kind: &str) -> Vec<&'a Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == kind)
        .collect()
}

fn dps(player: &Value) -> f64 {
    player["dps"]["avg"].as_f64().unwrap()
}

/// The reasons the gate gives a prepared input.
fn gate_reasons(prepared: &Value) -> Vec<String> {
    let typed: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    prepared_refusals(&typed)
        .into_iter()
        .map(|refusal| refusal.reason)
        .collect()
}

/// Damage the player's action with this spell id dealt, over every target.
fn spell_damage(player: &Value, spell_id: i64) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"].as_i64() == Some(spell_id))
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["damage"].as_f64().unwrap_or(0.0))
        .sum()
}

/// Seconds the player had the aura of this spell up, on average over the iterations.
fn aura_uptime(player: &Value, spell_id: i64) -> f64 {
    player["auras"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|aura| aura["id"]["spellId"].as_i64() == Some(spell_id))
        .map(|aura| aura["uptimeSecondsAvg"].as_f64().unwrap_or(0.0))
        .sum()
}

/// A Retribution Paladin (the request without a Manual Crowd Pummeler) wearing this item in the
/// main hand, with no buffs, on Forever's talent tree.
fn forever_paladin(main_hand: Value) -> Value {
    let mut request = request("retribution-paladin-swap-without-crowd-pummeler");
    let paladin = player(&mut request);
    paladin["equipment"]["items"][MAIN_HAND] = main_hand;
    paladin["buffs"] = json!({});
    request["raid"]["buffs"] = json!({});
    request["raid"]["parties"][0]["buffs"] = json!({});
    request
}

/// The same Paladin on the reference's talent tree, which its rules prepare.
fn reference_paladin(main_hand: Value) -> Value {
    let mut request = forever_paladin(main_hand);
    player(&mut request)["talentsString"] = json!("52003-503-05225231001330321");
    request
}

/// The reference's rules refuse a request in preparation or at the gate: the reasons.
fn reference_refusals(request: &Value) -> Vec<String> {
    match prepare(request, Rules::GoReference) {
        Ok(prepared) => gate_reasons(&prepared),
        Err(PrepareError::Refused(refusal)) => vec![refusal.to_string()],
        Err(other) => panic!("{other}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Ceremonial Mallet.
// ---------------------------------------------------------------------------------------------

const MALLET: i64 = 279885;
const MAIN_HAND: usize = 14;

fn with_target_type(mut request: Value, mob_type: &str) -> Value {
    request["encounter"]["targets"][0]["mobType"] = json!(mob_type);
    request
}

/// The Enhancement Shaman of the refused request wears the mallet in the main hand: the engine
/// refused it as an item with no id.
#[test]
fn the_ceremonial_mallet_prepares_and_runs() {
    let request = request("enhancement-shaman-ceremonial-mallet");
    let items = &request["raid"]["parties"][0]["players"][0]["equipment"]["items"];
    assert!(items
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == MALLET));
    run(&request);
}

/// The tooltip's plain stats are the item's: 15 Stamina and 22 attack power, with nothing from
/// the creature type line on a target of no type.
#[test]
fn the_mallets_row_carries_the_tooltips_stats() {
    let stats = |main_hand: Value| {
        let prepared = prepare(&forever_paladin(main_hand), Rules::Forever).unwrap();
        let stat = |name: &str| prepared["player"]["stats"][name].as_f64().unwrap();
        (stat("Stamina"), stat("AttackPower"))
    };
    let (worn, bare) = (stats(json!({ "id": MALLET })), stats(json!({})));
    assert_eq!(worn.0 - bare.0, 15.0, "Stamina");
    assert_eq!(worn.1 - bare.1, 22.0, "Attack Power");
}

/// "+33 Attack Power against Undead": melee attack power on a target of that type alone. A
/// target of another type or of none (the app's requests name none) takes nothing, as the
/// reference's Mark of the Champion.
#[test]
fn the_mallets_bonus_applies_against_undead_only() {
    let base = request("enhancement-shaman-ceremonial-mallet");
    let undead = with_target_type(base.clone(), "MobTypeUndead");
    let humanoid = with_target_type(base.clone(), "MobTypeHumanoid");

    let (prepared, undead_player) = run(&undead);
    let bonus = effects(&prepared, "mob_type_bonus");
    assert_eq!(bonus.len(), 1, "{bonus:?}");
    assert_eq!(bonus[0]["aura"], "Ceremonial Mallet");
    assert_eq!(bonus[0]["melee_attack_power"], 33.0);

    for other in [humanoid, base] {
        let (prepared, player) = run(&other);
        assert!(effects(&prepared, "mob_type_bonus").is_empty());
        assert!(
            dps(&undead_player) > dps(&player),
            "Undead {} against {}",
            dps(&undead_player),
            dps(&player)
        );
    }
}

/// The mallet in a swap set follows the weapon in and out of the hand: the aura is one of the
/// swap's callbacks, and the bonus is described against an Undead target.
#[test]
fn the_mallet_follows_an_item_swap() {
    let mut request = with_target_type(
        request("retribution-paladin-swap-sword-of-zeal"),
        "MobTypeUndead",
    );
    player(&mut request)["itemSwap"]["items"][MAIN_HAND] = json!({ "id": MALLET });
    let (prepared, _) = run(&request);
    assert_eq!(effects(&prepared, "mob_type_bonus").len(), 1);
    let swap = effects(&prepared, "item_swap");
    assert!(
        swap[0]["callbacks"]
            .to_string()
            .contains("Ceremonial Mallet"),
        "{}",
        swap[0]["callbacks"]
    );
}

/// The reference's rules know no such item.
#[test]
fn the_reference_rules_still_refuse_the_mallet() {
    let reasons = reference_refusals(&reference_paladin(json!({ "id": MALLET })));
    assert_eq!(reasons, ["unknown_item: no item with id 279885"]);
}

// ---------------------------------------------------------------------------------------------
// Defias Leather.
// ---------------------------------------------------------------------------------------------

const DEVIOUS_STRIKE_BLEED: i64 = 1292029;
/// The fixtures wear four pieces; the feet are the set's fifth.
const DEFIAS_BOOTS: i64 = 10402;
const FEET: usize = 8;

fn defias_proc(prepared: &Value) -> Option<&Value> {
    effects(prepared, "spell_data_damage_proc")
        .into_iter()
        .find(|effect| effect["trigger_aura"] == "Defias Leather 4P")
}

/// Four pieces strike from behind with a 5% chance of a bleed of 15 a second for 5 seconds on
/// the unit hit, which ticks on current stats, rolls the physical crit as its row states
/// (Periodic Can Crit) and takes no armor.
#[test]
fn defias_leather_four_pieces_bleed_the_target() {
    for name in [
        "assassination-rogue-defias-leather",
        "feral-druid-defias-leather",
    ] {
        let (prepared, player) = run(&request(name));
        let proc = defias_proc(&prepared).unwrap_or_else(|| panic!("{name}: no bleed proc"));
        assert_eq!(proc["proc_chance"], 0.05, "{name}");
        assert_eq!(proc["landed_only"], true, "{name}");
        let periodic = &proc["periodic"];
        assert_eq!(periodic["tick_base"], 15.0, "{name}");
        assert_eq!(periodic["tick_outcome"], "tick_physical_crit", "{name}");
        assert_eq!(periodic["with_direct"], false, "{name}");
        let spell = &prepared["player"]["spells"][proc["spell"].as_u64().unwrap() as usize];
        assert_eq!(
            spell["action_id"]["spell_id"], DEVIOUS_STRIKE_BLEED,
            "{name}"
        );
        assert!(spell_damage(&player, DEVIOUS_STRIKE_BLEED) > 0.0, "{name}");
    }
}

/// Five ticks of 15 are 75 a proc, inside the tooltip's 68 to 83, and the bleed does not feel
/// the target's armor: the same damage against none and against a great deal.
#[test]
fn the_bleed_takes_no_armor() {
    let damage = |armor: f64| {
        let mut request = request("assassination-rogue-defias-leather");
        request["encounter"]["targets"][0]["stats"][30] = json!(armor);
        let (_, player) = run(&request);
        spell_damage(&player, DEVIOUS_STRIKE_BLEED)
    };
    let (soft, hard) = (damage(0.0), damage(12000.0));
    assert!(soft > 0.0);
    assert_eq!(soft, hard);
}

/// Three pieces have no bleed; a wearer who stands in front of the target has none either (its
/// listener is not registered), and both still pass the gate.
#[test]
fn the_bleed_needs_four_pieces_and_a_wearer_behind() {
    let mut three = request("assassination-rogue-defias-leather");
    let items = player(&mut three)["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    let chest = items.iter().position(|item| item["id"] == 10399).unwrap();
    items[chest] = json!({});
    let (prepared, _) = run(&three);
    assert!(defias_proc(&prepared).is_none());

    // A Rogue cannot Backstab from the front, so this one only prepares and passes the gate.
    let mut front = request("assassination-rogue-defias-leather");
    player(&mut front)["inFrontOfTarget"] = json!(true);
    let prepared = prepare(&front, Rules::Forever).unwrap();
    assert!(defias_proc(&prepared).is_none());
    let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
    check_prepared(&typed).unwrap();
}

/// The set's other bonuses: the second piece's 5 Arcane Resistance and the third's 15 attack
/// power against humanoids (inert against a target of no type), and the fifth's dagger skill.
#[test]
fn the_other_defias_leather_bonuses_are_handled() {
    let resistance = |pieces: usize| {
        let mut request = request("assassination-rogue-defias-leather");
        let items = player(&mut request)["equipment"]["items"]
            .as_array_mut()
            .unwrap();
        let worn: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| (10399..=10403).contains(&item["id"].as_i64().unwrap_or(0)))
            .map(|(slot, _)| slot)
            .collect();
        for &slot in worn.iter().skip(pieces) {
            items[slot] = json!({});
        }
        let prepared = prepare(&request, Rules::Forever).unwrap();
        prepared["player"]["stats"]["ArcaneResistance"]
            .as_f64()
            .unwrap()
    };
    assert_eq!(resistance(2) - resistance(1), 5.0);
    assert_eq!(resistance(4) - resistance(2), 0.0);

    // A fifth piece joins the four: Increased Daggers +1 and the bleed both stay.
    let mut five = request("assassination-rogue-defias-leather");
    player(&mut five)["equipment"]["items"][FEET] = json!({ "id": DEFIAS_BOOTS });
    let (prepared, _) = run(&five);
    assert!(defias_proc(&prepared).is_some());
}

/// The reference's set has no fourth piece bonus: four pieces give a Paladin on its tree the
/// second piece's resistance and nothing else, so there is no listener to claim.
#[test]
fn the_reference_rules_have_no_four_piece_bleed() {
    let mut request = reference_paladin(json!({ "id": 9449 }));
    // Chest, hands, waist and legs.
    for (slot, id) in [(3, 10399), (5, 10401), (6, 10403), (7, 10400)] {
        player(&mut request)["equipment"]["items"][slot] = json!({ "id": id });
    }
    let prepared = prepare(&request, Rules::GoReference).unwrap();
    let labels: Vec<&str> = prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|aura| aura["label"].as_str())
        .filter(|label| label.starts_with("Defias Leather"))
        .collect();
    assert_eq!(labels, ["Defias Leather 2P"]);
    assert!(defias_proc(&prepared).is_none());
}

// ---------------------------------------------------------------------------------------------
// Guardian's Dualblade.
// ---------------------------------------------------------------------------------------------

const DUALBLADE_PROC: &str = "Guardian's Dualblade Proc";
/// Guardian's Durability (1293345), the aura the proc puts up.
const DUALBLADE_BUFF: i64 = 1293345;

fn tank(mut request: Value) -> Value {
    request["raid"]["tanks"] = json!([{ "index": 0, "type": "Player" }]);
    request
}

/// Nothing attacks a damage dealer, so the Dualblade's aura can never fire: the engine claims it
/// as inert, where it refused the request, and adds no stat aura for a proc that cannot happen.
#[test]
fn the_dualblade_is_inert_when_nothing_damages_the_wearer() {
    let (prepared, _) = run(&request("retribution-paladin-guardians-dualblade"));
    let inert: Vec<&Value> = effects(&prepared, "inert_listener")
        .into_iter()
        .filter(|effect| effect["aura"] == "Guardian's Dualblade")
        .collect();
    assert_eq!(inert.len(), 1);
    assert!(effects(&prepared, "spell_data_stat_proc")
        .iter()
        .all(|effect| effect["trigger_aura"] != "Guardian's Dualblade"));
    let stat_auras = effects(&prepared, "stat_auras");
    assert!(
        stat_auras
            .iter()
            .all(|effect| !effect["auras"].to_string().contains(DUALBLADE_PROC)),
        "{stat_auras:?}"
    );
}

/// A tank takes the target's swings: each landed hit rolls the 33% chance, behind the item's
/// minute of cooldown, for 1080 armor for 12 seconds.
#[test]
fn the_dualblade_gives_a_tank_armor_when_damaged() {
    let (prepared, player) = run(&tank(request("retribution-paladin-guardians-dualblade")));
    let proc: Vec<&Value> = effects(&prepared, "spell_data_stat_proc")
        .into_iter()
        .filter(|effect| effect["trigger_aura"] == "Guardian's Dualblade")
        .collect();
    assert_eq!(proc.len(), 1, "{proc:?}");
    assert_eq!(proc[0]["aura"], DUALBLADE_PROC);
    assert_eq!(proc[0]["proc_chance"], 0.33);
    assert_eq!(proc[0]["struck"], true);
    assert_eq!(proc[0]["callbacks"], json!(["on_spell_hit_taken"]));
    assert!(
        proc[0]["gain_log"]
            .as_str()
            .unwrap()
            .contains("\"Armor\": 1080"),
        "{}",
        proc[0]["gain_log"]
    );
    // The armor changes what the target's swings do, so the aura is one of the stat auras.
    let stat_auras = effects(&prepared, "stat_auras");
    let armor: Vec<&&Value> = stat_auras
        .iter()
        .filter(|effect| effect["auras"].to_string().contains(DUALBLADE_PROC))
        .collect();
    assert_eq!(armor.len(), 1, "{stat_auras:?}");
    assert!(armor[0]["changed"].to_string().contains("Armor"));
    // About twice a two minute fight, 12 seconds each.
    let up = aura_uptime(&player, DUALBLADE_BUFF);
    assert!((12.0..=36.0).contains(&up), "armor up {up} seconds");
}

/// The proc's aura lasts 12 seconds and sits behind a minute of cooldown, shared with its
/// trigger.
#[test]
fn the_dualblades_proc_lasts_12_seconds_behind_a_minute_of_cooldown() {
    let prepared = prepare(
        &tank(request("retribution-paladin-guardians-dualblade")),
        Rules::Forever,
    )
    .unwrap();
    let auras = prepared["player"]["auras"].as_array().unwrap();
    let aura = |label: &str| auras.iter().find(|aura| aura["label"] == label).unwrap();
    assert_eq!(aura(DUALBLADE_PROC)["duration_ns"], 12_000_000_000_i64);
    assert_eq!(
        aura("Guardian's Dualblade")["icd"]["duration_ns"],
        60_000_000_000_i64
    );
}

/// The reference's rules know no entry for it and keep refusing the aura.
#[test]
fn the_reference_rules_still_refuse_the_dualblade() {
    let reasons = reference_refusals(&reference_paladin(json!({ "id": 273046 })));
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("Guardian's Dualblade")),
        "{reasons:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Sword of Zeal in an item swap.
// ---------------------------------------------------------------------------------------------

const ZEAL: i64 = 6622;
/// Zeal (8191), the aura Sword of Zeal puts up.
const ZEAL_BUFF: i64 = 8191;
const ARGENT_AVENGER: i64 = 13246;

fn swapping_to(item: i64) -> Value {
    let mut request = request("retribution-paladin-swap-sword-of-zeal");
    player(&mut request)["itemSwap"]["items"][MAIN_HAND] = json!({ "id": item });
    request
}

/// The Manual Crowd Pummeler opener swaps onto Sword of Zeal: the weapon's proc follows it into
/// the hand and puts up Zeal (8191: 150 armor and 10 Shadow resistance for 15 seconds) there.
#[test]
fn a_swap_onto_sword_of_zeal_runs_and_procs_zeal() {
    let (prepared, player) = run(&swapping_to(ZEAL));
    let proc: Vec<&Value> = effects(&prepared, "stat_proc")
        .into_iter()
        .filter(|effect| effect["trigger_aura"] == "Sword of Zeal Proc")
        .collect();
    assert_eq!(proc.len(), 1, "{proc:?}");
    assert!(aura_uptime(&player, ZEAL_BUFF) > 0.0, "Zeal never came up");
    // The swap's callbacks switch the trigger with the weapon.
    let swap = effects(&prepared, "item_swap");
    assert_eq!(swap.len(), 1);
    assert!(
        swap[0]["callbacks"]
            .to_string()
            .contains("Sword of Zeal Proc"),
        "{}",
        swap[0]["callbacks"]
    );
}

/// The proc only rolls while the sword is in the hand: the opener's first 30 seconds hold the
/// Pummeler, so Zeal's procs come from the rest of the fight alone.
#[test]
fn zeal_does_not_proc_before_the_swap() {
    let (_, swapped) = run(&swapping_to(ZEAL));
    let mut never = swapping_to(ZEAL);
    player(&mut never)["enableItemSwap"] = json!(false);
    player(&mut never)["equipment"]["items"][MAIN_HAND] = json!({ "id": 9449 });
    let (_, pummeler) = run(&never);
    assert_eq!(aura_uptime(&pummeler, ZEAL_BUFF), 0.0);
    assert!(aura_uptime(&swapped, ZEAL_BUFF) > 0.0);
}

/// Argent Avenger is registered through the same helper and follows a swap the same way.
#[test]
fn a_swap_onto_argent_avenger_runs() {
    let (_, player) = run(&swapping_to(ARGENT_AVENGER));
    assert!(
        aura_uptime(&player, 17352) > 0.0,
        "Argent Avenger never procced"
    );
}

/// The reference never registered the swap callback and keeps refusing the swap.
#[test]
fn the_reference_rules_still_refuse_the_zeal_swap() {
    let mut request = reference_paladin(json!({ "id": 9449 }));
    player(&mut request)["enableItemSwap"] = json!(true);
    player(&mut request)["itemSwap"] = json!({
        "items": (0..17)
            .map(|slot| if slot == MAIN_HAND { json!({ "id": ZEAL }) } else { json!({}) })
            .collect::<Vec<_>>()
    });
    let reasons = reference_refusals(&request);
    assert_eq!(
        reasons,
        ["item_swap: an item swap with the effect of item 6622 is not prepared yet"]
    );
}
