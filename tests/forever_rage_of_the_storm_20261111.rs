//! Rage of the Storm (280604), the Enhancement Shaman's two-hand mace, after the hotfix of the 10
//! November 2026 weekly reset, under Forever's rules.
//!
//! Hotfix push 113886 (client 1.60.1.70888, wago.tools hotfixes, 10 November 2026) rewrites its
//! ItemSparse row: ItemDelay 3600 (was 3200) and stats Strength at 6956 and Stamina at 6521 of
//! 10000 (was Intellect at 5217 and Stamina at 6521). At the row's 23 points that is +16 Strength
//! and +15 Stamina, and at the unchanged item level 40 (ItemDamageTwoHand 35.7078 damage per
//! second for a rare, damage variance 0.4) the slower swing deals 102 to 154 (was 91 to 137).
//! `data/forever-live-items.jsonl` holds the row; the reference's rules keep the pinned one.
//!
//! Its equip effect is unchanged and was not modelled: "Equip: Increases the damage dealt by your
//! Stormstrike ability by 10%." (Rage of Earth 1309422, SpellEffect 1338404: a percent damage
//! modifier of 10 on Stormstrike's class mask). A Shaman's two Stormstrike hits now take 0.10 in
//! the additive damage modifier (`classes::shaman::prepare::items::rage_of_the_storm`).
//!
//! The tests put the mace in the main hand of the level 40 Enhancement Shaman of
//! `fixtures/forever-gaps/enhancement-shaman-ceremonial-mallet`, who holds a two-hand mace (50
//! iterations).

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const RAGE_OF_THE_STORM: i64 = 280604;
/// The two-hand mace the fixture holds, which has no Stormstrike effect.
const CEREMONIAL_MALLET: i64 = 279885;
const STORMSTRIKE: i64 = 17364;

const MAIN_HAND: usize = 14;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

fn fixture(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/{path}.request.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The Enhancement Shaman with this main hand.
fn shaman(main_hand: Value) -> Value {
    let mut request = fixture("forever-gaps/enhancement-shaman-ceremonial-mallet");
    player(&mut request)["equipment"]["items"][MAIN_HAND] = main_hand;
    request
}

/// The Enhancement Shaman with an item swap between these two main hands, which the rotation
/// never performs.
fn shaman_with_swap(main_hand: i64, swap: i64) -> Value {
    let mut request = shaman(json!({ "id": main_hand }));
    let player = player(&mut request);
    let mut set = vec![json!({}); 17];
    set[MAIN_HAND] = json!({ "id": swap });
    player["enableItemSwap"] = json!(true);
    player["itemSwap"] = json!({ "items": set });
    request
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

/// The spellbook positions and rows of Stormstrike's two weapon hits (tags 1 and 2).
fn stormstrike_hits(prepared: &Value) -> Vec<(usize, &Value)> {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, spell)| {
            spell["action_id"]["spell_id"] == STORMSTRIKE
                && spell["action_id"]["tag"].as_i64().unwrap_or(0) > 0
        })
        .collect()
}

fn additive(spell: &Value) -> f64 {
    spell["damage_multiplier_additive"].as_f64().unwrap()
}

/// The committed live row is the hotfixed one: the reference row with the new speed, damage and
/// stats, and its name, kind, points and effect kept.
#[test]
fn the_live_row_is_the_hotfixed_row() {
    let row = |file: &str| -> Value {
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(file))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .find(|row| row["id"] == RAGE_OF_THE_STORM)
            .unwrap()
    };
    let (reference, live) = (
        row("data/items.jsonl"),
        row("data/forever-live-items.jsonl"),
    );
    assert_eq!(reference["weapon_speed"], 3.2);
    assert_eq!(
        reference["scaling_options"]["0"],
        json!({"rand_prop_points": 23, "weapon_damage_min": 91, "weapon_damage_max": 137,
               "stats": {"2": 15, "3": 12}, "ilvl": 40})
    );
    assert_eq!(live["weapon_speed"], 3.6);
    assert_eq!(
        live["scaling_options"]["0"],
        json!({"rand_prop_points": 23, "weapon_damage_min": 102, "weapon_damage_max": 154,
               "stats": {"0": 16, "2": 15}, "ilvl": 40})
    );
    let mut same = live.clone();
    same["weapon_speed"] = reference["weapon_speed"].clone();
    same["scaling_options"] = reference["scaling_options"].clone();
    assert_eq!(same, reference);
}

