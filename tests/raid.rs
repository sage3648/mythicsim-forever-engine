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

/// The reference raid's boss as `targets` copies, one tank list entry per raid index in `tanks`,
/// and the given players in one party. A target is its `tankIndex` and optional `secondTankIndex`.
fn tank_raid(
    players: Vec<Value>,
    tanks: &[usize],
    targets: &[(i64, Option<i64>)],
    iterations: u32,
) -> Value {
    let mut input = reference_raid();
    let buffs = input["request"]["raid"]["parties"][1]["buffs"].clone();
    input["request"]["raid"]["parties"] = json!([{ "players": players, "buffs": buffs }]);
    input["request"]["raid"]["tanks"] = tanks
        .iter()
        .map(|index| json!({ "type": "Player", "index": index }))
        .collect();
    let boss = input["request"]["encounter"]["targets"][0].clone();
    input["request"]["encounter"]["targets"] = targets
        .iter()
        .map(|&(first, second)| {
            let mut target = boss.clone();
            target["tankIndex"] = json!(first);
            if let Some(second) = second {
                target["secondTankIndex"] = json!(second);
            }
            target
        })
        .collect();
    input["request"]["simOptions"] = json!({ "iterations": iterations, "randomSeed": "42" });
    input
}

/// A member's damage taken per second; a member nothing swings at has none.
fn dtps(report: &Value, name: &str) -> f64 {
    member(report, name)["dtps"]["avg"].as_f64().unwrap_or(0.0)
}

fn tanking(report: &Value) -> Vec<bool> {
    report["mythicsim"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member["tanking"].as_bool().unwrap())
        .collect()
}

#[test]
fn each_target_swings_only_at_its_own_tank() {
    let (first, second) = (reference_player(0, 0), reference_player(0, 1));
    let hunter = reference_player(1, 4);
    let alone = |player: &Value| run(&tank_raid(vec![player.clone()], &[0], &[(0, None)], 300));
    let (first_alone, second_alone) = (
        dtps(&alone(&first), "protection-warrior"),
        dtps(&alone(&second), "fury-protection-warrior"),
    );

    // Two targets, two tanks, one each: each tank takes what it takes tanking one target alone.
    // The tolerance is 5%: a tank's damage taken per second varies about 7% from fight to fight
    // and two runs with other seeds agree within about 2% at 300 iterations, so 5% is a wide
    // margin for the noise and still far from the +100% of a second boss swinging at the tank.
    let split = run(&tank_raid(
        vec![first.clone(), second.clone(), hunter.clone()],
        &[0, 1],
        &[(0, None), (1, None)],
        300,
    ));
    let (first_split, second_split) = (
        dtps(&split, "protection-warrior"),
        dtps(&split, "fury-protection-warrior"),
    );
    assert!(
        (first_split / first_alone - 1.0).abs() < 0.05,
        "{first_split} against {first_alone} alone"
    );
    assert!(
        (second_split / second_alone - 1.0).abs() < 0.05,
        "{second_split} against {second_alone} alone"
    );
    assert_eq!(tanking(&split), [true, true, false]);
    // The hunter is hit by nothing.
    assert_eq!(dtps(&split, "hunter"), 0.0);

    // Both targets on the first tank: it takes two swings, the second tank none and is no tank.
    let both = run(&tank_raid(
        vec![first, second, hunter],
        &[0, 1],
        &[(0, None), (0, None)],
        300,
    ));
    let doubled = dtps(&both, "protection-warrior") / first_alone;
    assert!((doubled - 2.0).abs() < 0.2, "{doubled} swings' worth");
    assert_eq!(dtps(&both, "fury-protection-warrior"), 0.0);
    assert_eq!(tanking(&both), [true, false, false]);
}

/// The raid with a fixed fight length, so a swap lands at the same moments every iteration.
fn fixed_length(mut input: Value, swap_seconds: Option<f64>) -> Value {
    input["request"]["encounter"]["durationVariation"] = json!(0);
    if let Some(seconds) = swap_seconds {
        input["tankSwapSeconds"] = json!(seconds);
    }
    input
}

/// The times of each member's boss swings in the first iteration, by member in raid order. The
/// raid's log is every member's in turn, each starting again at the pull.
fn swing_times(report: &Value) -> Vec<Vec<f64>> {
    let mut members: Vec<Vec<f64>> = Vec::new();
    let mut last = f64::MAX;
    for line in report["result"]["logs"].as_str().unwrap().lines() {
        let Some((time, rest)) = line[1..].split_once("] [") else {
            continue;
        };
        let time: f64 = time.parse().unwrap();
        if time < last - 50.0 {
            members.push(Vec::new());
        }
        last = time;
        if rest.starts_with("Target ") && rest.contains("Casting {OtherID: 3, Tag: 1}") {
            members.last_mut().unwrap().push(time);
        }
    }
    members
}

