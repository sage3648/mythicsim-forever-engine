//! MythicSim's healer sims for the Priest, the Restoration Druid and the Restoration Shaman,
//! which Rust alone runs: the Go engine has none of their heals, so there is no Go golden (the
//! Holy Paladin has Go goldens, in the fixture family). Each request in `fixtures/healers`
//! prepares in Rust, passes the gate and heals.
//!
//! Under Forever's rules a healer sim counts only the share of each heal real healers do not
//! overheal (`prepare::overheal`), so each heal line logs what heals and the overhealing beside
//! it; their sum is the heal the client rows give.
//!
//! Two kinds of check:
//!
//! - The first fight's log lines below pin one heal of each kind to the amount its client rows
//!   give, worked out by hand in the comments: the rank's amount at level 60, its coefficient on
//!   the healing power the line states, and the talents' client modifiers at their ranks.
//! - `<name>.rust-golden.json` keeps the whole fight's healing by spell, as Rust made it, so a
//!   change that moves it shows. It is not evidence that the numbers are right; the log lines
//!   are. Regenerate it with `UPDATE_HEALER_GOLDENS=1 cargo test --test healers` after a change
//!   that should move it, and say why in the commit.

use forever_engine::{check_prepared, prepare, prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Map, Value};
use std::{fs, path::Path};

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/healers"))
}

/// The fight's result for a healer request: prepared in Rust, gated and simulated.
fn run(name: &str) -> Value {
    let request = fs::read(fixtures().join(format!("{name}.request.json"))).unwrap();
    let prepared = prepare(&request, "rust").unwrap_or_else(|err| panic!("{name}: {err:?}"));
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{name}: {err:?}"));
    simulate_prepared(&prepared)
        .unwrap_or_else(|err| panic!("{name}: {err:?}"))
        .result
}

fn player(result: &Value) -> &Value {
    &result["raidMetrics"]["parties"][0]["players"][0]
}

/// The player's healing per second and, by action, what each did in every fight together.
fn summary(result: &Value) -> Value {
    let player = player(result);
    let mut actions = Map::new();
    for action in player["actions"].as_array().unwrap() {
        let sum = |key: &str| -> f64 {
            action["targets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|target| target[key].as_f64().unwrap_or(0.0))
                .sum()
        };
        let healing = sum("healing") + sum("shielding");
        if healing == 0.0 {
            continue;
        }
        actions.insert(
            action["id"].to_string(),
            json!({
                "casts": sum("casts"), "hits": sum("hits"), "crits": sum("crits"),
                "ticks": sum("ticks"), "healing": sum("healing"), "shielding": sum("shielding"),
                "overhealing": sum("overhealing"),
            }),
        );
    }
    json!({"hps": player["hps"]["avg"], "seconds_oom": player["secondsOomAvg"], "actions": actions})
}

/// The summary equals its golden, or the golden is rewritten when asked.
fn golden(name: &str, result: &Value) {
    let path = fixtures().join(format!("{name}.rust-golden.json"));
    let actual = summary(result);
    if std::env::var_os("UPDATE_HEALER_GOLDENS").is_some() {
        fs::write(&path, serde_json::to_string_pretty(&actual).unwrap() + "\n").unwrap();
        return;
    }
    let expected: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        actual, expected,
        "{name}: the healing moved from its golden"
    );
}

/// The first fight's log has every one of these lines.
fn logs(name: &str, result: &Value, lines: &[&str]) {
    let log = result["logs"].as_str().unwrap();
    for line in lines {
        assert!(
            log.contains(line),
            "{name}: the first fight never logs {line:?}"
        );
    }
}

/// The 38/13 Holy priest: Spiritual Healing 3 (10% direct and periodic), Improved Renew 3
/// (15%), Twin Disciplines 5 (5% periodic on Renew), Improved Power Word: Shield 3 (20%), on
/// 268.2 healing power.
#[test]
fn holy_priest_heals_as_its_rows_state() {
    let result = run("holy-priest");
    logs(
        "holy-priest",
        &result,
        &[
            // Prayer of Mending rank 1 (401859): 172 + 0.429 x 268.2 = 287.06, then +10%, on
            // each of its five charges.
            "{SpellID: 401859} [DEBUG] HealingPower: 268.2, BaseHealing:287.1, AfterCasterMods:315.8",
            // Power Word: Shield rank 10 (10901): 928 + 0.1 x 268.2 = 954.82, then +20%.
            "{SpellID: 10901} Shield for 1145.790.",
            // Renew rank 10 (25315): 166 + 0.2 x 268.2 = 219.64 a tick, then 1.10 x 1.15 x 1.05,
            // 291.750, of which Renew's 16.06% is overhealing.
            "{SpellID: 25315} tick Hit for 244.895 healing, 46.855 overhealing.",
        ],
    );
    let hits = player(&result)["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"] == 401859)
        .map(|action| {
            action["targets"][1]["hits"].as_f64().unwrap_or(0.0)
                + action["targets"][1]["crits"].as_f64().unwrap_or(0.0)
        })
        .unwrap();
    let casts: f64 = player(&result)["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"] == 401859)
        .map(|action| {
            action["targets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["casts"].as_f64().unwrap_or(0.0))
                .sum()
        })
        .unwrap();
    assert_eq!(
        hits,
        casts * 5.0,
        "each Prayer of Mending heals five players"
    );
    golden("holy-priest", &result);
}

