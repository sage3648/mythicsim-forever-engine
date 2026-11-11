//! Totem of Rage (22395) and Maw of Min'loth (274608), two items that refused players' sims on 10
//! November 2026 ("no item with id"), under Forever's rules.
//!
//! - Totem of Rage, a relic: "Equip: Increases the damage of your Shock spells by 2%." Its equip
//!   spell 27859 is not in the pinned store; the live client's two effects are a 2% damage
//!   modifier on Earth, Flame and Frost Shock and the same on Flame Shock's damage over time
//!   (`classes::shaman::prepare::items::totem_of_rage`).
//! - Maw of Min'loth, a shield: "Equip: When struck in melee combat, has a 10% chance of stealing
//!   40 life from the target enemy. (Proc chance: 10%, 3s cooldown)". Its two client rows are
//!   readable, and `src/data/forever_client_spells.rs` restates them: the equip aura 1295064 and
//!   the Physical leech of 40 it casts, 1295065. The same shape as Skullflame Shield's Drain Life.
//!
//! No refused request was at hand, so the tests put the totem in the relic slot of the Elemental
//! Shaman of `fixtures/forever-gaps/elemental-shaman-the-elements` and the shield in the off hand
//! of the Protection Paladin of `fixtures/forever-gaps/protection-paladin-quel-serrar`, who tanks
//! the target (50 iterations each).

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const TOTEM_OF_RAGE: i64 = 22395;
const MAW_OF_MINLOTH: i64 = 274608;
/// The leech the shield's equip aura casts.
const MAW_LEECH: i64 = 1295065;
/// The top ranks of Earth Shock, Flame Shock and Frost Shock.
const SHOCKS: [i64; 3] = [10414, 29228, 10473];

const OFF_HAND: usize = 15;
const RANGED: usize = 16;

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// A fixture's request with this item in a slot.
fn wearing(fixture: &str, slot: usize, item: Value) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/forever-gaps/{fixture}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    player(&mut request)["equipment"]["items"][slot] = item;
    request
}

/// The Elemental Shaman with this relic.
fn shaman(relic: Value) -> Value {
    wearing("elemental-shaman-the-elements", RANGED, relic)
}

/// The Protection Paladin, who tanks the target, with this off hand.
fn paladin(off_hand: Value) -> Value {
    wearing("protection-paladin-quel-serrar", OFF_HAND, off_hand)
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

fn spells(prepared: &Value) -> &Vec<Value> {
    prepared["player"]["spells"].as_array().unwrap()
}

fn maw_proc(prepared: &Value) -> Option<&Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["kind"] == "spell_data_damage_proc"
                && effect["trigger_aura"] == "Maw of Min'loth"
        })
}

