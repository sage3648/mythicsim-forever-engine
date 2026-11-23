//! Shaman Flurry under Forever's rules: a Windfury Weapon strike spends a charge as a white
//! swing does, on the same 500 ms charge cooldown, except in the instant another Windfury
//! Weapon strike's crit gained the charges.
//!
//! Combat logs of 23 November 2026, 36 level 60 Enhancement shamans in 35 reports: a Stormstrike
//! that does not crit, with charges up and the cooldown ready, spends a charge 160 of 173 times
//! when Windfury Weapon strikes follow it, 45 of 1,507 times when no hit does and 7 of 171 times
//! when only Windfury Totem strikes do; a white or Stormstrike crit followed by Windfury Weapon
//! strikes alone ends on two charges 294 of 367 times, against three 2,931 of 3,052 times when no
//! hit follows; a second Windfury Weapon strike in the same millisecond as one whose crit gained
//! the charges leaves three 293 of 298 times. The Go reference keeps its own rules, where the
//! strike spends none.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// Windfury Weapon's main hand strike and Flurry's aura.
const STRIKE: i64 = 439440;
const FLURRY: i64 = 16257;
/// The live Enhancement reference's talents on the launch tree, which only Forever's rules take.
const ENHANCEMENT: &str = "053303102-053030310051002251-03";

/// The production Enhancement request with Windfury Weapon on its two-hand axe in place of
/// Rockbiter Weapon and no party Windfury Totem, as the Enhancement board runs it.
fn request(rules: Rules, iterations: u32, log: bool) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-enhancement-shaman.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(iterations);
    request["simOptions"]["debugFirstIteration"] = json!(log);
    request["encounter"]["duration"] = json!(600.0);
    request["raid"]["parties"][0]["buffs"]["windfuryTotem"] = json!(false);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["enhancementShaman"]["options"]["classOptions"]["imbueMh"] = json!("WindfuryWeapon");
    if rules == Rules::Forever {
        player["talentsString"] = json!(ENHANCEMENT);
    }
    request
}

fn prepare(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn run(prepared: &Value) -> Value {
    let prepared = serde_json::from_value(prepared.clone()).unwrap();
    simulate_prepared(&prepared).unwrap().result
}

fn flurry_mut(prepared: &mut Value) -> &mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "flurry")
        .expect("the flurry effect")
}

/// Flurry's uptime in seconds an iteration.
fn flurry_uptime(result: &Value) -> f64 {
    result["raidMetrics"]["parties"][0]["players"][0]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["id"]["spellId"].as_i64() == Some(FLURRY))
        .and_then(|aura| aura["uptimeSecondsAvg"].as_f64())
        .expect("Flurry")
}

#[derive(Clone, Copy, Debug)]
struct Attack {
    time: f64,
    crit: bool,
}

/// The first iteration's white swings, Windfury Weapon strikes, the moments a charge was spent
/// and the moments Flurry was gained or refreshed.
struct Logged {
    swings: Vec<Attack>,
    strikes: Vec<Attack>,
    spent: Vec<f64>,
    started: Vec<f64>,
    /// Every change of Flurry's charges: when, and how many are left.
    charges: Vec<(f64, i32)>,
}

fn time_of(line: &str) -> Option<f64> {
    line.strip_prefix('[')
        .and_then(|rest| rest.split_once(']'))
        .and_then(|(time, _)| time.parse::<f64>().ok())
}

fn logged(result: &Value) -> Logged {
    let log = result["logs"].as_str().expect("the first iteration's log");
    let mut logged = Logged {
        swings: Vec::new(),
        strikes: Vec::new(),
        spent: Vec::new(),
        started: Vec::new(),
        charges: Vec::new(),
    };
    let strike = format!("{{SpellID: {STRIKE}}}");
    let charges = format!("{{SpellID: {FLURRY}}} stacks: ");
    let gained = format!("Aura gained: {{SpellID: {FLURRY}}}");
    let refreshed = format!("Aura refreshed: {{SpellID: {FLURRY}}}");
    let faded = format!("Aura faded: {{SpellID: {FLURRY}}}");
    let mut down = true;
    for line in log.lines() {
        let Some(time) = time_of(line) else {
            continue;
        };
        if line.contains(&gained) || line.contains(&refreshed) {
            down = false;
            logged.started.push(time);
            continue;
        }
        if line.contains(&faded) {
            down = true;
            continue;
        }
        if let Some((_, change)) = line.split_once(&charges) {
            let (from, to) = change.split_once(" --> ").expect("a stack change");
            let (from, to) = (
                from.trim().parse::<i32>().unwrap(),
                to.trim().parse::<i32>().unwrap(),
            );
            if !down && to < from {
                logged.spent.push(time);
            }
            logged.charges.push((time, to));
            continue;
        }
        if !line.contains("(Threat: ") || line.contains("[DEBUG]") {
            continue;
        }
        let attack = Attack {
            time,
            crit: line.contains("} Crit for "),
        };
        if line.contains("{OtherID: 3, Tag: 1}") || line.contains("{OtherID: 3, Tag: 2}") {
            logged.swings.push(attack);
        } else if line.contains(&strike) {
            logged.strikes.push(attack);
        }
    }
    logged
}

