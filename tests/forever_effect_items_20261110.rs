//! Two Forever effect items that refused real players' sims on 9 November 2026, each read from a
//! production request (fixtures/forever-gaps, names replaced, 50 iterations). Their client spell
//! rows are partly encrypted; `src/data/forever_client_spells.rs` says where each number comes
//! from.
//!
//! - Serpentbane Crown (274609), head: "+8 Nature Resistance", and "Use: Inflict 89 to 112 Fire
//!   damage to your current target and an additional 225 damage over 10 sec. Inflicts 2 times as
//!   much damage to Beast and Serpentine targets. (5 Min Cooldown)" (spell 1295039).
//! - Bigger Rock on Bigger Stick (279480), two-hand mace: "Chance on hit: Bash the target in the
//!   face, causing 14 Physical damage every 2 sec for 20 sec." (spell 1306816). Rock on Stick
//!   (279852), its one-hand twin, deals 7 a tick (spell 1309967).

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const SERPENTBANE_CROWN: i64 = 274609;
const SERPENTBANE: i64 = 1295039;
const BIGGER_ROCK: i64 = 279480;
const BIGGER_ROCK_TO_THE_FACE: i64 = 1306816;
const ROCK_ON_STICK: i64 = 279852;
const ROCK_TO_THE_FACE: i64 = 1309967;

const HEAD: usize = 0;
const MAIN_HAND: usize = 14;
const OFF_HAND: usize = 15;

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

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap_or(0.0)
}

/// The player's actions with this id (an item's use is its item id, a proc its spell id).
fn actions<'a>(player: &'a Value, field: &str, id: i64) -> Vec<&'a Value> {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"][field].as_i64() == Some(id))
        .collect()
}

/// Damage the player's actions with this id dealt, over every target.
fn damage(player: &Value, field: &str, id: i64) -> f64 {
    actions(player, field, id)
        .iter()
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["damage"].as_f64().unwrap_or(0.0))
        .sum()
}

fn with_target_type(mut request: Value, mob_type: &str) -> Value {
    request["encounter"]["targets"][0]["mobType"] = json!(mob_type);
    request
}