#[test]
fn a_target_swaps_between_two_tanks_on_a_timer() {
    let (first, second) = (reference_player(0, 0), reference_player(0, 1));
    let alone = |player: &Value, name: &str| {
        let input = fixed_length(
            tank_raid(vec![player.clone()], &[0], &[(0, None)], 300),
            None,
        );
        dtps(&run(&input), name)
    };
    let (first_alone, second_alone) = (
        alone(&first, "protection-warrior"),
        alone(&second, "fury-protection-warrior"),
    );
    let mut input = fixed_length(
        tank_raid(vec![first, second], &[0, 1], &[(0, Some(1))], 300),
        Some(30.0),
    );
    let swapped = run(&input);
    // Each tank holds the boss for half of the 120 second fight and takes about half of what it
    // takes tanking it alone. The tolerance is 10% of that: a swing at a window's edge goes to one
    // tank or the other, which is a swing in sixty, and the fight to fight noise is a few percent
    // when only half the swings land. At 3,000 iterations the second tank takes 0.536 of its damage
    // alone and the first 0.507, before and after Ironfoe's 3% (13 November 2026), so the second
    // tank's share sits near the edge of an 8% tolerance at the test's 300.
    for (name, alone) in [
        ("protection-warrior", first_alone),
        ("fury-protection-warrior", second_alone),
    ] {
        let share = dtps(&swapped, name) / alone;
        assert!(
            (share - 0.5).abs() < 0.05,
            "{name} takes {share} of its damage alone"
        );
    }
    assert_eq!(tanking(&swapped), [true, true]);

    // The first iteration's swings alternate at the 30 second marks: the first tank's fall in
    // the windows from 0 to 30 and 60 to 90, the second's in the others, and each lands about
    // every other swing of the boss's two seconds.
    input["request"]["simOptions"]["iterations"] = json!(2);
    input["request"]["simOptions"]["debugFirstIteration"] = json!(true);
    let logged = run(&input);
    let swings = swing_times(&logged);
    assert_eq!(swings.len(), 2);
    for (tank, times) in swings.iter().enumerate() {
        assert!(
            times.len() >= 20 && times.len() <= 40,
            "{} swings",
            times.len()
        );
        for time in times {
            // The log rounds to hundredths, so a swing in a window's last 5 ms reads as the
            // mark that ends it (119.995 as 120.00).
            let windows = [*time, time - 0.005].map(|time| (time / 30.0).floor() as usize % 2);
            assert!(windows.contains(&tank), "tank {tank} is hit at {time}");
        }
        // Both of the tank's windows see swings.
        for window in (tank..4).step_by(2) {
            let in_window = times
                .iter()
                .filter(|time| (**time / 30.0).floor() as usize == window)
                .count();
            assert!(
                in_window >= 8,
                "tank {tank} window {window}: {in_window} swings"
            );
        }
    }
}

#[test]
fn a_swap_time_changes_nothing_without_a_second_tank() {
    let (first, second) = (reference_player(0, 0), reference_player(0, 1));
    let hunter = reference_player(1, 4);
    // The report's digest is of the input bytes, and its timing of the run.
    let strip = |mut report: Value| {
        report.as_object_mut().unwrap().remove("elapsed_ns");
        report.as_object_mut().unwrap().remove("request_sha256");
        report
    };
    // One tank; a second tank entry that is the first; one target each for two tanks.
    for (players, tanks, targets) in [
        (
            vec![first.clone(), hunter.clone()],
            vec![0],
            vec![(0, None)],
        ),
        (
            vec![first.clone(), hunter.clone()],
            vec![0],
            vec![(0, Some(0))],
        ),
        (
            vec![first.clone(), second.clone()],
            vec![0, 1],
            vec![(0, None), (1, None)],
        ),
    ] {
        let without = tank_raid(players, &tanks, &targets, 20);
        let mut with = without.clone();
        with["tankSwapSeconds"] = json!(45);
        assert_eq!(strip(run(&without)), strip(run(&with)), "{targets:?}");
    }
}

#[test]
fn a_second_tank_needs_a_swap_time_in_range() {
    let (first, second) = (reference_player(0, 0), reference_player(0, 1));
    let input = tank_raid(vec![first, second], &[0, 1], &[(0, Some(1))], 20);
    match simulate_raid(&serde_json::to_vec(&input).unwrap(), Rules::Forever, false) {
        Err(RaidError::Refused(refusals)) => {
            assert_eq!(refusals.len(), 2);
            assert!(
                refusals[0].reason.contains("tankSwapSeconds"),
                "{}",
                refusals[0].reason
            );
        }
        other => panic!("{:?}", other.map(|_| ())),
    }
    for seconds in [4.0, 601.0] {
        let mut input = input.clone();
        input["tankSwapSeconds"] = json!(seconds);
        match simulate_raid(&serde_json::to_vec(&input).unwrap(), Rules::Forever, false) {
            Err(RaidError::Invalid(reason)) => assert!(reason.contains("tankSwapSeconds")),
            other => panic!("{:?}", other.map(|_| ())),
        }
    }
}