/// Flurry's handler runs one 10 ms batch window after the hit, and the log keeps two decimals.
fn same_batch(hit: f64, handled: f64) -> bool {
    (-0.001..=0.021).contains(&(handled - hit))
}

fn same_moment(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.011
}

/// The charges spent that follow a Windfury Weapon strike that did not crit and no white swing.
fn spent_by_strikes(logged: &Logged) -> usize {
    logged
        .spent
        .iter()
        .filter(|&&spent| {
            logged
                .strikes
                .iter()
                .any(|strike| !strike.crit && same_batch(strike.time, spent))
                && !logged
                    .swings
                    .iter()
                    .any(|swing| same_batch(swing.time, spent))
        })
        .count()
}

/// The prepared `flurry` effect carries the rule under Forever's rules only.
#[test]
fn flurry_carries_the_rule_under_forevers_rules_only() {
    let forever = prepare(&request(Rules::Forever, 10, false), Rules::Forever);
    let flurry = |prepared: &Value| {
        prepared["effects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|effect| effect["kind"] == "flurry")
            .cloned()
            .expect("the flurry effect")
    };
    assert_eq!(flurry(&forever)["imbue_strike_spends"], json!(true));
    let reference = prepare(&request(Rules::GoReference, 10, false), Rules::GoReference);
    assert!(flurry(&reference).get("imbue_strike_spends").is_none());
}

/// Under the rule a Windfury Weapon strike spends charges where no white swing does; without
/// it every charge spent follows a white swing.
#[test]
fn a_windfury_weapon_strike_spends_a_flurry_charge() {
    let prepared = prepare(&request(Rules::Forever, 1, true), Rules::Forever);
    let with_rule = logged(&run(&prepared));
    assert!(
        spent_by_strikes(&with_rule) > 0,
        "no charge spent by a strike in {} strikes and {} spends",
        with_rule.strikes.len(),
        with_rule.spent.len()
    );

    let mut without = prepared.clone();
    flurry_mut(&mut without)["imbue_strike_spends"] = json!(false);
    let without = logged(&run(&without));
    assert!(!without.spent.is_empty(), "no Flurry charge was spent");
    for &spent in &without.spent {
        assert!(
            without
                .swings
                .iter()
                .any(|swing| !swing.crit && same_batch(swing.time, spent)),
            "without the rule the charge spent at {spent} follows no white swing"
        );
    }
}

/// A strike in the same moment as another strike whose crit started or refreshed Flurry
/// leaves every charge: once the moment is handled Flurry holds three.
#[test]
fn a_strike_beside_a_strike_crit_that_started_flurry_spends_none() {
    let prepared = prepare(&request(Rules::Forever, 1, true), Rules::Forever);
    let logged = logged(&run(&prepared));
    let mut checked = 0;
    for crit in logged.strikes.iter().filter(|strike| strike.crit) {
        let started = logged
            .started
            .iter()
            .any(|&start| same_batch(crit.time, start));
        let partners = logged
            .strikes
            .iter()
            .filter(|strike| !strike.crit && same_moment(strike.time, crit.time))
            .count();
        let swing_beside = logged
            .swings
            .iter()
            .any(|swing| same_moment(swing.time, crit.time));
        if !started || partners == 0 || swing_beside {
            continue;
        }
        let Some(&(_, left)) = logged
            .charges
            .iter()
            .filter(|&&(time, _)| same_batch(crit.time, time))
            .last()
        else {
            continue;
        };
        checked += 1;
        assert_eq!(
            left, 3,
            "the strike crit at {} left {left} charges",
            crit.time
        );
    }
    assert!(checked > 0, "no strike beside a strike crit");
}

/// Under the rule charges go faster, so Flurry is up less.
#[test]
fn flurry_is_up_less_under_the_rule() {
    let spending = prepare(&request(Rules::Forever, 300, false), Rules::Forever);
    let mut spared = spending.clone();
    flurry_mut(&mut spared)["imbue_strike_spends"] = json!(false);
    let (spending, spared) = (flurry_uptime(&run(&spending)), flurry_uptime(&run(&spared)));
    assert!(spending < spared, "{spending} vs {spared}");
}
