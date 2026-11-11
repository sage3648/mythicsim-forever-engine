//! Toofbreaker (286430), a Forever one-hand mace that refused a Combat Rogue's Quick Sims twice on
//! 10 November 2026 ("no item with id 286430"), under Forever's rules.
//!
//! The live tooltip (Wowhead's Forever tooltip, 11 November 2026): Item Level 45, One-Hand Mace,
//! 40 - 75 Damage, Speed 1.90, +7 Strength, +5 Stamina, "Chance on hit: Bash the target, causing
//! 42 Physical damage. Deals 2 times as much damage if the target is facing you."
//!
//! The client rows (wago.tools `product=wowf`, client 1.60.1.70888, 11 November 2026; none is
//! encrypted): ItemEffect 238089 is a chance on hit (trigger type 2) of Toofbreak (1320269) with
//! no rate and no cooldown, and the spell is a Physical School Damage of 42 on the melee table
//! with no variance and no coefficient (SpellEffect 1354774, SpellMisc 866009, SpellCategories
//! 189061) beside a dummy effect of 2, the tooltip's "2 times" (SpellEffect 1354775).
//! `src/data/forever_client_spells.rs` restates the row, which the pinned store lacks.
//!
//! No rate is stated, so the bash rolls the rate MythicSim assumes for a chance on hit without
//! one (`shared_items::CHANCE_ON_HIT_PPM`, 1.8% per second of the weapon's speed), as the Rock on
//! Stick weapons do. The hit is doubled for a character the target faces: one in front of it
//! (the request's `inFrontOfTarget`) or one it swings at.
//!
//! No refused request was at hand, so the tests put the mace in the main hand of the Combat Rogue
//! of `fixtures/forever-gaps/combat-rogue-self-casts-at-player` and of the Protection Paladin of
//! `fixtures/forever-gaps/protection-paladin-quel-serrar`, who tanks the target (50 iterations
//! each).

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/rogue_113483.rs"]
mod rogue_113483;

const TOOFBREAKER: i64 = 286430;
/// The bash the mace's chance on hit casts.
const TOOFBREAK: i64 = 1320269;

const MAIN_HAND: usize = 14;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// A fixture's request with this item in the main hand.
fn wielding(fixture: &str, main_hand: Value) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/forever-gaps/{fixture}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    player(&mut request)["equipment"]["items"][MAIN_HAND] = main_hand;
    request
}

/// The Combat Rogue with this main hand, in front of the target or behind it. The fixture's
/// talents are read in the trees Forever's rules prepare.
fn rogue(main_hand: Value, in_front: bool) -> Value {
    let mut request = wielding("combat-rogue-self-casts-at-player", main_hand);
    player(&mut request)["inFrontOfTarget"] = json!(in_front);
    rogue_113483::forever_request(&mut request);
    request
}

/// The Protection Paladin, who tanks the target, with the mace.
fn paladin() -> Value {
    wielding(
        "protection-paladin-quel-serrar",
        json!({ "id": TOOFBREAKER }),
    )
}

fn prepare(request: &Value, rules: Rules) -> Result<Value, PrepareError> {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
}

fn prepared(request: &Value) -> Value {
    prepare(request, Rules::Forever).unwrap_or_else(|err| panic!("{err}"))
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

fn bash_proc(prepared: &Value) -> &Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["kind"] == "spell_data_damage_proc" && effect["trigger_aura"] == "Toofbreaker"
        })
        .expect("the chance on hit")
}

/// The spell the chance on hit casts, by its spellbook position.
fn bash_spell(prepared: &Value) -> &Value {
    let position = bash_proc(prepared)["spell"].as_u64().unwrap() as usize;
    &prepared["player"]["spells"][position]
}

/// The mace prepares, passes the gate and runs, in front of the target and behind it, and the
/// bash shows in the player's results.
#[test]
fn the_toofbreaker_prepares_and_runs() {
    for in_front in [true, false] {
        let request = rogue(json!({ "id": TOOFBREAKER }), in_front);
        let player = simulate(&prepared(&request));
        assert!(total(&player, TOOFBREAK, "damage") > 0.0, "{in_front}");
        assert!(total(&player, TOOFBREAK, "hits") > 0.0, "{in_front}");
    }
}

/// The row is the tooltip's: 40 to 75 at 1.90, 7 Strength and 5 Stamina over an empty main hand.
#[test]
fn the_row_carries_the_tooltips_weapon() {
    // Without the fixture's consumables, whose sharpening stone adds weapon damage.
    let bare = |main_hand: Value| {
        let mut request = rogue(main_hand, false);
        player(&mut request)["consumables"] = json!({});
        prepared(&request)
    };
    let with = bare(json!({ "id": TOOFBREAKER }));
    let without = bare(json!({}));
    let main_hand = &with["melee"]["main_hand"];
    assert_eq!(main_hand["base_damage_min"], 40.0);
    assert_eq!(main_hand["base_damage_max"], 75.0);
    assert_eq!(main_hand["swing_speed"], 1.9);
    assert!(stat(&with, "Strength") - stat(&without, "Strength") >= 7.0);
    assert!(stat(&with, "Stamina") - stat(&without, "Stamina") >= 5.0);
}

