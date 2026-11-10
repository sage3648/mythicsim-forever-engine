//! Dreadfrost Saber (274603), a Forever main hand sword that refused players' sims ("no item with
//! id 274603"): "Equip: Adds 1 Frost damage to your melee attacks." Its two client spell rows are
//! readable, and `src/data/forever_client_spells.rs` restates them field by field: the equip aura
//! 1294939 (a proc trigger aura on the main hand's melee hits, chance 100) and the hit it casts,
//! 1294940 (1 Frost damage on the magic table, no coefficient).
//!
//! No refused request was at hand, so the tests put the sword in the main hand of the level 46
//! Arms Warrior of `fixtures/forever-gaps/warrior-bigger-rock-on-bigger-stick` (50 iterations).

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const DREADFROST_SABER: i64 = 274603;
/// The hit the sword's equip aura casts.
const DREADFROST_HIT: i64 = 1294940;
/// A one-hand mace with no equip line, for the off hand.
const ROCK_ON_STICK: i64 = 279852;

const MAIN_HAND: usize = 14;
const OFF_HAND: usize = 15;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// The Warrior with these weapons.
fn warrior(main_hand: Value, off_hand: Value) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/forever-gaps/warrior-bigger-rock-on-bigger-stick.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    player(&mut request)["equipment"]["items"][MAIN_HAND] = main_hand;
    player(&mut request)["equipment"]["items"][OFF_HAND] = off_hand;
    request
}

/// The Warrior with the sword in the main hand and nothing in the off hand.
fn with_the_saber() -> Value {
    warrior(json!({ "id": DREADFROST_SABER }), json!({}))
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

fn saber_proc(prepared: &Value) -> Option<&Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["kind"] == "spell_data_damage_proc"
                && effect["trigger_aura"] == "Dreadfrost Saber"
        })
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap_or(0.0)
}

/// The player's actions with this spell id.
fn actions(player: &Value, id: i64) -> Vec<&Value> {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"].as_i64() == Some(id))
        .collect()
}

/// A field of the player's actions with this spell id, summed over every target.
fn total(player: &Value, id: i64, field: &str) -> f64 {
    actions(player, id)
        .iter()
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target[field].as_f64().unwrap_or(0.0))
        .sum()
}

/// The sword prepares, passes the gate and runs, and its Frost hit deals damage.
#[test]
fn the_dreadfrost_saber_prepares_and_runs() {
    let (prepared, player) = run(&with_the_saber());
    assert!(saber_proc(&prepared).is_some());
    assert!(total(&player, DREADFROST_HIT, "damage") > 0.0);
}

/// The row is the tooltip's: 46 to 86 at 2.40, and 8 Stamina.
#[test]
fn the_row_carries_the_tooltips_weapon_and_stamina() {
    let with = prepare(&with_the_saber(), Rules::Forever).unwrap();
    let main_hand = &with["melee"]["main_hand"];
    assert_eq!(main_hand["base_damage_min"], 46.0);
    assert_eq!(main_hand["base_damage_max"], 86.0);
    assert_eq!(main_hand["swing_speed"], 2.4);
    // The same request with a one-hand mace of no Stamina in the main hand.
    let without = prepare(
        &warrior(json!({ "id": ROCK_ON_STICK }), json!({})),
        Rules::Forever,
    )
    .unwrap();
    let added = stat(&with, "Stamina") - stat(&without, "Stamina");
    assert!(added >= 8.0, "{added}");
}

/// Every landed melee hit that deals damage casts the hit, with no roll: a Frost hit of 1 on the
/// magic table, with no variance and no spell power share, which can crit.
#[test]
fn the_proc_is_a_frost_hit_of_one_on_every_landed_melee_hit() {
    let prepared = prepare(&with_the_saber(), Rules::Forever).unwrap();
    let proc = saber_proc(&prepared).unwrap();
    assert_eq!(proc["proc_chance"], 1.0);
    assert_eq!(proc["landed_only"], true);
    assert_eq!(proc["require_damage"], true);
    assert_eq!(proc["average"], 1.0);
    assert_eq!(proc["variance"], 0.0);
    assert_eq!(proc["can_crit"], true);
    assert!(proc.get("chances").is_none());
    assert!(proc.get("periodic").is_none());
    let spell = &prepared["player"]["spells"][proc["spell"].as_u64().unwrap() as usize];
    assert_eq!(spell["action_id"]["spell_id"], DREADFROST_HIT);
    assert_eq!(spell["school"], 16);
    assert_eq!(spell["defense_type"], "DefenseTypeMagic");
    assert_eq!(spell["bonus_coefficient"], 0.0);
}

