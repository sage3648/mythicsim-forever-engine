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
    // A caster of a shared debuff alone: the raid's toggle off, as the raid leaves its fight.
    for (party, slot, field) in [(1, 2, "faerieFire"), (3, 1, "curseOfElements")] {
        let input = reference_party(vec![reference_player(party, slot)], &[(field, false)]);
        let report = run(&input);
        let alone = solo(&input["request"]);
        assert_eq!(
            players(&report)[0],
            alone["raidMetrics"]["parties"][0]["players"][0],
            "{field}"
        );
    }
}

#[test]
fn the_raid_sums_its_members_and_the_first_fights_as_alone() {
    let druid = player("balance-druid-thorns");
    // Without its Curse of the Elements, which would be the raid's in place of the druid's
    // toggle: the druid then shares nothing that changes.
    let warlock = without_cast(player("demonology-warlock-pet-values"), 1311680);
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
    // The feral druid's Faerie Fire and the three warlocks' Curse of the Elements are the raid's.
    // A warlock's curse is up on every warlock's target, so none recasts it every global
    // cooldown.
    assert_eq!(find("faerie_fire")["casters"], json!([7]));
    assert_eq!(find("faerie_fire")["live"], json!(true));
    assert_eq!(find("curse_of_elements")["casters"], json!([16, 17, 18]));
    assert_eq!(find("curse_of_elements")["live"], json!(true));
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
    let mut party = vec![reference_player(1, 4)];
    party.extend(warrior);
    run(&reference_party(party, &[("sunderArmor", sunder_toggle)]))
}

/// A player of the reference raid, by party and slot.
fn reference_player(party: usize, slot: usize) -> Value {
    reference_raid()["request"]["raid"]["parties"][party]["players"][slot].clone()
}