/// The bash is the client row's: a Physical hit of 42 on the melee table, with no variance, no
/// damage over time and no attack power or spell power share, cast on the unit hit. No rate is
/// stated, so the weapon's own landed hits roll the assumed 1.8% per second of its speed: 3.42% a
/// swing at 1.90.
#[test]
fn the_bash_is_a_physical_hit_of_forty_two_on_the_weapons_proc_rate() {
    let prepared = prepared(&rogue(json!({ "id": TOOFBREAKER }), false));
    let proc = bash_proc(&prepared);
    assert_eq!(proc["landed_only"], true);
    assert_eq!(proc["average"], 42.0);
    assert_eq!(proc["variance"], 0.0);
    assert_eq!(proc["outcome"], "melee_special_hit_and_crit");
    assert!(proc.get("periodic").is_none());
    assert!(proc.get("area").is_none());
    assert!(proc.get("chain").is_none());
    let spell = bash_spell(&prepared);
    assert_eq!(spell["action_id"]["spell_id"], TOOFBREAK);
    assert_eq!(spell["school"], 1);
    assert_eq!(spell["defense_type"], "DefenseTypeMelee");
    assert_eq!(spell["bonus_coefficient"], 0.0);
    let chances: Vec<f64> = proc["chances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|chance| chance["chance"].as_f64().unwrap())
        .collect();
    assert!(!chances.is_empty());
    for chance in chances {
        assert!((chance - 0.018 * 1.9).abs() < 1e-9, "{chance}");
    }
}

/// "Deals 2 times as much damage if the target is facing you": the bash's damage multiplier is 2
/// for a character in front of the target and 1 for one behind it, and nothing else about the
/// proc or its spell differs.
#[test]
fn the_bash_is_doubled_in_front_of_the_target_and_not_behind_it() {
    let front = prepared(&rogue(json!({ "id": TOOFBREAKER }), true));
    let behind = prepared(&rogue(json!({ "id": TOOFBREAKER }), false));
    assert_eq!(bash_spell(&front)["damage_multiplier"], 2.0);
    assert_eq!(bash_spell(&behind)["damage_multiplier"], 1.0);
    assert_eq!(bash_proc(&front), bash_proc(&behind));
    let mut same = bash_spell(&front).clone();
    same["damage_multiplier"] = json!(1.0);
    assert_eq!(&same, bash_spell(&behind));
}

/// In the fight the multiplier is all of the doubling: the fight from behind with the bash's
/// multiplier raised to 2 lands the same bashes, roll for roll, for twice the damage. The raid's
/// debuffs are taken off, since Gift of Arthas adds a flat 8 to each physical hit after the
/// multiplier. (In front of the target a bash may also be blocked, which takes a flat amount off
/// after the doubling.)
#[test]
fn the_multiplier_doubles_every_bash() {
    let mut request = rogue(json!({ "id": TOOFBREAKER }), false);
    request["raid"]["debuffs"] = json!({});
    let behind = prepared(&request);
    let position = bash_proc(&behind)["spell"].as_u64().unwrap() as usize;
    let mut doubled = behind.clone();
    doubled["player"]["spells"][position]["damage_multiplier"] = json!(2.0);
    let (single, doubled) = (simulate(&behind), simulate(&doubled));
    let hits = total(&single, TOOFBREAK, "hits");
    assert!(hits > 0.0);
    assert_eq!(total(&doubled, TOOFBREAK, "hits"), hits);
    assert_eq!(
        total(&doubled, TOOFBREAK, "crits"),
        total(&single, TOOFBREAK, "crits")
    );
    let ratio = total(&doubled, TOOFBREAK, "damage") / total(&single, TOOFBREAK, "damage");
    assert!((ratio - 2.0).abs() < 1e-9, "{ratio}");
}

/// A target faces the character it swings at: a tank's bash is doubled whatever the request says
/// of standing in front, and the same character who does not tank and stands behind deals the
/// row's amount.
#[test]
fn a_tanks_bash_is_doubled() {
    let mut request = paladin();
    player(&mut request)["inFrontOfTarget"] = json!(false);
    let tanking = prepared(&request);
    assert_eq!(bash_spell(&tanking)["damage_multiplier"], 2.0);
    request["raid"].as_object_mut().unwrap().remove("tanks");
    let behind = prepared(&request);
    assert_eq!(bash_spell(&behind)["damage_multiplier"], 1.0);
    let player = simulate(&tanking);
    assert!(total(&player, TOOFBREAK, "damage") > 0.0);
}

/// The bash is a direct Physical hit, so the target's armor reduces it, where the Rock on Stick
/// weapons' ticks take none.
#[test]
fn the_bash_takes_armor() {
    let bash = |armor: f64| {
        let mut request = rogue(json!({ "id": TOOFBREAKER }), false);
        request["raid"]["debuffs"] = json!({});
        request["encounter"]["targets"][0]["stats"][30] = json!(armor);
        let player = simulate(&prepared(&request));
        let hit = &player["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["id"]["spellId"].as_i64() == Some(TOOFBREAK))
            .unwrap()["targets"][0]["hitRange"];
        (hit["min"].as_f64().unwrap(), hit["max"].as_f64().unwrap())
    };
    // Against no armor every hit that is not a crit is the row's 42.
    assert_eq!(bash(0.0), (42.0, 42.0));
    let (_, hard) = bash(12000.0);
    assert!(hard < 42.0, "{hard}");
}

/// The reference's rules have no row for the mace and refuse it as unknown, on a request they
/// prepare otherwise (the Elemental Shaman's).
#[test]
fn the_reference_rules_still_refuse_the_toofbreaker() {
    let shaman = "elemental-shaman-the-elements";
    assert!(prepare(&wielding(shaman, json!({})), Rules::GoReference).is_ok());
    let request = wielding(shaman, json!({ "id": TOOFBREAKER }));
    match prepare(&request, Rules::GoReference) {
        Err(PrepareError::Refused(refusal)) => {
            let reason = refusal.to_string();
            assert!(reason.contains("no item with id 286430"), "{reason}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}
