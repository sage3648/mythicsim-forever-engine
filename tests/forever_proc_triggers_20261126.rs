//! Under `--rotation-stats` the player's metrics list each proc's results by the ability whose
//! hit triggered it (`procTriggers`), so the application's ability table can break Windfury
//! Totem's strikes and Flametongue's hits down by the ability that fired them (MythicSim Discord
//! #paladin, 13 November 2026, WizardXeze). The list only reads what the fight did: every
//! Windfury Totem strike and Flametongue hit is fired by a hit, so their triggers add up to the
//! proc's own row, and nothing simulated changes.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const PLAYER: &str = "/raidMetrics/parties/0/players/0";

fn request(totem: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/proc-triggers/retribution-paladin.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let buffs = request["raid"]["parties"][0]["buffs"]
        .as_object_mut()
        .unwrap();
    buffs.remove("windfuryTotem");
    buffs.insert(totem.into(), json!(true));
    request
}

/// The result of the fixture's 300 fights (the Retribution board's request, built by the
/// application on 26 November 2026), before and after the adapter's additions.
fn run(totem: &str) -> (Value, Value) {
    let mut prepared = prepare_json_with(
        request(totem).to_string().as_bytes(),
        "rust",
        Rules::Forever,
    )
    .unwrap();
    prepared["sim"]["iterations"] = json!(300);
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let mut report = simulate_prepared(&prepared).unwrap();
    let plain = report.result.clone();
    assert!(report.add_rotation_stats());
    (plain, report.result)
}

fn action<'a>(result: &'a Value, id: &Value) -> &'a Value {
    result
        .pointer(&format!("{PLAYER}/actions"))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|action| &action["id"] == id)
        .unwrap_or_else(|| panic!("no action {id}"))
}

/// The damage an action dealt to the boss.
fn damage(action: &Value) -> f64 {
    action["targets"][0]["damage"].as_f64().unwrap_or(0.0)
}

/// Each proc's entries, by its action ID.
fn procs(result: &Value) -> Vec<(Value, Vec<Value>)> {
    let mut out: Vec<(Value, Vec<Value>)> = Vec::new();
    for entry in result
        .pointer(&format!("{PLAYER}/procTriggers"))
        .unwrap()
        .as_array()
        .unwrap()
    {
        match out.iter_mut().find(|(id, _)| *id == entry["id"]) {
            Some((_, entries)) => entries.push(entry.clone()),
            None => out.push((entry["id"].clone(), vec![entry.clone()])),
        }
    }
    out
}

fn is_white_swing(id: &Value) -> bool {
    id["otherId"] == "OtherActionAttack" && id["tag"] == 1
}

/// The proc whose triggers deal all of its damage, with those triggers.
fn whole_proc(result: &Value, is_proc: impl Fn(&Value) -> bool) -> (Value, Vec<Value>) {
    let (id, triggers) = procs(result)
        .into_iter()
        .find(|(id, _)| is_proc(id))
        .expect("the proc is listed");
    let total: f64 = triggers.iter().map(|t| t["damage"].as_f64().unwrap()).sum();
    let row = damage(action(result, &id));
    assert!(row > 0.0, "{id} dealt nothing");
    assert!(
        (total - row).abs() <= row * 1e-9,
        "{id}: its triggers deal {total}, its row {row}"
    );
    (id, triggers)
}

#[test]
fn windfury_totem_strikes_add_up_by_their_triggers() {
    let (plain, result) = run("windfuryTotem");
    assert!(plain.pointer(&format!("{PLAYER}/procTriggers")).is_none());
    // The totem's extra attack is an attack with the totem's spell as its tag.
    let (_, triggers) = whole_proc(&result, |id| {
        id["otherId"] == "OtherActionAttack" && id["tag"].as_i64().unwrap_or(0) > 2
    });
    assert!(triggers.iter().any(|t| is_white_swing(&t["trigger"])));
    assert!(
        triggers
            .iter()
            .any(|t| t["trigger"].get("spellId").is_some()),
        "no ability triggered a strike: {triggers:?}"
    );
    assert!(triggers.iter().all(|t| t["results"].as_u64().unwrap() > 0));
    // The additions leave the simulated result as it was.
    let mut stripped = result.clone();
    for key in ["procTriggers", "extraAttackStrikes", "castCooldowns"] {
        stripped
            .pointer_mut(PLAYER)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
    }
    assert_eq!(
        stripped.pointer(&format!("{PLAYER}/dps")),
        plain.pointer(&format!("{PLAYER}/dps"))
    );
    assert_eq!(
        stripped.pointer(&format!("{PLAYER}/actions")),
        plain.pointer(&format!("{PLAYER}/actions"))
    );
}

#[test]
fn flametongue_totem_hits_add_up_by_their_triggers() {
    let (_, result) = run("flametongueTotem");
    let (_, windfury) = run("windfuryTotem");
    // The totem's hit is the proc the build has only under Flametongue Totem.
    let others: Vec<Value> = procs(&windfury).into_iter().map(|(id, _)| id).collect();
    let (_, triggers) = whole_proc(&result, |id| !others.contains(id));
    // The party totem's imbue hears white swings only, so they fire every hit.
    assert!(triggers.iter().all(|t| is_white_swing(&t["trigger"])));
}