/// The reference's rules refuse an item they have no row for.
fn reference_refusal(request: &Value) -> String {
    match prepare(request, Rules::GoReference) {
        Err(PrepareError::Refused(refusal)) => refusal.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The request with every other Forever item (ids from 270000) taken off, in the equipment and
/// the character sheet, so the reference's refusal names this one.
fn only_forever_item(mut request: Value, keep: i64) -> Value {
    for pointer in ["/equipment/items", "/characterSheet/equipment/items"] {
        let Some(items) = player(&mut request).pointer_mut(pointer) else {
            continue;
        };
        for item in items.as_array_mut().unwrap() {
            if item["id"]
                .as_i64()
                .is_some_and(|id| id >= 270_000 && id != keep)
            {
                *item = json!({});
            }
        }
    }
    request
}

// ---------------------------------------------------------------------------------------------
// Serpentbane Crown.
// ---------------------------------------------------------------------------------------------

fn shaman() -> Value {
    request("enhancement-shaman-serpentbane-crown")
}

fn shaman_with_head(item: Value) -> Value {
    let mut request = shaman();
    player(&mut request)["equipment"]["items"][HEAD] = item;
    request
}

fn serpentbane_effect(prepared: &Value) -> Value {
    let found = effects(prepared, "damage_on_use");
    let found: Vec<_> = found
        .into_iter()
        .filter(|effect| effect["item_id"] == SERPENTBANE_CROWN)
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    found[0].clone()
}

/// The refused request prepares, passes the gate and runs, and the crown's use deals damage.
#[test]
fn the_serpentbane_crown_prepares_and_runs() {
    let (_, player) = run(&shaman());
    assert!(damage(&player, "itemId", SERPENTBANE_CROWN) > 0.0);
}

/// The row is the tooltip's: 213 armor, 16 Stamina and 8 Nature Resistance.
#[test]
fn the_crowns_row_carries_the_tooltips_stats() {
    let with = prepare(&shaman(), Rules::Forever).unwrap();
    let without = prepare(&shaman_with_head(json!({})), Rules::Forever).unwrap();
    let added = |name: &str| stat(&with, name) - stat(&without, name);
    assert_eq!(added("NatureResistance"), 8.0);
    assert!(added("Stamina") >= 16.0, "{}", added("Stamina"));
    assert!(added("Armor") >= 213.0, "{}", added("Armor"));
}

/// The use is a Fire hit of 100 (variance 0.23: 88.5 to 111.5, the tooltip's 89 to 112) on the
/// magic table with a crit, then five ticks of 45 every 2 sec, on a 5 minute cooldown that also
/// starts 10 sec of the shared on-use category.
#[test]
fn the_crowns_use_is_a_fire_hit_and_a_damage_over_time() {
    let prepared = prepare(&shaman(), Rules::Forever).unwrap();
    let effect = serpentbane_effect(&prepared);
    assert_eq!(effect["direct"]["average"], 100.0);
    assert_eq!(effect["direct"]["variance"], f64::from(0.23_f32));
    assert_eq!(effect["direct"]["scale"], 1.0);
    assert_eq!(effect["direct"]["outcome"], "magic_hit_and_crit");
    assert_eq!(effect["periodic"]["tick_base"], 45.0);
    let spell = &prepared["player"]["spells"][effect["spell"].as_u64().unwrap() as usize];
    assert_eq!(spell["action_id"]["item_id"], SERPENTBANE_CROWN);
    assert_eq!(spell["school"], 4);
    assert_eq!(spell["defense_type"], "DefenseTypeMagic");
    assert_eq!(spell["bonus_coefficient"], 0.0);
    assert_eq!(spell["cd"]["duration_ns"], 300_000_000_000_i64);
    assert_eq!(spell["shared_cd"]["duration_ns"], 10_000_000_000_i64);
    assert_eq!(spell["dot"]["base_tick_count"], 5);
    assert_eq!(spell["dot"]["base_tick_length_ns"], 2_000_000_000_i64);
    assert_eq!(spell["dot"]["bonus_coefficient"], 0.0);
}

/// Over a 120 second fight the crown is used once an iteration: one hit and the five ticks after
/// it, about 100 + 225 before crits and resists. Metrics total the 50 iterations.
#[test]
fn the_crown_is_used_once_in_a_two_minute_fight() {
    let request = shaman();
    let iterations = request["simOptions"]["iterations"].as_f64().unwrap();
    let (_, player) = run(&request);
    let uses = actions(&player, "itemId", SERPENTBANE_CROWN);
    assert!(!uses.is_empty());
    let casts: f64 = uses
        .iter()
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["casts"].as_f64().unwrap_or(0.0))
        .sum();
    assert_eq!(casts, iterations, "{uses:?}");
    let each = damage(&player, "itemId", SERPENTBANE_CROWN) / iterations;
    assert!((250.0..450.0).contains(&each), "{each}");
}

/// Twice the damage against a Beast, the hit and the ticks, and the same against any other type.
#[test]
fn the_crown_deals_twice_as_much_to_a_beast() {
    let beast = prepare(&with_target_type(shaman(), "MobTypeBeast"), Rules::Forever).unwrap();
    let effect = serpentbane_effect(&beast);
    assert_eq!(effect["direct"]["scale"], 2.0);
    assert_eq!(effect["periodic"]["tick_base"], 90.0);
    for other in ["MobTypeHumanoid", "MobTypeDragonkin"] {
        let prepared = prepare(&with_target_type(shaman(), other), Rules::Forever).unwrap();
        let effect = serpentbane_effect(&prepared);
        assert_eq!(effect["direct"]["scale"], 1.0, "{other}");
        assert_eq!(effect["periodic"]["tick_base"], 45.0, "{other}");
    }
    let (_, player) = run(&with_target_type(shaman(), "MobTypeBeast"));
    let (_, plain) = run(&shaman());
    assert!(
        damage(&player, "itemId", SERPENTBANE_CROWN) > damage(&plain, "itemId", SERPENTBANE_CROWN)
    );
}

#[test]
fn the_reference_rules_still_refuse_the_crown() {
    let request = only_forever_item(shaman(), SERPENTBANE_CROWN);
    let reason = reference_refusal(&request);
    assert!(reason.contains("no item with id 274609"), "{reason}");
}

// ---------------------------------------------------------------------------------------------
// Bigger Rock on Bigger Stick and Rock on Stick.
// ---------------------------------------------------------------------------------------------

fn rock_proc<'a>(prepared: &'a Value, label: &str) -> Option<&'a Value> {
    effects(prepared, "spell_data_damage_proc")
        .into_iter()
        .find(|effect| effect["trigger_aura"] == label)
}

