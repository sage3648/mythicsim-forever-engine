//! Raid simulations: members prepared and gated as Quick Sims, run on one clock.

use forever_engine::{simulate_raid, RaidError, Rules};
use serde_json::{json, Value};

fn request(name: &str) -> Value {
    let path = format!(
        "{}/fixtures/forever-gaps/{name}.request.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).expect("fixture")).expect("fixture json")
}

fn player(name: &str) -> Value {
    request(name)["raid"]["parties"][0]["players"][0].clone()
}

/// The first fixture's request with the given parties of players.
fn raid(base: &str, parties: Vec<Vec<Value>>, iterations: u32) -> Value {
    let mut request = request(base);
    let buffs = request["raid"]["parties"][0]["buffs"].clone();
    request["raid"]["parties"] = parties
        .into_iter()
        .map(|players| json!({ "players": players, "buffs": buffs }))
        .collect();
    request["simOptions"] = json!({ "iterations": iterations, "randomSeed": "42" });
    json!({ "schema": 1, "request": request })
}

fn run(input: &Value) -> Value {
    simulate_raid(&serde_json::to_vec(input).unwrap(), Rules::Forever, false)
        .unwrap_or_else(|err| panic!("{err}"))
}

fn solo(request: &Value) -> Value {
    let bytes = serde_json::to_vec(request).unwrap();
    let prepared = forever_engine::prepare(&bytes, "raid").expect("prepares");
    forever_engine::simulate_prepared(&prepared)
        .expect("simulates")
        .result
}

fn players(report: &Value) -> Vec<Value> {
    report["result"]["raidMetrics"]["parties"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|party| party["players"].as_array().unwrap().clone())
        .collect()
}

#[test]
fn a_raid_of_one_is_its_quick_sim() {
    for name in [
        "balance-druid-thorns",
        "demonology-warlock-pet-values",
        "hunter-skysight",
    ] {
        let request = request(name);
        let report = run(&json!({ "schema": 1, "request": request }));
        let alone = solo(&request);
        assert_eq!(
            players(&report)[0],
            alone["raidMetrics"]["parties"][0]["players"][0],
            "{name}"
        );
        assert_eq!(
            report["result"]["raidMetrics"]["dps"]["avg"],
            alone["raidMetrics"]["dps"]["avg"]
        );
    }
}

#[test]
fn the_raid_sums_its_members_and_the_first_fights_as_alone() {
    let druid = player("balance-druid-thorns");
    let warlock = player("demonology-warlock-pet-values");
    let hunter = player("hunter-skysight");
    let input = raid(
        "balance-druid-thorns",
        vec![
            vec![druid.clone(), warlock.clone()],
            vec![hunter.clone(), druid.clone()],
        ],
        40,
    );
    let report = run(&input);
    let members = players(&report);
    assert_eq!(members.len(), 4);
    let dps = |player: &Value| player["dps"]["avg"].as_f64().unwrap();
    let raid_dps = report["result"]["raidMetrics"]["dps"]["avg"]
        .as_f64()
        .unwrap();
    let sum: f64 = members.iter().map(dps).sum();
    assert!((raid_dps - sum).abs() < 1e-9 * sum, "{raid_dps} vs {sum}");
    let parties = report["result"]["raidMetrics"]["parties"]
        .as_array()
        .unwrap();
    let party_dps = |party: &Value| party["dps"]["avg"].as_f64().unwrap();
    assert!((party_dps(&parties[0]) - dps(&members[0]) - dps(&members[1])).abs() < 1e-6);
    assert!((party_dps(&parties[1]) - dps(&members[2]) - dps(&members[3])).abs() < 1e-6);

    // The first member's seed is the raid's and its fight length its own roll.
    let mut first = request("balance-druid-thorns");
    first["raid"]["parties"] = json!([{
        "players": [druid.clone()],
        "buffs": input["request"]["raid"]["parties"][0]["buffs"],
    }]);
    first["simOptions"] = input["request"]["simOptions"].clone();
    assert_eq!(
        members[0],
        solo(&first)["raidMetrics"]["parties"][0]["players"][0]
    );

    // Two copies of a player draw their own random numbers.
    assert_ne!(dps(&members[0]), dps(&members[3]));

    let summary = &report["mythicsim"]["members"];
    let indexes: Vec<u64> = summary
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member["raidIndex"].as_u64().unwrap())
        .collect();
    assert_eq!(indexes, [0, 1, 5, 6]);
}