/// The hotfixed numbers follow from the client rows: 16 Strength and 15 Stamina are the row's
/// shares of its 23 points, and 102 to 154 is the item level's damage per second at the new
/// speed with the row's variance, the low end cut and the high end rounded as the client shows a
/// weapon (the old row's 91 to 137 at 3.20 comes out of the same sum).
#[test]
fn the_hotfixed_numbers_follow_from_the_client_rows() {
    let points = 23.0_f64;
    assert_eq!((6956.0 * points / 10000.0).round(), 16.0);
    assert_eq!((6521.0 * points / 10000.0).round(), 15.0);
    assert_eq!((5217.0 * points / 10000.0).round(), 12.0);
    // ItemDamageTwoHand, item level 40, quality 3.
    let per_second = 35.70782470703_f64;
    let range = |speed: f64| {
        let average = per_second * speed;
        ((average * 0.8).floor(), (average * 1.2 + 0.5).floor())
    };
    assert_eq!(range(3.2), (91.0, 137.0));
    assert_eq!(range(3.6), (102.0, 154.0));
}

/// The mace prepares, passes the gate and runs with the hotfixed weapon: 102 to 154 at 3.60, and
/// over an empty main hand 16 Strength and 15 Stamina and no Intellect.
#[test]
fn the_shaman_swings_the_hotfixed_mace() {
    // Without the fixture's consumables, whose stone adds weapon damage.
    let bare = |main_hand: Value| {
        let mut request = shaman(main_hand);
        player(&mut request)["consumables"] = json!({});
        prepared(&request)
    };
    let with = bare(json!({ "id": RAGE_OF_THE_STORM }));
    let without = bare(json!({}));
    let main_hand = &with["melee"]["main_hand"];
    assert_eq!(main_hand["base_damage_min"], 102.0);
    assert_eq!(main_hand["base_damage_max"], 154.0);
    assert_eq!(main_hand["swing_speed"], 3.6);
    assert!(stat(&with, "Strength") - stat(&without, "Strength") >= 16.0);
    assert!(stat(&with, "Stamina") - stat(&without, "Stamina") >= 15.0);
    assert_eq!(stat(&with, "Intellect"), stat(&without, "Intellect"));
    simulate(&prepared(&shaman(json!({ "id": RAGE_OF_THE_STORM }))));
}

/// The reference's rules read the pinned row: 91 to 137 at 3.20.
#[test]
fn the_reference_rules_keep_the_pinned_row() {
    let mut request = fixture("forever-gaps/elemental-shaman-the-elements");
    player(&mut request)["equipment"]["items"][MAIN_HAND] = json!({ "id": RAGE_OF_THE_STORM });
    player(&mut request)["consumables"] = json!({});
    let prepared = prepare(&request, Rules::GoReference).unwrap_or_else(|err| panic!("{err}"));
    let main_hand = &prepared["melee"]["main_hand"];
    assert_eq!(main_hand["base_damage_min"], 91.0);
    assert_eq!(main_hand["base_damage_max"], 137.0);
    assert_eq!(main_hand["swing_speed"], 3.2);
}

