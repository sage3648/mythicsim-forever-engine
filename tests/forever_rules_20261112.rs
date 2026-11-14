//! Rules measured on combat logs of client 1.60.1.70862, under Forever's rules only (the Go
//! reference keeps its own): Flametongue Totem off auto attacks alone, an avoided Execute's rage,
//! Thunder Clap beside Sweeping Strikes and its attack power share, Shield Slam and Shield Bash
//! as main hand specials, Shield Slam's block value and threat, and rage from damage taken. The
//! live item rows read on 12 November 2026 are checked here too. docs/mechanics.md has the
//! evidence of each.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const FLAMETONGUE_TOTEM: i64 = 16387;
const WINDFURY_WEAPON_MAIN_HAND_STRIKE: i64 = 439440;
const SHIELD_SLAM: i64 = 23925;
const SHIELD_BASH: i64 = 1672;
/// The live Fury and Protection references' talents on the launch tree.
const FURY: &str = "30303213002-050530045120100231-2";
const PROTECTION: &str = "-05250035-250033120310021251";

fn request(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{name}.request.json"));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(200);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    request
}

/// A production Warrior request with the live reference's talents on the launch tree, which
/// only Forever's rules take; the Go reference keeps the fixture's own.
fn warrior(name: &str, talents: &str, rules: Rules) -> Value {
    let mut request = request(name);
    if rules == Rules::Forever {
        request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(talents);
    }
    prepared(&request, rules)
}

fn protection(rules: Rules) -> Value {
    warrior("production-protection-warrior", PROTECTION, rules)
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn run(prepared: &Value) -> Value {
    let prepared = serde_json::from_value(prepared.clone()).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["raidMetrics"]["parties"][0]["players"][0].clone()
}

fn effect<'a>(prepared: &'a Value, kind: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

fn effect_mut<'a>(prepared: &'a mut Value, kind: &str) -> &'a mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

/// The spell with this id and no tag, and its position in the player's book.
fn spell(prepared: &Value, spell_id: i64) -> (u64, &Value) {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, spell)| {
            let id = &spell["action_id"];
            id["spell_id"] == spell_id && id["tag"].as_i64().unwrap_or(0) == 0
        })
        .map(|(position, spell)| (position as u64, spell))
        .unwrap_or_else(|| panic!("no spell {spell_id}"))
}

fn rage_from_damage_taken(player: &Value) -> f64 {
    player["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|resource| {
            resource["id"]["otherId"] == "OtherActionDamageTaken"
                && resource["type"] == "ResourceTypeRage"
        })
        .map(|resource| resource["gain"].as_f64().unwrap_or(0.0))
        .sum()
}

fn dps(player: &Value) -> f64 {
    player["dps"]["avg"].as_f64().unwrap()
}

/// The production Enhancement request with a main hand Windfury Weapon and the shaman's own
/// Flametongue Totem kept up, and no Windfury Totem in the party.
fn enhancement_with_flametongue_totem() -> Value {
    let mut request = request("production-enhancement-shaman");
    request["raid"]["parties"][0]["buffs"]["windfuryTotem"] = json!(false);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    // The live Enhancement reference's talents on the launch tree.
    player["talentsString"] = json!("453003102-055030310051002251");
    player["enhancementShaman"]["options"]["classOptions"]["imbueMh"] = json!("WindfuryWeapon");
    let list = player["rotation"]["priorityList"].as_array_mut().unwrap();
    list.insert(
        0,
        json!({"action": {"castSpell": {"spellId": {"spellId": FLAMETONGUE_TOTEM}},
            "condition": {"not": {"val": {"auraIsActive":
                {"auraId": {"spellId": FLAMETONGUE_TOTEM}}}}}}}),
    );
    request
}