/// The 46-point Restoration druid: Gift of Nature 5 (10% direct and periodic), Improved
/// Rejuvenation 3 (15%), Genesis 5 (5% periodic), Nature's Splendor, on 302 healing power.
#[test]
fn restoration_druid_heals_as_its_rows_state() {
    let result = run("restoration-druid");
    logs(
        "restoration-druid",
        &result,
        &[
            // Wild Growth rank 3 (1238215): 97 + 0.033 x 302 = 106.97 a tick, then +10%, on
            // each of five players, 117.663, of which a group heal's 24.58% is overhealing.
            "{SpellID: 1238215} tick Hit for 88.741 healing, 28.921 overhealing.",
            // Rejuvenation rank 11 (25299): 194 + 0.2 x 302 = 254.4 a tick, then 1.10 x 1.15 x
            // 1.05, 337.907, of which Rejuvenation's 17.76% is overhealing.
            "{SpellID: 25299} tick Hit for 277.895 healing, 60.012 overhealing.",
            // Swiftmend consumes that Rejuvenation: its five ticks with Nature's Splendor, 1689.5,
            // then Gift of Nature's 10% on Swiftmend's own direct heal.
            "{SpellID: 18562} [DEBUG] HealingPower: 302.0, BaseHealing:1689.5, AfterCasterMods:1858.5",
        ],
    );
    golden("restoration-druid", &result);
}

/// The 40-point Restoration shaman: Purification 5 (10% direct and periodic), Healing Way 3
/// (25% on Healing Wave), Restorative Totems 5 (25% on Healing Stream's ticks).
#[test]
fn restoration_shaman_heals_as_its_rows_state() {
    let result = run("restoration-shaman");
    let log = result["logs"].as_str().unwrap();
    // Chain Heal rank 3 (10623) lands on three players, each with its own roll of 473 to 539
    // plus 0.714 of healing power, the second at half and the third at a quarter.
    let field = |line: &str, name: &str| -> f64 {
        let at = line.find(name).unwrap() + name.len();
        line[at..].split(',').next().unwrap().parse().unwrap()
    };
    let chain: Vec<(f64, f64)> = log
        .lines()
        .filter(|line| line.contains("{SpellID: 10623} [DEBUG]"))
        .take(3)
        .map(|line| (field(line, "HealingPower: "), field(line, "BaseHealing:")))
        .collect();
    assert_eq!(chain.len(), 3);
    for (jump, (healing_power, base)) in chain.into_iter().enumerate() {
        let roll = base / 0.5f64.powi(jump as i32) - 0.714 * healing_power;
        assert!((472.5..=539.5).contains(&roll), "jump {jump}: roll {roll}");
    }
    // Riptide recast every 6 seconds keeps its 15 second heal over time on each player: about
    // five ticks a cast, not the two or three a cut short one would give.
    let riptide = player(&result)["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"] == 1239243)
        .unwrap();
    let sum = |key: &str| -> f64 {
        riptide["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t[key].as_f64().unwrap_or(0.0))
            .sum()
    };
    assert!(
        sum("ticks") > 4.5 * sum("casts"),
        "{} ticks for {} casts",
        sum("ticks"),
        sum("casts")
    );
    golden("restoration-shaman", &result);
}

/// Under Forever's rules a healer sim names the overhealing share of each of its heals, and only
/// what is left counts: the Holy Paladin's Flash of Light, Holy Light and Holy Shock heals lose
/// their measured shares, so its healing per second falls by each heal's share of its healing.
/// The Go reference's rules, which the Holy Paladin's Go goldens use, and a damage spec name none.
#[test]
fn forever_healer_sims_count_healing_less_overhealing() {
    let family = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/mage/prepared-v2"
    ));
    let request = |name: &str| fs::read(family.join(format!("{name}.request.json"))).unwrap();
    let holy = request("holy-paladin-healing");
    let forever = prepare_json_with(&holy, "rust", Rules::Forever).unwrap();
    let reference = prepare_json_with(&holy, "rust", Rules::GoReference).unwrap();
    let share = |spell: i64| {
        forever["player"]["overheal"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["spell_id"] == spell)
            .map(|entry| entry["fraction"].as_f64().unwrap())
    };
    // Flash of Light rank 6, Holy Light rank 9 and the Go reference's Holy Shock heal.
    assert_eq!(share(19943), Some(0.0699));
    assert_eq!(share(25292), Some(0.1024));
    assert_eq!(share(25903), Some(0.0658));
    // Holy Shock's damage is no heal.
    assert_eq!(share(1311606), None);
    assert!(reference["player"].get("overheal").is_none());
    let shadow = request("shadow-priest-holy-nova");
    let shadow = prepare_json_with(&shadow, "rust", Rules::Forever).unwrap();
    assert!(shadow["player"].get("overheal").is_none());

    // The heals draw the same rolls either way, so each heal's healing and overhealing add up
    // to what the reference heals, split at its share.
    let sim = |prepared: &Value| {
        let prepared = serde_json::from_value(prepared.clone()).unwrap();
        simulate_prepared(&prepared).unwrap().result
    };
    let (forever, reference) = (sim(&forever), sim(&reference));
    let totals = |result: &Value, spell: i64, key: &str| -> f64 {
        player(result)["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|action| action["id"]["spellId"] == spell)
            .flat_map(|action| action["targets"].as_array().unwrap())
            .map(|target| target[key].as_f64().unwrap_or(0.0))
            .sum()
    };
    for (spell, fraction) in [(19943, 0.0699), (25292, 0.1024), (25903, 0.0658)] {
        let whole = totals(&reference, spell, "healing");
        let healing = totals(&forever, spell, "healing");
        let overhealing = totals(&forever, spell, "overhealing");
        assert!(whole > 0.0, "{spell} heals");
        assert!(
            ((healing + overhealing) / whole - 1.0).abs() < 1e-9,
            "{spell}"
        );
        assert!((overhealing / whole - fraction).abs() < 1e-9, "{spell}");
    }
    let hps = |result: &Value| player(result)["hps"]["avg"].as_f64().unwrap();
    assert!(hps(&forever) < hps(&reference));
}