/// "Increases the damage dealt by your Stormstrike ability by 10%": each of Stormstrike's two
/// hits takes 0.10 in the additive damage modifier beside the same Shaman with another two-hand
/// mace, and its own multiplier (150% weapon damage) is as it was.
#[test]
fn stormstrike_takes_ten_percent_with_the_mace() {
    let with = prepared(&shaman(json!({ "id": RAGE_OF_THE_STORM })));
    let other = prepared(&shaman(json!({ "id": CEREMONIAL_MALLET })));
    let (with, other) = (stormstrike_hits(&with), stormstrike_hits(&other));
    assert_eq!(with.len(), 2);
    assert_eq!(other.len(), 2);
    for ((_, with), (_, other)) in with.iter().zip(&other) {
        assert_eq!(with["action_id"], other["action_id"]);
        let added = additive(with) - additive(other);
        assert!((added - 0.10).abs() < 1e-9, "{added}");
        assert_eq!(with["damage_multiplier"], other["damage_multiplier"]);
    }
}

/// In the fight the modifier is all of the effect: the same prepared fight with the 0.10 taken
/// back lands the same Stormstrikes, roll for roll, and the mace's deal the modifier's share
/// more. The raid's debuffs are taken off, since a flat amount added to each physical hit after
/// the multiplier (Gift of Arthas) would dilute the share.
#[test]
fn stormstrike_hits_harder_by_the_modifiers_share() {
    let mut request = shaman(json!({ "id": RAGE_OF_THE_STORM }));
    request["raid"]["debuffs"] = json!({});
    let with = prepared(&request);
    let mut without = with.clone();
    let mut share = 0.0;
    for (position, spell) in stormstrike_hits(&with) {
        share = additive(spell) / (additive(spell) - 0.10);
        without["player"]["spells"][position]["damage_multiplier_additive"] =
            json!(additive(spell) - 0.10);
    }
    let (with, without) = (simulate(&with), simulate(&without));
    let hits = total(&without, STORMSTRIKE, "hits") + total(&without, STORMSTRIKE, "crits");
    assert!(hits > 0.0);
    assert_eq!(
        total(&with, STORMSTRIKE, "hits") + total(&with, STORMSTRIKE, "crits"),
        hits
    );
    let ratio = total(&with, STORMSTRIKE, "damage") / total(&without, STORMSTRIKE, "damage");
    assert!((ratio - share).abs() < 1e-9, "{ratio} against {share}");
}

/// The modifier is a fixed one, which an item swap that takes the mace off or puts it on cannot
/// follow. A request whose swap moves the mace still prepares, passes the gate and runs, without
/// the effect, as it did before the effect was modelled.
#[test]
fn a_swap_that_moves_the_mace_prepares_without_the_effect() {
    let plain = prepared(&shaman(json!({ "id": CEREMONIAL_MALLET })));
    let base = additive(stormstrike_hits(&plain)[0].1);
    for (main_hand, swap) in [
        (RAGE_OF_THE_STORM, CEREMONIAL_MALLET),
        (CEREMONIAL_MALLET, RAGE_OF_THE_STORM),
    ] {
        let prepared = prepared(&shaman_with_swap(main_hand, swap));
        for (_, spell) in stormstrike_hits(&prepared) {
            assert_eq!(additive(spell), base, "{main_hand} then {swap}");
        }
        simulate(&prepared);
    }
}

/// The mace has no class restriction. A character who is not a Shaman holds it as a plain weapon:
/// the request prepares, passes the gate and runs.
#[test]
fn another_class_holds_the_mace_as_a_plain_weapon() {
    // The level 30 Retribution Paladin's two-hand sword, Blade of Hanna.
    const PALADIN_WEAPON: i64 = 2801;
    let mut request = fixture("levels/retribution-paladin-L30");
    let items = player(&mut request)["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    let held = items
        .iter_mut()
        .find(|item| item["id"] == PALADIN_WEAPON)
        .expect("the paladin's weapon");
    *held = json!({ "id": RAGE_OF_THE_STORM });
    let prepared = prepared(&request);
    assert_eq!(prepared["melee"]["main_hand"]["swing_speed"], 3.6);
    simulate(&prepared);
}