/// Flametongue Totem's trigger hears the main hand's auto attacks alone: Windfury Weapon's main
/// hand strike (439440) is not among its spells, and the totem's hit lands no more often than
/// the autos do.
#[test]
fn flametongue_totem_procs_off_autos_and_never_off_a_windfury_weapon_strike() {
    let request = enhancement_with_flametongue_totem();
    let forever = prepared(&request, Rules::Forever);
    let (strike, _) = spell(&forever, WINDFURY_WEAPON_MAIN_HAND_STRIKE);
    let triggers: Vec<u64> = effect(&forever, "flametongue_totem")["trigger_spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|position| position.as_u64().unwrap())
        .collect();
    assert!(!triggers.is_empty());
    assert!(!triggers.contains(&strike), "{triggers:?}");
    let spells = forever["player"]["spells"].as_array().unwrap();
    for position in &triggers {
        let mask = spells[*position as usize]["proc_mask"].to_string();
        assert!(mask.contains("ProcMaskMeleeMHAuto"), "{mask}");
    }

    let player = run(&forever);
    let landed = |spell_id: Option<i64>, other: Option<&str>, tag: i64| -> f64 {
        player["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|action| {
                let id = &action["id"];
                id["spellId"].as_i64() == spell_id
                    && id["otherId"].as_str() == other
                    && id["tag"].as_i64().unwrap_or(0) == tag
            })
            .flat_map(|action| action["targets"].as_array().unwrap().iter())
            .map(|target| {
                ["hits", "crits", "glances", "blocks"]
                    .iter()
                    .map(|outcome| target[*outcome].as_f64().unwrap_or(0.0))
                    .sum::<f64>()
            })
            .sum()
    };
    let strikes = landed(Some(WINDFURY_WEAPON_MAIN_HAND_STRIKE), None, 0);
    assert!(strikes > 0.0, "Windfury Weapon strikes");
    let autos = landed(None, Some("OtherActionAttack"), 1);
    let totem_hits: f64 = player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"].as_i64() == Some(16389))
        .flat_map(|action| action["targets"].as_array().unwrap().iter())
        .map(|target| {
            target["hits"].as_f64().unwrap_or(0.0) + target["crits"].as_f64().unwrap_or(0.0)
        })
        .sum();
    assert!(totem_hits > 0.0, "the totem's hit lands");
    assert!(
        totem_hits <= autos,
        "{totem_hits} totem hits for {autos} landed autos"
    );
}

#[test]
fn an_avoided_execute_keeps_its_extra_rage_under_forever_rules() {
    let forever = warrior("production-warrior", FURY, Rules::Forever);
    assert_eq!(effect(&forever, "execute")["extra_rage_on_landed"], true);
    let reference = warrior("production-warrior", FURY, Rules::GoReference);
    assert!(effect(&reference, "execute")
        .get("extra_rage_on_landed")
        .is_none());

    // The rule is what the flag carries: without it the same fight spends the extra rage
    // before the roll and refunds the cost's share, so the fights differ.
    let mut without = forever.clone();
    effect_mut(&mut without, "execute")
        .as_object_mut()
        .unwrap()
        .remove("extra_rage_on_landed");
    assert_ne!(dps(&run(&forever)), dps(&run(&without)));
}

#[test]
fn thunder_clap_takes_seven_percent_and_spends_no_sweeping_strikes_charge() {
    let forever = protection(Rules::Forever);
    let clap = effect(&forever, "thunder_clap");
    assert_eq!(clap["attack_power_share"], 0.07);
    assert_eq!(clap["no_sweeping_strikes"], true);
    let reference = protection(Rules::GoReference);
    let clap = effect(&reference, "thunder_clap");
    assert_eq!(clap["attack_power_share"], 0.0255);
    assert!(clap.get("no_sweeping_strikes").is_none());
}