/// Both refused requests (a Warrior and a Feral Druid) prepare, pass the gate and run, and the
/// bash ticks on the target.
#[test]
fn bigger_rock_on_bigger_stick_prepares_and_runs() {
    for name in [
        "warrior-bigger-rock-on-bigger-stick",
        "feral-druid-bigger-rock-on-bigger-stick",
    ] {
        let (prepared, player) = run(&request(name));
        assert!(
            rock_proc(&prepared, "Bigger Rock on Bigger Stick").is_some(),
            "{name}"
        );
        assert!(
            damage(&player, "spellId", BIGGER_ROCK_TO_THE_FACE) > 0.0,
            "{name}"
        );
    }
}

/// The bash is a chance on hit with no stated rate, so it rolls the rate MythicSim assumes for one
/// (1.8% per second of the weapon's speed: 6.12% a swing at 3.40) on the weapon's own landed hits,
/// and puts ten ticks of 14 every 2 sec on the unit hit, which roll the physical crit (as every
/// Forever physical damage over time row states) and have no hit of their own.
#[test]
fn the_bash_is_a_physical_damage_over_time_on_the_weapons_proc_rate() {
    let prepared = prepare(
        &request("warrior-bigger-rock-on-bigger-stick"),
        Rules::Forever,
    )
    .unwrap();
    let proc = rock_proc(&prepared, "Bigger Rock on Bigger Stick").unwrap();
    assert_eq!(proc["landed_only"], true);
    let periodic = &proc["periodic"];
    assert_eq!(periodic["tick_base"], 14.0);
    assert_eq!(periodic["tick_outcome"], "tick_physical_crit");
    assert_eq!(periodic["with_direct"], false);
    assert!(periodic.get("application").is_none());
    let spell = &prepared["player"]["spells"][proc["spell"].as_u64().unwrap() as usize];
    assert_eq!(spell["action_id"]["spell_id"], BIGGER_ROCK_TO_THE_FACE);
    assert_eq!(spell["school"], 1);
    assert_eq!(spell["dot"]["base_tick_count"], 10);
    assert_eq!(spell["dot"]["base_tick_length_ns"], 2_000_000_000_i64);
    let main_hand = proc["chances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|chance| chance["chance"].as_f64().unwrap())
        .fold(0.0, f64::max);
    assert!((main_hand - 0.018 * 3.4).abs() < 1e-9, "{main_hand}");
}

/// The ticks take no armor, as every physical damage over time here: the same damage against no
/// armor and a great deal.
#[test]
fn the_bash_takes_no_armor() {
    let bash = |armor: f64| {
        let mut request = request("warrior-bigger-rock-on-bigger-stick");
        request["encounter"]["targets"][0]["stats"][30] = json!(armor);
        let (_, player) = run(&request);
        damage(&player, "spellId", BIGGER_ROCK_TO_THE_FACE)
    };
    let (soft, hard) = (bash(0.0), bash(12000.0));
    assert!(soft > 0.0);
    assert_eq!(soft, hard);
}

/// Rock on Stick, the one-hand twin, ticks for 7 and runs in either hand.
#[test]
fn rock_on_stick_ticks_for_seven() {
    let mut request = request("feral-druid-bigger-rock-on-bigger-stick");
    player(&mut request)["equipment"]["items"][MAIN_HAND] = json!({ "id": ROCK_ON_STICK });
    player(&mut request)["equipment"]["items"][OFF_HAND] = json!({});
    let (prepared, player) = run(&request);
    let proc = rock_proc(&prepared, "Rock on Stick").unwrap();
    assert_eq!(proc["periodic"]["tick_base"], 7.0);
    assert!(damage(&player, "spellId", ROCK_TO_THE_FACE) > 0.0);
}

#[test]
fn the_rows_carry_the_tooltips_weapon() {
    let prepared = prepare(
        &request("warrior-bigger-rock-on-bigger-stick"),
        Rules::Forever,
    )
    .unwrap();
    let main_hand = &prepared["melee"]["main_hand"];
    assert_eq!(main_hand["base_damage_min"], 112.0);
    assert_eq!(main_hand["base_damage_max"], 168.0);
    assert_eq!(main_hand["swing_speed"], 3.4);
}

#[test]
fn the_reference_rules_still_refuse_the_rock() {
    let request = only_forever_item(request("warrior-bigger-rock-on-bigger-stick"), BIGGER_ROCK);
    let reason = reference_refusal(&request);
    assert!(reason.contains("no item with id 279480"), "{reason}");
}