/// A healer from the healer fixtures: a player with its heals and no tank of its own.
fn healer(name: &str) -> Value {
    let path = format!(
        "{}/fixtures/healers/{name}.request.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let request: Value =
        serde_json::from_slice(&std::fs::read(path).expect("fixture")).expect("fixture json");
    request["raid"]["parties"][0]["players"][0].clone()
}

/// The first tank of the reference raid with healers beside it, the boss's swing weakened to
/// 1000 damage so a healer or two can cover it, and each healer assigned to the tank when
/// `assigned` says so (healers after the tank, at raid indexes 1, 2, ...).
fn healed_tank(healers: &[&str], assigned: &[bool], iterations: u32) -> Value {
    let mut players = vec![reference_player(0, 0)];
    players.extend(healers.iter().map(|name| healer(name)));
    let mut input = tank_raid(players, &[0], &[(0, None)], iterations);
    input["request"]["encounter"]["targets"][0]["minBaseDamage"] = json!(1000);
    let list: Vec<Value> = assigned
        .iter()
        .enumerate()
        .filter(|(_, assigned)| **assigned)
        .map(|(index, _)| json!({ "from": index + 1, "to": 0 }))
        .collect();
    if !list.is_empty() {
        input["assignments"] = json!({ "tankHealers": list });
    }
    input
}

fn tank_healing(report: &Value) -> Vec<Value> {
    report["mythicsim"]["tankHealing"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn effective(report: &Value, from: u64) -> f64 {
    tank_healing(report)
        .iter()
        .find(|entry| entry["from"] == from)
        .unwrap_or_else(|| panic!("no tank healing from {from}"))["effective"]
        .as_f64()
        .unwrap()
}

#[test]
fn an_assigned_healer_heals_what_the_tank_takes_and_no_more() {
    for name in [
        "holy-priest",
        "restoration-druid",
        "restoration-shaman",
        "holy-paladin",
    ] {
        let report = run(&healed_tank(&[name], &[true], 100));
        let taken = dtps(&report, "protection-warrior");
        let entry = &tank_healing(&report)[0];
        assert_eq!(
            (entry["from"].as_u64(), entry["to"].as_u64()),
            (Some(1), Some(0))
        );
        let (landed, overheal, share) = (
            entry["effective"].as_f64().unwrap(),
            entry["overheal"].as_f64().unwrap(),
            entry["share"].as_f64().unwrap(),
        );
        assert!(
            landed > 0.0 && landed <= taken,
            "{name}: {landed} against {taken} taken"
        );
        // The weak boss leaves heals over: the healer casts without reading the tank's health.
        assert!(overheal > 0.0, "{name} overheals nothing");
        assert!(
            (share - landed / taken).abs() < 0.01 * share,
            "{name}: share {share}"
        );
        // The healer's own healing includes what landed on the tank.
        let own = member(&report, healer(name)["name"].as_str().unwrap());
        assert!(
            own["hps"]["avg"].as_f64().unwrap() >= landed - 1e-6,
            "{name}"
        );
    }
}

#[test]
fn healers_on_one_tank_share_what_it_takes() {
    let both = run(&healed_tank(
        &["holy-priest", "restoration-druid"],
        &[true, true],
        100,
    ));
    let taken = dtps(&both, "protection-warrior");
    let (first, second) = (effective(&both, 1), effective(&both, 2));
    assert!(
        first + second <= taken,
        "{first} and {second} against {taken} taken"
    );
    // The second healer, behind the first in raid order, alone would have healed more: the first
    // leaves less of the tank's health missing for it.
    let alone = run(&healed_tank(&["restoration-druid"], &[true], 100));
    let alone = effective(&alone, 1);
    assert!(
        second < 0.95 * alone,
        "{second} beside another healer, {alone} alone"
    );
    // With a healer on each, the tank survives more fights than with one healer or none.
    let none = run(&healed_tank(
        &["holy-priest", "restoration-druid"],
        &[false, false],
        100,
    ));
    let death = |report: &Value| {
        member(report, "protection-warrior")["chanceOfDeath"]
            .as_f64()
            .unwrap_or(0.0)
    };
    assert_eq!(death(&none), 1.0);
    assert!(death(&both) < 0.5, "chance of death {}", death(&both));
    assert!(tank_healing(&none).is_empty());
}

#[test]
fn an_unassigned_healer_heals_as_without_assignments() {
    // The priest has no assignment: it heals as in a raid with none, bit for bit, whatever the
    // druid does to the tank.
    let assigned = run(&healed_tank(
        &["holy-priest", "restoration-druid"],
        &[false, true],
        60,
    ));
    let bare = run(&healed_tank(
        &["holy-priest", "restoration-druid"],
        &[false, false],
        60,
    ));
    assert_eq!(
        member(&assigned, "holy-priest"),
        member(&bare, "holy-priest")
    );
    assert_ne!(
        member(&assigned, "restoration-druid"),
        member(&bare, "restoration-druid")
    );
    assert_eq!(tank_healing(&assigned).len(), 1);
}

#[test]
fn a_tank_healing_assignment_names_a_healer_and_a_tank() {
    let refused = |mut input: Value, list: Value| {
        input["assignments"] = json!({ "tankHealers": list });
        match simulate_raid(&serde_json::to_vec(&input).unwrap(), Rules::Forever, false) {
            Err(RaidError::Invalid(reason)) => reason,
            other => panic!("{:?}", other.map(|_| ())),
        }
    };
    let input = healed_tank(&["holy-priest"], &[false], 20);
    assert!(refused(input.clone(), json!([{ "from": 0, "to": 1 }])).contains("no heals"));
    assert!(refused(input.clone(), json!([{ "from": 1, "to": 1 }])).contains("itself"));
    assert!(refused(input.clone(), json!([{ "from": 1, "to": 9 }])).contains("no member"));
    assert!(refused(
        input.clone(),
        json!([{ "from": 1, "to": 0 }, { "from": 1, "to": 0 }])
    )
    .contains("two members"));
    // A healer on a member no target swings at: both are named.
    let untanked = healed_tank(&["holy-priest", "restoration-druid"], &[false, false], 20);
    let reason = refused(untanked, json!([{ "from": 1, "to": 2 }]));
    assert!(
        reason.contains("holy-priest") && reason.contains("restoration-druid"),
        "{reason}"
    );
}

#[test]
fn each_members_target_auras_are_in_the_summary() {
    let report = run(&reference_raid());
    let listed = report["mythicsim"]["targetAuras"].as_array().unwrap();
    assert_eq!(listed.len(), 20);
    let indexes: Vec<u64> = listed
        .iter()
        .map(|entry| entry["raidIndex"].as_u64().unwrap())
        .collect();
    assert_eq!(indexes, (0..20).collect::<Vec<u64>>());
    // The first member's list is the result's own encounter metrics, untouched.
    assert_eq!(
        listed[0]["auras"],
        report["result"]["encounterMetrics"]["targets"][0]["auras"]
    );
    let uptime = |entry: &Value, spell: i64| {
        entry["auras"]
            .as_array()
            .unwrap()
            .iter()
            .find(|aura| aura["id"]["spellId"] == json!(spell))
            .map_or(0.0, |aura| aura["uptimeSecondsAvg"].as_f64().unwrap())
    };
    // The feral druid's Faerie Fire and a warlock's Curse of the Elements are on every member's
    // target in about the length of the fight, which the raid shares; Sunder Armor's ranks are
    // the warriors'. A member sees the debuffs its own copy of the target carried.
    for entry in listed {
        assert!(uptime(entry, 1311680) > 60.0, "{}", entry["raidIndex"]);
    }
    // The members' lists are each member's own: not one list copied.
    assert!(listed
        .iter()
        .any(|entry| entry["auras"] != listed[0]["auras"]));
}

#[test]
fn the_two_tanks_two_healers_fixture_runs_with_swaps_and_assigned_healers() {
    let path = format!(
        "{}/fixtures/raid/two-tanks-two-healers.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let input: Value = serde_json::from_slice(&std::fs::read(path).expect("fixture")).unwrap();
    let report = run(&input);
    assert_eq!(tanking(&report), [true, true, false, false]);
    let healing = tank_healing(&report);
    assert_eq!(healing.len(), 2);
    for (entry, (from, to)) in healing.iter().zip([(2, 0), (3, 1)]) {
        assert_eq!(
            (entry["from"].as_u64(), entry["to"].as_u64()),
            (Some(from), Some(to))
        );
        assert!(entry["effective"].as_f64().unwrap() > 0.0, "{entry}");
    }
    // Each tank takes about half of the boss's swings, so a healer's share of it is a real one.
    for name in ["protection-warrior", "fury-protection-warrior"] {
        assert!(dtps(&report, name) > 0.0, "{name}");
    }
}