#[test]
fn shield_slam_and_shield_bash_strike_with_the_main_hand() {
    let forever = protection(Rules::Forever);
    let reference = protection(Rules::GoReference);
    for spell_id in [SHIELD_SLAM, SHIELD_BASH] {
        assert_eq!(
            spell(&forever, spell_id).1["proc_mask"],
            json!(["ProcMaskMeleeMHSpecial"])
        );
        assert_eq!(
            spell(&reference, spell_id).1["proc_mask"],
            json!(["ProcMaskMeleeOHSpecial"])
        );
    }
    // Rank 4 at spell level 60 carries rank 1's measured 4.45 a spell level: 267.
    assert_eq!(spell(&forever, SHIELD_SLAM).1["flat_threat_bonus"], 267.0);
    assert_eq!(spell(&reference, SHIELD_SLAM).1["flat_threat_bonus"], 508.0);
    // Windfury Totem rolls on them since the hotfixes of 13 November 2026 ("activated by all
    // melee abilities even if it doesn't involve your main hand weapon"); it never did before.
    if let Some(totem) = forever["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "windfury_totem")
    {
        let triggers = totem["trigger_spells"].to_string();
        for spell_id in [SHIELD_SLAM, SHIELD_BASH] {
            let position = spell(&forever, spell_id).0;
            assert!(
                totem["trigger_spells"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|spell| spell.as_u64() == Some(position)),
                "{triggers}"
            );
        }
    }
}

#[test]
fn shield_slam_adds_twice_the_block_value() {
    let forever = protection(Rules::Forever);
    assert_eq!(effect(&forever, "shield_slam")["twice_block_value"], true);
    let reference = protection(Rules::GoReference);
    assert!(effect(&reference, "shield_slam")
        .get("twice_block_value")
        .is_none());

    let slam = |prepared: &Value| -> f64 {
        let player = run(prepared);
        let (mut damage, mut landed) = (0.0, 0.0);
        for action in player["actions"].as_array().unwrap() {
            if action["id"]["spellId"].as_i64() != Some(SHIELD_SLAM) {
                continue;
            }
            for target in action["targets"].as_array().unwrap() {
                damage += target["damage"].as_f64().unwrap_or(0.0);
                landed += target["hits"].as_f64().unwrap_or(0.0)
                    + target["crits"].as_f64().unwrap_or(0.0);
            }
        }
        assert!(landed > 0.0, "Shield Slam lands");
        damage / landed
    };
    let mut once = forever.clone();
    effect_mut(&mut once, "shield_slam")
        .as_object_mut()
        .unwrap()
        .remove("twice_block_value");
    assert!(slam(&forever) > slam(&once));
}

#[test]
fn rage_from_damage_taken_reads_the_attackers_level_not_the_tanks_health() {
    let forever = protection(Rules::Forever);
    // A level 63 boss: 2.5 * (1 - 0.40) / 243.64, a point of rage for about 162 damage.
    let per_damage = effect(&forever, "rage_bar")["damage_taken_rage_per_damage"]
        .as_f64()
        .unwrap();
    assert!((1.0 / per_damage - 162.43).abs() < 0.01, "{per_damage}");
    let reference = protection(Rules::GoReference);
    assert!(effect(&reference, "rage_bar")
        .get("damage_taken_rage_per_damage")
        .is_none());

    // The rate is the whole rule: a tenth of it gives less rage in the same fight, and the
    // tank's health no longer enters.
    let mut tenth = forever.clone();
    effect_mut(&mut tenth, "rage_bar")["damage_taken_rage_per_damage"] = json!(per_damage / 10.0);
    let full = rage_from_damage_taken(&run(&forever));
    assert!(full > 0.0);
    assert!(rage_from_damage_taken(&run(&tenth)) < full);
}

/// The live rows read from Wowhead's Forever tooltips on 12 November 2026.
#[test]
fn live_rows_of_12_november() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/forever-live-items.jsonl");
    let text = fs::read_to_string(path).unwrap();
    let stats = |id: i64| -> Value {
        text.lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .find(|row| row["id"] == id)
            .unwrap_or_else(|| panic!("no live row {id}"))["scaling_options"]["0"]["stats"]
            .clone()
    };
    // Counterattack Lodestone: "+30 Attack Power", melee and ranged, and no parry.
    assert_eq!(stats(18537), json!({"17": 30, "18": 30}));
    // Briarwood Reed: "by up to 15", doubled only in Marsh and Swamp areas.
    assert_eq!(stats(12930), json!({"4": 15, "5": 15}));
    // Royal Seal of Eldre'Thalas, the Warlock's and the Shaman's: "by up to 18".
    for id in [18467, 18471] {
        assert_eq!(stats(id), json!({"4": 18, "5": 18, "38": 15}));
    }
}