/// The refusal the reference's rules give a request.
fn reference_refusal(request: &Value) -> String {
    match prepare(request, Rules::GoReference) {
        Err(PrepareError::Refused(refusal)) => refusal.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The totem prepares, passes the gate and runs.
#[test]
fn the_totem_of_rage_prepares_and_runs() {
    run(&shaman(json!({ "id": TOTEM_OF_RAGE })));
}

/// Each shock, and Flame Shock's damage over time (the same spell id under a second tag), takes
/// 2% in the additive damage modifier the talents' percentages share, and no other spell of the
/// shaman's changes.
#[test]
fn the_totem_adds_two_percent_to_the_shocks_alone() {
    let without = prepare(&shaman(json!({})), Rules::Forever).unwrap();
    let with = prepare(&shaman(json!({ "id": TOTEM_OF_RAGE })), Rules::Forever).unwrap();
    assert_eq!(spells(&without).len(), spells(&with).len());
    let mut raised = Vec::new();
    for (before, after) in spells(&without).iter().zip(spells(&with)) {
        assert_eq!(before["action_id"], after["action_id"]);
        let id = before["action_id"]["spell_id"].as_i64().unwrap_or(0);
        if !SHOCKS.contains(&id) {
            assert_eq!(before, after, "{}", before["action_id"]);
            continue;
        }
        let added = after["damage_multiplier_additive"].as_f64().unwrap()
            - before["damage_multiplier_additive"].as_f64().unwrap();
        assert!((added - 0.02).abs() < 1e-9, "{id}: {added}");
        assert_eq!(before["damage_multiplier"], after["damage_multiplier"]);
        raised.push(id);
    }
    // Earth Shock, Flame Shock's hit and its ticks, Frost Shock.
    raised.sort_unstable();
    assert_eq!(raised, [10414, 10473, 29228, 29228]);
}

/// In a fight the Flame Shock this shaman casts deals more with the totem, by the share the 2%
/// is of its additive modifier, on the same seed.
#[test]
fn flame_shock_hits_harder_with_the_totem() {
    let (prepared, without) = run(&shaman(json!({})));
    let (_, with) = run(&shaman(json!({ "id": TOTEM_OF_RAGE })));
    let additive = spells(&prepared)
        .iter()
        .find(|spell| spell["action_id"]["spell_id"] == 29228)
        .unwrap()["damage_multiplier_additive"]
        .as_f64()
        .unwrap();
    let before = total(&without, 29228, "damage");
    let after = total(&with, 29228, "damage");
    assert!(before > 0.0);
    let ratio = after / before;
    let expected = (additive + 0.02) / additive;
    assert!(
        (ratio - expected).abs() < 1e-6,
        "{ratio} against {expected}"
    );
}

/// The reference's rules have no row for the totem and refuse it as unknown.
#[test]
fn the_reference_rules_still_refuse_the_totem() {
    let reason = reference_refusal(&shaman(json!({ "id": TOTEM_OF_RAGE })));
    assert!(reason.contains("no item with id 22395"), "{reason}");
}

/// The shield prepares, passes the gate and runs, and its leech deals damage to the target that
/// strikes the tank.
#[test]
fn the_maw_of_minloth_prepares_and_runs() {
    let (prepared, player) = run(&paladin(json!({ "id": MAW_OF_MINLOTH })));
    assert!(maw_proc(&prepared).is_some());
    assert!(total(&player, MAW_LEECH, "damage") > 0.0);
}

/// The row is the tooltip's: 8 Stamina over an empty off hand.
#[test]
fn the_row_carries_the_tooltips_stamina() {
    let with = prepare(&paladin(json!({ "id": MAW_OF_MINLOTH })), Rules::Forever).unwrap();
    let without = prepare(&paladin(json!({})), Rules::Forever).unwrap();
    let added = stat(&with, "Stamina") - stat(&without, "Stamina");
    assert!(added >= 8.0, "{added}");
}

/// The proc is the client rows': a melee hit the wearer takes casts it 10% of the time, at most
/// once in 3 seconds, and it is a Physical hit of 40 on the melee table with no variance and no
/// attack power or spell power share.
#[test]
fn the_proc_is_a_physical_leech_of_forty_when_struck() {
    let prepared = prepare(&paladin(json!({ "id": MAW_OF_MINLOTH })), Rules::Forever).unwrap();
    let proc = maw_proc(&prepared).unwrap();
    assert_eq!(proc["struck"], true);
    assert_eq!(proc["proc_chance"], 0.1);
    assert_eq!(proc["average"], 40.0);
    assert_eq!(proc["variance"], 0.0);
    assert!(proc.get("periodic").is_none());
    let spell = &spells(&prepared)[proc["spell"].as_u64().unwrap() as usize];
    assert_eq!(spell["action_id"]["spell_id"], MAW_LEECH);
    assert_eq!(spell["school"], 1);
    assert_eq!(spell["defense_type"], "DefenseTypeMelee");
    assert_eq!(spell["bonus_coefficient"], 0.0);
    let aura = prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == "Maw of Min'loth")
        .expect("the equip aura");
    assert_eq!(aura["icd"]["duration_ns"], 3_000_000_000_i64);
}

/// A character who does not tank the target is never struck, so the shield's leech never lands.
#[test]
fn a_character_who_is_not_struck_never_leeches() {
    let mut request = paladin(json!({ "id": MAW_OF_MINLOTH }));
    request["raid"].as_object_mut().unwrap().remove("tanks");
    let (_, player) = run(&request);
    assert_eq!(total(&player, MAW_LEECH, "damage"), 0.0);
}

/// The reference's rules have no row for the shield and refuse it as unknown.
#[test]
fn the_reference_rules_still_refuse_the_shield() {
    let reason = reference_refusal(&paladin(json!({ "id": MAW_OF_MINLOTH })));
    assert!(reason.contains("no item with id 274608"), "{reason}");
}
