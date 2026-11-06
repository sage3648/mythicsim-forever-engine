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

fn wisdom(report: &Value) -> Value {
    report["mythicsim"]["sharedDebuffs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|debuff| debuff["debuff"] == "judgement_of_wisdom")
        .cloned()
        .expect("judgement of wisdom")
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
    assert_eq!(wisdom(&idle)["casters"], json!([1]));
    assert_eq!(wisdom_mana(&idle), 0.0);
    // A paladin who judges it: the druid's mana returns follow the paladin's judgements.
    let judged = with(vec![druid, wisdom_paladin(true)]);
    let mana = wisdom_mana(&judged);
    assert!(mana > 0.5 * wisdom_mana(&assumed), "{mana}");
    assert!(mana < wisdom_mana(&assumed), "{mana}");
}

fn reference_raid() -> Value {
    let path = format!(
        "{}/fixtures/raid/reference-raid-20.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_slice(&std::fs::read(path).expect("fixture")).expect("fixture json")
}

fn member<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["result"]["raidMetrics"]["parties"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|party| party["players"].as_array().unwrap())
        .find(|player| player["name"] == name)
        .unwrap_or_else(|| panic!("no member {name}"))
}

fn casts(player: &Value, spell: i64) -> f64 {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"] == json!(spell))
        .flat_map(|action| action["targets"].as_array().unwrap())
        .map(|target| target["casts"].as_f64().unwrap_or(0.0))
        .sum()
}

#[test]
fn a_reference_raid_shares_what_its_members_cast() {
    let report = run(&reference_raid());
    let shared = report["mythicsim"]["sharedDebuffs"].as_array().unwrap();
    let find = |name: &str| {
        shared
            .iter()
            .find(|debuff| debuff["debuff"] == name)
            .unwrap_or_else(|| panic!("no {name}"))
            .clone()
    };
    // The four warriors' rotations sunder; their stacks are the raid's.
    assert_eq!(find("sunder_armor")["casters"], json!([0, 1, 2, 3]));
    assert_eq!(find("sunder_armor")["live"], json!(true));
    // The three warlocks cast Curse of the Elements: the others assume it, while each warlock's
    // own fight has its own curse, which lands instead of being recast every global cooldown.
    assert_eq!(find("curse_of_elements")["casters"], json!([16, 17, 18]));
    assert_eq!(find("curse_of_elements")["live"], json!(false));
    let iterations = report["result"]["iterationsDone"].as_f64().unwrap();
    for warlock in [
        "demonology-warlock",
        "affliction-warlock",
        "destruction-warlock",
    ] {
        let player = member(&report, warlock);
        let curses = casts(player, 1311680) / iterations;
        assert!(
            curses < 10.0,
            "{warlock} casts the curse {curses} times a fight"
        );
        assert!(player["dps"]["avg"].as_f64().unwrap() > 600.0, "{warlock}");
    }
    assert_eq!(report["mythicsim"]["members"].as_array().unwrap().len(), 20);
    assert_eq!(report["mythicsim"]["members"][0]["tanking"], json!(true));
}

/// The hunter of the reference raid beside the given warrior, with the raid's Sunder Armor
/// toggle as given.
fn hunter_beside(warrior: Option<Value>, sunder_toggle: bool) -> Value {
    let raid = reference_raid();
    let players = &raid["request"]["raid"]["parties"];
    let hunter = players[1]["players"][4].clone();
    let mut input = raid.clone();
    let mut party = vec![hunter];
    party.extend(warrior);
    input["request"]["raid"]["parties"] =
        json!([{ "players": party, "buffs": players[1]["buffs"] }]);
    input["request"]["raid"]["tanks"] = json!([]);
    input["request"]["raid"]["debuffs"]["sunderArmor"] = json!(sunder_toggle);
    for target in input["request"]["encounter"]["targets"]
        .as_array_mut()
        .unwrap()
    {
        target.as_object_mut().unwrap().remove("tankIndex");
    }
    run(&input)
}

#[test]
fn sunder_armor_follows_the_warriors_stacks() {
    let raid = reference_raid();
    let arms = raid["request"]["raid"]["parties"][0]["players"][2].clone();
    let dps = |report: &Value| players(report)[0]["dps"]["avg"].as_f64().unwrap();
    let sunder_uptime = |report: &Value| -> f64 {
        report["result"]["encounterMetrics"]["targets"][0]["auras"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|aura| aura["id"]["spellId"] == json!(11597))
            .map(|aura| aura["uptimeSecondsAvg"].as_f64().unwrap_or(0.0))
            .fold(0.0, f64::max)
    };
    let bare = hunter_beside(None, false);
    let assumed = hunter_beside(None, true);
    let sundered = hunter_beside(Some(arms.clone()), false);
    assert_eq!(
        sundered["mythicsim"]["sharedDebuffs"][0]["casters"],
        json!([1])
    );
    // The warrior's stacks reach the hunter's target, which takes the armor off for the hunter.
    let duration = sundered["result"]["avgIterationDuration"].as_f64().unwrap();
    assert!(
        sunder_uptime(&sundered) > 0.9 * duration,
        "{}",
        sunder_uptime(&sundered)
    );
    assert!(
        dps(&sundered) > dps(&bare) * 1.02,
        "{} vs {}",
        dps(&sundered),
        dps(&bare)
    );
    // A warrior who never sunders leaves the raid's toggle to the hunter, as alone.
    let mut idle = arms;
    let rotation = idle["rotation"]["priorityList"].as_array_mut().unwrap();
    rotation.retain(|item| item["action"]["castSpell"]["spellId"]["spellId"] != json!(11597));
    let beside_idle = hunter_beside(Some(idle), true);
    assert_eq!(
        beside_idle["mythicsim"]["sharedDebuffs"][0]["live"],
        json!(false)
    );
    assert_eq!(players(&beside_idle)[0], players(&assumed)[0]);
}

/// The shadow priest with Power Infusion, whose rotation autocasts its cooldowns, without the
/// fixture's assumed external one.
fn shadow_priest() -> Value {
    let path = format!(
        "{}/fixtures/mage/prepared-v2/shadow-priest-own-and-external-power-infusion.request.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let request: Value =
        serde_json::from_slice(&std::fs::read(path).expect("fixture")).expect("fixture json");
    let mut priest = request["raid"]["parties"][0]["players"][0].clone();
    priest["buffs"]
        .as_object_mut()
        .unwrap()
        .remove("powerInfusions");
    priest
}

/// A druid and a priest, each with an assumed copy of what the other is assigned to cast on it.
fn druid_and_priest(mut druid: Value, mut priest: Value) -> Value {
    druid["buffs"]["powerInfusions"] = json!(1);
    priest["buffs"]["innervates"] = json!(1);
    let mut input = raid("balance-druid-thorns", vec![vec![druid, priest]], 20);
    input["request"]["simOptions"]["debugFirstIteration"] = json!(true);
    input["assignments"] = json!({
        "innervate": [{ "from": 0, "to": 1 }],
        "powerInfusion": [{ "from": 1, "to": 0 }],
    });
    input
}

/// The times of a member's first iteration log lines that contain `text`.
fn log_times(report: &Value, member: &str, text: &str) -> Vec<String> {
    let unit = format!("] [{member} (#1)] ");
    report["result"]["logs"]
        .as_str()
        .expect("the raid's log")
        .lines()
        .filter(|line| line.contains(&unit) && line.contains(text))
        .map(|line| line[1..line.find(']').unwrap()].to_string())
        .collect()
}

fn assignment(report: &Value, spell: &str) -> Value {
    report["mythicsim"]["assignments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|assignment| assignment["spell"] == spell)
        .unwrap_or_else(|| panic!("no {spell} assignment"))
        .clone()
}

#[test]
fn an_assigned_innervate_and_power_infusion_land_when_their_casters_cast_them() {
    let report = run(&druid_and_priest(
        player("balance-druid-thorns"),
        shadow_priest(),
    ));
    for (spell, caster, target, id) in [
        ("innervate", "balance-druid", "shadow-priest", 29166),
        ("powerInfusion", "shadow-priest", "balance-druid", 10060),
    ] {
        let cast = log_times(
            &report,
            caster,
            &format!("Completed cast {{SpellID: {id}}}"),
        );
        assert!(!cast.is_empty(), "{caster} never casts {spell}");
        let gained = format!("Aura gained: {{SpellID: {id},");
        // The target gains the aura exactly when the caster casts it, and the caster never.
        assert_eq!(log_times(&report, target, &gained), cast, "{spell}");
        assert!(log_times(&report, caster, &gained).is_empty(), "{spell}");
        let summary = assignment(&report, spell);
        assert_eq!(summary["cast"], json!(true));
        assert!(summary["casts"].as_u64().unwrap() >= 20, "{summary}");
        assert_eq!(summary["landed"], summary["casts"], "{summary}");
        // The caster's casts replace the target's assumed ones.
        assert_eq!(casts(member(&report, target), id), 0.0, "{spell}");
    }
}

#[test]
fn an_assigned_caster_whose_rotation_never_casts_gives_nothing() {
    let mut druid = player("balance-druid-thorns");
    druid["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .retain(|item| item["action"]["castSpell"]["spellId"]["spellId"] != json!(29166));
    let mut priest = shadow_priest();
    priest["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .retain(|item| item["action"].get("autocastOtherCooldowns").is_none());
    let report = run(&druid_and_priest(druid.clone(), priest.clone()));
    for (spell, target, id) in [
        ("innervate", "shadow-priest", 29166),
        ("powerInfusion", "balance-druid", 10060),
    ] {
        let summary = assignment(&report, spell);
        assert_eq!(summary["cast"], json!(false), "{summary}");
        assert_eq!(summary["casts"], json!(0));
        assert_eq!(summary["landed"], json!(0));
        assert!(log_times(&report, target, &format!("Aura gained: {{SpellID: {id},")).is_empty());
    }
    // The targets fight as with neither the assignment nor the assumed casts.
    let mut bare = raid("balance-druid-thorns", vec![vec![druid, priest]], 20);
    bare["request"]["simOptions"]["debugFirstIteration"] = json!(true);
    let bare = run(&bare);
    for (assigned, alone) in players(&report).iter().zip(players(&bare)) {
        assert_eq!(assigned["dps"], alone["dps"]);
    }
}

#[test]
fn an_assignment_names_one_other_member_per_caster() {
    let refused = |assignments: Value| {
        let mut input = druid_and_priest(player("balance-druid-thorns"), shadow_priest());
        input["assignments"] = assignments;
        match simulate_raid(&serde_json::to_vec(&input).unwrap(), Rules::Forever, false) {
            Err(RaidError::Invalid(reason)) => reason,
            other => panic!("{:?}", other.map(|_| ())),
        }
    };
    assert!(refused(json!({ "innervate": [{ "from": 0, "to": 0 }] })).contains("itself"));
    assert!(refused(json!({ "powerInfusion": [{ "from": 0, "to": 1 }] })).contains("cannot"));
    assert!(
        refused(json!({ "innervate": [{ "from": 0, "to": 1 }, { "from": 0, "to": 1 }] }))
            .contains("two members")
    );
}