/// The trigger row names the main hand (the second proc mask word's 32), so the main hand's
/// swings and strikes cast the hit and the off hand's never do.
#[test]
fn only_the_main_hands_hits_cast_it() {
    let request = warrior(
        json!({ "id": DREADFROST_SABER }),
        json!({ "id": ROCK_ON_STICK }),
    );
    let prepared = prepare(&request, Rules::Forever).unwrap();
    let proc = saber_proc(&prepared).unwrap();
    let spells = prepared["player"]["spells"].as_array().unwrap();
    let masks = |position: &Value| -> Vec<String> {
        spells[position.as_u64().unwrap() as usize]["proc_mask"]
            .as_array()
            .unwrap()
            .iter()
            .map(|mask| mask.as_str().unwrap().to_string())
            .collect()
    };
    let triggers = proc["trigger_spells"].as_array().unwrap();
    assert!(!triggers.is_empty());
    let mut main_hand_swing = false;
    for position in triggers {
        let masks = masks(position);
        assert!(
            masks.iter().any(|mask| mask.contains("MeleeMH")),
            "{position}: {masks:?}"
        );
        main_hand_swing |= masks.iter().any(|mask| mask == "ProcMaskMeleeMHAuto");
    }
    assert!(main_hand_swing);
    // The off hand swings, and its swing is no trigger.
    let off_hand_swing = spells
        .iter()
        .position(|spell| spell["proc_mask"] == json!(["ProcMaskMeleeOHAuto"]))
        .expect("an off hand swing");
    assert!(!triggers.contains(&json!(off_hand_swing)));
    run(&request);
}

/// In a fight the hit is cast about once per landed melee hit and rolls the magic hit table, so
/// some miss; one that lands deals 1, or a crit's 1.5, less what the target resists. Metrics
/// total the 50 iterations.
#[test]
fn the_hit_adds_about_one_damage_a_melee_hit() {
    let request = with_the_saber();
    let iterations = request["simOptions"]["iterations"].as_f64().unwrap();
    let (_, player) = run(&request);
    let hits = total(&player, DREADFROST_HIT, "hits") + total(&player, DREADFROST_HIT, "crits");
    let misses = total(&player, DREADFROST_HIT, "misses");
    let damage = total(&player, DREADFROST_HIT, "damage");
    // A two minute fight at 2.40 speed is 50 swings before haste, strikes and misses.
    assert!(hits / iterations > 20.0, "{hits}");
    assert!(misses > 0.0 && misses < hits, "{misses} of {hits}");
    let each = damage / hits;
    assert!((0.5..=1.5).contains(&each), "{each}");
}

/// The reference's rules have no row for the item and refuse it as unknown.
#[test]
fn the_reference_rules_still_refuse_the_saber() {
    let mut request = with_the_saber();
    // Every other Forever item (ids from 270000) comes off, in the equipment and the character
    // sheet, so the refusal names this one.
    for pointer in ["/equipment/items", "/characterSheet/equipment/items"] {
        let Some(items) = player(&mut request).pointer_mut(pointer) else {
            continue;
        };
        for item in items.as_array_mut().unwrap() {
            if item["id"]
                .as_i64()
                .is_some_and(|id| id >= 270_000 && id != DREADFROST_SABER)
            {
                *item = json!({});
            }
        }
    }
    match prepare(&request, Rules::GoReference) {
        Err(PrepareError::Refused(refusal)) => {
            let reason = refusal.to_string();
            assert!(reason.contains("no item with id 274603"), "{reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}