#[test]
fn every_member_fights_to_the_first_members_length() {
    let input = raid(
        "balance-druid-thorns",
        vec![
            vec![player("balance-druid-thorns"), player("hunter-skysight")],
            vec![player("demonology-warlock-pet-values")],
        ],
        20,
    );
    let report = run(&input);
    let duration = &report["result"]["avgIterationDuration"];
    for member in report["mythicsim"]["members"].as_array().unwrap() {
        assert_eq!(
            &member["avgIterationDuration"], duration,
            "{}",
            member["name"]
        );
    }
    // Alone from its own seed in the raid, the hunter would roll lengths of its own.
    let mut alone = input["request"].clone();
    alone["raid"]["parties"] = json!([{ "players": [player("hunter-skysight")] }]);
    alone["simOptions"]["randomSeed"] = json!((42i64 + (1 << 32)).to_string());
    assert_ne!(&solo(&alone)["avgIterationDuration"], duration);
}

#[test]
fn a_member_rust_cannot_simulate_refuses_the_raid_by_name() {
    let input = raid(
        "balance-druid-thorns",
        vec![vec![
            player("balance-druid-thorns"),
            player("arms-warrior-revenge"),
        ]],
        10,
    );
    match simulate_raid(&serde_json::to_vec(&input).unwrap(), Rules::Forever, false) {
        Err(RaidError::Refused(refusals)) => {
            assert_eq!(refusals.len(), 1);
            assert_eq!(refusals[0].member, 1);
            assert_eq!(refusals[0].code, "talents");
        }
        other => panic!("expected a refusal, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn a_raid_needs_a_seed_and_at_most_five_a_party() {
    let mut input = raid(
        "balance-druid-thorns",
        vec![vec![player("balance-druid-thorns")]],
        10,
    );
    input["request"]["simOptions"] = json!({ "iterations": 10 });
    let err =
        simulate_raid(&serde_json::to_vec(&input).unwrap(), Rules::Forever, false).unwrap_err();
    assert!(err.to_string().contains("randomSeed"), "{err}");
    let druid = player("balance-druid-thorns");
    let input = raid("balance-druid-thorns", vec![vec![druid; 6]], 10);
    let err =
        simulate_raid(&serde_json::to_vec(&input).unwrap(), Rules::Forever, false).unwrap_err();
    assert!(err.to_string().contains("at most 5"), "{err}");
}

/// A Retribution paladin who keeps Seal of Wisdom up and judges it, or only keeps the seal up.
fn wisdom_paladin(judges: bool) -> Value {
    let mut paladin = player("retribution-paladin-swap-ironfoe");
    let seal = json!({ "spellId": 20357 });
    let judgement = json!({ "spellId": 20271 });
    let mut priority = vec![json!({ "action": {
        "castSpell": { "spellId": seal },
        "condition": { "not": { "val": { "auraIsActive": { "auraId": seal } } } },
    } })];
    if judges {
        priority.insert(
            0,
            json!({ "action": {
            "castSpell": { "spellId": judgement },
            "condition": { "and": { "vals": [
                { "auraIsActive": { "auraId": seal } },
                { "spellCanCast": { "spellId": judgement } },
            ] } },
        } }),
        );
    }
    paladin["rotation"] = json!({
        "type": "TypeAPL",
        "prepullActions": [{ "action": { "castSpell": { "spellId": seal } },
                             "doAtValue": { "const": { "val": "-1.5s" } } }],
        "priorityList": priority,
    });
    paladin
}

fn wisdom_mana(report: &Value) -> f64 {
    let druid = &players(report)[0];
    druid["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|resource| resource["id"]["spellId"] == json!(20355))
        .map(|resource| resource["gain"].as_f64().unwrap_or(0.0))
        .sum()
}

#[test]
fn judgement_of_wisdom_lasts_only_while_a_paladin_in_the_raid_judges_it() {
    let druid = player("balance-druid-thorns");
    let with = |players: Vec<Value>| {
        let mut input = raid("balance-druid-thorns", vec![players], 30);
        input["request"]["raid"]["debuffs"]["judgementOfWisdom"] = json!(true);
        run(&input)
    };
    // Nobody in the raid judges Wisdom: the toggle's copy is up from the pull, as alone.
    let assumed = with(vec![druid.clone()]);
    assert!(wisdom_mana(&assumed) > 0.0);
    assert_eq!(
        assumed["mythicsim"]["sharedDebuffs"][0]["live"],
        json!(false)
    );
    // A paladin who could judge it but does not: the druid gets nothing.
    let idle = with(vec![druid.clone(), wisdom_paladin(false)]);
    assert_eq!(idle["mythicsim"]["sharedDebuffs"][0]["casters"], json!([1]));
    assert_eq!(wisdom_mana(&idle), 0.0);
    // A paladin who judges it: the druid's mana returns follow the paladin's judgements.
    let judged = with(vec![druid, wisdom_paladin(true)]);
    let mana = wisdom_mana(&judged);
    assert!(mana > 0.5 * wisdom_mana(&assumed), "{mana}");
    assert!(mana < wisdom_mana(&assumed), "{mana}");
}