/// The reference raid's request with one untanked party of the given players and the raid's
/// debuff toggles as given.
fn reference_party(party: Vec<Value>, toggles: &[(&str, bool)]) -> Value {
    let mut input = reference_raid();
    let buffs = input["request"]["raid"]["parties"][1]["buffs"].clone();
    input["request"]["raid"]["parties"] = json!([{ "players": party, "buffs": buffs }]);
    input["request"]["raid"]["tanks"] = json!([]);
    for (field, on) in toggles {
        input["request"]["raid"]["debuffs"][*field] = json!(on);
    }
    for target in input["request"]["encounter"]["targets"]
        .as_array_mut()
        .unwrap()
    {
        target.as_object_mut().unwrap().remove("tankIndex");
    }
    input
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

fn first_dps(report: &Value) -> f64 {
    players(report)[0]["dps"]["avg"].as_f64().unwrap()
}

fn shared(report: &Value, name: &str) -> Value {
    report["mythicsim"]["sharedDebuffs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|debuff| debuff["debuff"] == name)
        .cloned()
        .unwrap_or_else(|| panic!("no {name}"))
}

/// The longest uptime of any aura of the spell on the first member's target.
fn target_uptime(report: &Value, spell: i64) -> f64 {
    report["result"]["encounterMetrics"]["targets"][0]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|aura| aura["id"]["spellId"] == json!(spell))
        .map(|aura| aura["uptimeSecondsAvg"].as_f64().unwrap_or(0.0))
        .fold(0.0, f64::max)
}

fn duration(report: &Value) -> f64 {
    report["result"]["avgIterationDuration"].as_f64().unwrap()
}

/// The player with every cast of the spell in its rotation's priority list left out.
fn without_cast(mut player: Value, spell: i64) -> Value {
    let list = player["rotation"]["priorityList"].as_array_mut().unwrap();
    list.retain(|item| item["action"]["castSpell"]["spellId"]["spellId"] != json!(spell));
    player
}

/// The player with every cast of the spell in its rotation's priority list kept, but never
/// cast: its rotation casts the debuff, so the raid's copy follows it, and it is never up.
fn never_casts(mut player: Value, spell: i64) -> Value {
    let list = player["rotation"]["priorityList"].as_array_mut().unwrap();
    for item in list {
        if item["action"]["castSpell"]["spellId"]["spellId"] == json!(spell) {
            item["action"]["condition"] = json!({ "const": { "val": "false" } });
        }
    }
    player
}

/// A warlock of the reference raid who keeps Curse of Recklessness up in place of the
/// Elements.
fn recklessness_warlock() -> Value {
    let text = reference_player(3, 1).to_string();
    serde_json::from_str(&text.replace("1311680", "11717")).unwrap()
}

/// What a debuff a member casts does on a receiving member's target: nothing until the caster
/// lands it, and only while the caster keeps it up. The receiver is the first of the party,
/// and the raid's Faerie Fire and Curse of Recklessness are off, so neither holds the minor
/// armor category.
fn follows_the_caster(receiver: Value, caster: Value, field: &str, name: &str, spell: i64) {
    let toggles = |on: bool| {
        let mut toggles = vec![("faerieFire", false), ("curseOfRecklessness", false)];
        toggles.retain(|(other, _)| *other != field);
        toggles.push((field, on));
        toggles
    };
    let alone = |on: bool| run(&reference_party(vec![receiver.clone()], &toggles(on)));
    let (bare, assumed) = (alone(false), alone(true));
    let live = run(&reference_party(
        vec![receiver.clone(), caster.clone()],
        &toggles(false),
    ));
    assert_eq!(shared(&live, name)["casters"], json!([1]), "{name}");
    assert_eq!(shared(&live, name)["live"], json!(true), "{name}");
    let uptime = target_uptime(&live, spell);
    assert!(
        uptime > 0.5 * duration(&live) && uptime < duration(&live),
        "{name} is up {uptime} s"
    );
    assert_eq!(target_uptime(&bare, spell), 0.0, "{name}");
    let (bare, live, assumed) = (first_dps(&bare), first_dps(&live), first_dps(&assumed));
    assert!(
        bare < live && live < assumed,
        "{name}: {bare} {live} {assumed}"
    );
    // A caster whose rotation never casts it: nothing on the receiver's target, whatever the
    // raid's toggle says, as alone without it.
    let idle = run(&reference_party(
        vec![receiver, never_casts(caster, spell)],
        &toggles(true),
    ));
    assert_eq!(shared(&idle, name)["live"], json!(true), "{name}");
    assert_eq!(target_uptime(&idle, spell), 0.0, "{name}");
    assert_eq!(first_dps(&idle), bare, "{name}");
}

#[test]
fn faerie_fire_follows_the_druids_casts() {
    follows_the_caster(
        reference_player(1, 4),
        reference_player(1, 2),
        "faerieFire",
        "faerie_fire",
        9907,
    );
}

#[test]
fn curse_of_recklessness_follows_the_warlocks_casts() {
    follows_the_caster(
        reference_player(1, 4),
        recklessness_warlock(),
        "curseOfRecklessness",
        "curse_of_recklessness",
        11717,
    );
}

#[test]
fn curse_of_the_elements_follows_the_warlocks_casts() {
    follows_the_caster(
        reference_player(2, 0),
        reference_player(3, 1),
        "curseOfElements",
        "curse_of_elements",
        1311680,
    );
}

/// Casts of the spell a fight by the named member.
fn casts_a_fight(report: &Value, name: &str, spell: i64) -> f64 {
    casts(member(report, name), spell) / report["result"]["iterationsDone"].as_f64().unwrap()
}

#[test]
fn a_warlock_takes_another_warlocks_curse() {
    let demonology = reference_player(3, 1);
    let affliction = reference_player(3, 2);
    let toggles = [("curseOfElements", false)];
    let alone = run(&reference_party(vec![demonology.clone()], &toggles));
    let together = run(&reference_party(vec![demonology, affliction], &toggles));
    // Whoever lands the curse first keeps it up for both: neither recasts a curse the other
    // holds, so together they cast it about as often as one alone.
    let one = casts_a_fight(&alone, "demonology-warlock", 1311680);
    let both = casts_a_fight(&together, "demonology-warlock", 1311680)
        + casts_a_fight(&together, "affliction-warlock", 1311680);
    assert!(
        one >= 1.0 && both < 1.5 * one,
        "{one} alone, {both} together"
    );
    let uptime = target_uptime(&together, 1311680);
    assert!(uptime > 0.9 * duration(&together), "{uptime}");
}

#[test]
fn a_feral_druids_faerie_fire_and_a_balance_druids_are_one() {
    let feral = reference_player(1, 2);
    // Forever has no Faerie Fire (Feral): the Balance druid keeps up the feral's spell.
    let mut balance = reference_player(2, 3);
    let faerie_fire = feral["rotation"]["priorityList"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["action"]["castSpell"]["spellId"]["spellId"] == json!(9907))
        .cloned()
        .unwrap();
    balance["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap()
        .insert(0, faerie_fire);
    let toggles = [("faerieFire", false)];
    let alone = |player: &Value| run(&reference_party(vec![player.clone()], &toggles));
    let together = run(&reference_party(
        vec![feral.clone(), balance.clone()],
        &toggles,
    ));
    assert_eq!(shared(&together, "faerie_fire")["casters"], json!([0, 1]));
    for name in ["feral-druid", "balance-druid"] {
        let player = if name == "feral-druid" {
            &feral
        } else {
            &balance
        };
        let own = casts_a_fight(&alone(player), name, 9907);
        let shared = casts_a_fight(&together, name, 9907);
        assert!(
            shared < own,
            "{name}: {shared} beside the other, {own} alone"
        );
    }
}

#[test]
fn faerie_fire_and_curse_of_recklessness_hold_one_armor_category() {
    let hunter = reference_player(1, 4);
    let off = [("faerieFire", false), ("curseOfRecklessness", false)];
    let on = [("faerieFire", true), ("curseOfRecklessness", true)];
    let recklessness = first_dps(&run(&reference_party(
        vec![hunter.clone(), recklessness_warlock()],
        &off,
    )));
    let both = first_dps(&run(&reference_party(
        vec![
            hunter.clone(),
            recklessness_warlock(),
            reference_player(1, 2),
        ],
        &off,
    )));
    let assumed = first_dps(&run(&reference_party(vec![hunter], &on)));
    // Faerie Fire adds only while the curse is down, and never on top of it.
    assert!(
        recklessness < both && both < assumed,
        "{recklessness} {both} {assumed}"
    );
}

#[test]
fn curse_of_recklessness_reaches_a_tanked_target() {
    let mut input = reference_raid();
    let text = input["request"]["raid"]["parties"][3]["players"][3].to_string();
    input["request"]["raid"]["parties"][3]["players"][3] =
        serde_json::from_str(&text.replace("1311680", "11717")).unwrap();
    input["request"]["raid"]["debuffs"]["curseOfRecklessness"] = json!(false);
    let report = run(&input);
    assert_eq!(
        shared(&report, "curse_of_recklessness")["casters"],
        json!([18])
    );
    assert_eq!(report["mythicsim"]["members"][0]["tanking"], json!(true));
    // The tank's target carries the curse while the warlock keeps it up.
    let uptime = target_uptime(&report, 11717);
    assert!(uptime > 0.5 * duration(&report), "{uptime}");
}
