//! `sim --request --rotation-stats` adds the character sheet the fight starts with to the
//! player's metrics (`prepare/stat_sheet.rs`), for the application to show beside the result.
//!
//! The sheet is read during preparation and never fed to the simulation: a run that asks for it
//! and one that does not simulate the same fight.

use serde_json::Value;
use std::{fs, path::Path, process::Command};

/// What `--rotation-stats` adds to the player's metrics.
const PRESENTATION: [&str; 6] = [
    "effectiveRotation",
    "rotationStats",
    "rotationPresentationVersion",
    "extraAttackStrikes",
    "castCooldowns",
    "statSheet",
];

/// The fixtures whose talents are in the reference's order, which Forever's rules refuse: they
/// run under the reference's rules, as the rotation goldens do (tests/rotation_stats.rs). The
/// sheet is read the same way under either.
const GO_RULES: [&str; 1] = ["arms-warrior-sweeping-strikes"];

/// The player's metrics of fixture `name`. `test` keeps the scratch folders of tests that run
/// the same fixture at once apart.
fn run(test: &str, name: &str, rotation_stats: bool) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let own = root.join(format!("rotation-stats/{name}.request.json"));
    let request = if own.exists() {
        own
    } else {
        root.join(format!("mage/prepared-v2/{name}.request.json"))
    };
    let mut request: Value = serde_json::from_str(&fs::read_to_string(request).unwrap()).unwrap();
    request["simOptions"] = serde_json::json!({"iterations": 20, "randomSeed": "7"});
    let directory = std::env::temp_dir().join(format!(
        "forever-stat-sheet-{}-{test}-{name}-{rotation_stats}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    fs::write(&input, request.to_string()).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_forever-engine"));
    command
        .args(["sim", "--scenario", "stat-sheet", "--gate", "--request"])
        .arg(&input)
        .arg("--outfile")
        .arg(&output);
    if rotation_stats {
        command.arg("--rotation-stats");
    }
    if GO_RULES.contains(&name) {
        command.arg("--go-rules");
    }
    let done = command.output().unwrap();
    assert!(
        done.status.success(),
        "{name}: {}{}",
        String::from_utf8_lossy(&done.stdout),
        String::from_utf8_lossy(&done.stderr)
    );
    let report: Value = serde_json::from_str(&fs::read_to_string(&output).unwrap()).unwrap();
    fs::remove_dir_all(directory).unwrap();
    report["result"]["raidMetrics"]["parties"][0]["players"][0].clone()
}

fn rows(player: &Value) -> &Vec<Value> {
    player["statSheet"]["rows"]
        .as_array()
        .expect("the sheet's rows")
}

fn row<'a>(player: &'a Value, key: &str) -> Option<&'a Value> {
    rows(player).iter().find(|row| row["key"] == key)
}

fn assert_parts_add_up(name: &str, player: &Value) {
    assert_eq!(
        player["statSheet"]["sources"],
        serde_json::json!(["base", "gear", "bonus", "talents", "buffs", "consumes", "other"])
    );
    for row in rows(player) {
        let parts = row["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 7, "{name} {}", row["key"]);
        let sum: f64 = parts.iter().map(|part| part.as_f64().unwrap()).sum();
        let value = row["value"].as_f64().unwrap();
        assert!(
            (sum - value).abs() < 1e-3,
            "{name} {}: parts {sum} != {value}",
            row["key"]
        );
    }
}

/// A caster's sheet: a mana row, Intellect that is positive and mostly not from talents, a
/// Shadow school total at or above the plain spell damage, and parts that add up.
#[test]
fn a_casters_sheet_adds_up() {
    let name = "shadow-priest-line-checks";
    let player = run("caster", name, true);
    assert_eq!(player["statSheet"]["version"], 1);
    assert_parts_add_up(name, &player);
    let value = |key: &str| row(&player, key).expect(key)["value"].as_f64().unwrap();
    assert!(value("mana") > 0.0 && value("intellect") > 0.0);
    assert!(value("shadowDamage") >= value("spellDamage"));
    assert!(
        row(&player, "energy").is_none(),
        "a priest has no energy bar"
    );
    // Race and class give a level 60 character its first attributes: the base part is never 0.
    assert!(
        row(&player, "stamina").unwrap()["parts"][0]
            .as_f64()
            .unwrap()
            > 0.0
    );
}

/// A warrior's sheet: no mana row, a main hand skill row naming its skill line at 300 or more,
/// and parts that add up.
#[test]
fn a_melee_sheet_names_the_weapon_skill() {
    let name = "arms-warrior-sweeping-strikes";
    let player = run("melee", name, true);
    assert_parts_add_up(name, &player);
    assert!(row(&player, "mana").is_none(), "a warrior has no mana bar");
    let skill = row(&player, "mainHandSkill").expect("a main hand skill row");
    assert!(skill["value"].as_f64().unwrap() >= 300.0);
    assert!(skill["detail"]
        .as_str()
        .is_some_and(|line| !line.is_empty()));
    assert!(
        row(&player, "attackPower").unwrap()["value"]
            .as_f64()
            .unwrap()
            > 0.0
    );
}

/// A hunter's sheet under Forever's rules: mana, a ranged skill row naming its skill line, and
/// ranged attack power of its own.
#[test]
fn a_hunters_sheet_has_the_ranged_rows() {
    let name = "beast-mastery-hunter-nested-groups";
    let player = run("hunter", name, true);
    assert_parts_add_up(name, &player);
    let value = |key: &str| row(&player, key).expect(key)["value"].as_f64().unwrap();
    assert!(value("mana") > 0.0 && value("rangedAttackPower") > 0.0);
    let skill = row(&player, "rangedSkill").expect("a ranged skill row");
    assert!(skill["value"].as_f64().unwrap() >= 300.0);
    assert!(["Bows", "Guns", "Crossbows"].contains(&skill["detail"].as_str().unwrap()));
}

/// The sheet comes only with `--rotation-stats`, and asking for it simulates the same fight.
#[test]
fn the_sheet_is_reported_only_on_request_and_changes_no_result() {
    for name in ["shadow-priest-line-checks", "arms-warrior-sweeping-strikes"] {
        let mut with = run("request", name, true);
        let without = run("request", name, false);
        assert!(with.get("statSheet").is_some(), "{name}: no sheet");
        assert!(without.get("statSheet").is_none(), "{name}: unasked sheet");
        let metrics = with.as_object_mut().unwrap();
        for key in PRESENTATION {
            metrics.remove(key);
        }
        assert_eq!(with, without, "{name}: the sheet changed the simulation");
    }
}
