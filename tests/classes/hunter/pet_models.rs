//! The Hunter pet models of Forever's rules (docs/mechanics.md): the tamed pet's own
//! attack power, weapon and inherited attack power, and a Summon Hawk hawk's swing and crit. The
//! reference rules keep Classic's pet and Go's hawk, which these tests pin beside them.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// A Beast Mastery Cat with Summon Hawk last in its priority list, 20 iterations with the first
/// one's debug log, against a level 63 target with 3731 armor.
const CASE: &str = "fixtures/prepull/hunter-turret-aimed-shot.request.json";

/// The client's level 60 creature auto attack dps, and what the pet model takes of it.
const CREATURE_DPS: f64 = 60.452_915_191_650_39;
const PET_OWN_ATTACK_POWER: f64 = 0.3 * 14.0 * CREATURE_DPS;
const PET_WEAPON_DPS: f64 = CREATURE_DPS - PET_OWN_ATTACK_POWER / 14.0;

fn request() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(CASE);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn prepared(rules: Rules, iterations: u64, debug: bool) -> Value {
    let mut request = request();
    request["simOptions"]["iterations"] = json!(iterations);
    request["simOptions"]["debugFirstIteration"] = json!(debug);
    prepare_json_with(request.to_string().as_bytes(), "rust", rules).unwrap()
}

fn run(prepared: &Value) -> Value {
    let prepared: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    assert_eq!(check_prepared(&prepared), Ok(()));
    simulate_prepared(&prepared).unwrap().result
}

fn hawk_effect(prepared: &mut Value) -> &mut Value {
    prepared["effects"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|effect| effect["kind"] == "summon_hawk")
        .expect("a Summon Hawk effect")
}

/// More ranged attack power on the owner: its stats, and every stat aura combination that
/// replaces them while an aura is up.
fn add_ranged_attack_power(prepared: &mut Value, extra: f64) {
    let stats = &mut prepared["player"]["stats"];
    stats["RangedAttackPower"] = json!(stats["RangedAttackPower"].as_f64().unwrap() + extra);
    for effect in prepared["effects"].as_array_mut().unwrap() {
        if effect["kind"] != "stat_auras" {
            continue;
        }
        for combo in effect["combos"].as_array_mut().unwrap() {
            let power = combo["RangedAttackPower"].as_f64().unwrap();
            combo["RangedAttackPower"] = json!(power + extra);
        }
    }
}

/// The numbers after `key` on a debug line: "MAP: 448.2, RAP: 100.0".
fn field(line: &str, key: &str) -> f64 {
    let rest = line
        .split(key)
        .nth(1)
        .unwrap_or_else(|| panic!("{key} in {line}"));
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == ' '))
        .unwrap_or(rest.len());
    rest[..end].trim().parse().unwrap()
}

/// The debug lines of a unit's damage calculations for an action, first fight.
fn debug_lines<'a>(logs: &'a str, unit: &str, action: &str) -> Vec<&'a str> {
    logs.lines()
        .filter(|line| {
            line.contains(unit) && line.contains(action) && line.contains("[DEBUG] MAP:")
        })
        .collect()
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// Forever's pet: 4.2 a point of the creature dps as its own attack power with nothing from
/// Strength, a weapon of the rest of the creature dps (42.3 a second, a fifth either side), and a
/// tenth of the owner's higher attack power. The reference's is Classic's: 136 Strength at two
/// attack power each less 20, an 18.17 to 27.66 a second weapon and nothing of its owner's.
#[test]
fn the_tamed_pet_is_the_creature_dps_and_a_tenth_of_its_owners_attack_power() {
    let forever = prepared(Rules::Forever, 1, false);
    let pet = &forever["pets"][0];
    let main_hand = &pet["melee"]["main_hand"];
    let speed = main_hand["swing_speed"].as_f64().unwrap();
    assert_eq!(speed, 1.2, "the request's Faster Attack");
    let low = main_hand["base_damage_min"].as_f64().unwrap();
    let high = main_hand["base_damage_max"].as_f64().unwrap();
    assert!((low - PET_WEAPON_DPS * 0.8 * speed).abs() < 1e-9, "{low}");
    assert!((high - PET_WEAPON_DPS * 1.2 * speed).abs() < 1e-9, "{high}");

    // The owner's attack power at the reset: the ranged one is higher for this Agility hunter.
    let stats = &forever["player"]["stats"];
    let (melee, ranged) = (
        stats["AttackPower"].as_f64().unwrap(),
        stats["RangedAttackPower"].as_f64().unwrap(),
    );
    assert!(ranged > melee);
    let inherited = pet["inheritance"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|term| term["pet"] == "AttackPower")
        .collect::<Vec<_>>();
    assert_eq!(
        inherited,
        [&json!({"owner": "RangedAttackPower", "pet": "AttackPower", "coefficient": 0.1})]
    );
    let pet_stats = &pet["stats"];
    assert_eq!(pet_stats["Strength"].as_f64().unwrap_or(0.0), 0.0);
    let attack_power = pet_stats["AttackPower"].as_f64().unwrap();
    assert!(
        (attack_power - (PET_OWN_ATTACK_POWER + 0.1 * ranged)).abs() < 1e-6,
        "{attack_power}"
    );

    let reference = prepared(Rules::GoReference, 1, false);
    let pet = &reference["pets"][0];
    let main_hand = &pet["melee"]["main_hand"];
    assert!((main_hand["base_damage_min"].as_f64().unwrap() - 18.17 * speed).abs() < 1e-9);
    assert!((main_hand["base_damage_max"].as_f64().unwrap() - 27.66 * speed).abs() < 1e-9);
    assert_eq!(pet["stats"]["Strength"].as_f64(), Some(136.0));
    assert!(pet["inheritance"]
        .as_array()
        .unwrap()
        .iter()
        .all(|term| term["pet"] != "AttackPower"));
}

/// A white swing's damage before armor is a roll of the weapon plus the pet's attack power over
/// 14 a second of its swing: every Cat swing of the first fight lies in that range at the attack
/// power its own debug line shows, and the rolls cover the range.
#[test]
fn a_pet_swing_is_a_weapon_roll_plus_its_attack_power_over_fourteen() {
    let prepared = prepared(Rules::Forever, 1, true);
    let main_hand = &prepared["pets"][0]["melee"]["main_hand"];
    let (low, high, speed) = (
        main_hand["base_damage_min"].as_f64().unwrap(),
        main_hand["base_damage_max"].as_f64().unwrap(),
        main_hand["swing_speed"].as_f64().unwrap(),
    );
    let logs = run(&prepared)["logs"].as_str().unwrap().to_string();
    let lines = debug_lines(&logs, " - Cat]", "{OtherID: 3, Tag: 1}");
    assert!(lines.len() > 50, "{} swings", lines.len());
    let mut rolls = Vec::new();
    for line in &lines {
        let attack_power = field(line, "MAP:");
        assert!(
            attack_power >= PET_OWN_ATTACK_POWER,
            "own attack power in {line}"
        );
        let fixed = attack_power / 14.0 * speed;
        let base = field(line, "BaseDamage:");
        // The line rounds to a tenth.
        assert!(
            base >= low + fixed - 0.06 && base <= high + fixed + 0.06,
            "{base} outside {} to {} in {line}",
            low + fixed,
            high + fixed
        );
        rolls.push((base - fixed - low) / (high - low));
    }
    let (lowest, highest) = rolls
        .iter()
        .fold((1.0f64, 0.0f64), |(lo, hi), r| (lo.min(*r), hi.max(*r)));
    assert!(
        lowest < 0.2 && highest > 0.8,
        "rolls span {lowest} to {highest}"
    );
    assert!(
        (mean(&rolls) - 0.5).abs() < 0.1,
        "mean roll {}",
        mean(&rolls)
    );
}

/// The hawk of the reference swings for the same 0.35 of the dive bomb's base every time, 37.8,
/// whatever the owner's ranged attack power.
#[test]
fn the_reference_hawk_swings_a_fixed_share_of_the_base() {
    let prepared = prepared(Rules::GoReference, 1, true);
    assert!(prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "summon_hawk")
        .unwrap()
        .get("guardian")
        .is_none());
    let logs = run(&prepared)["logs"].as_str().unwrap().to_string();
    let lines = debug_lines(&logs, "[hunter-turret (#1)]", "{SpellID: 1293527, Tag:");
    assert!(lines.len() > 20, "{} swings", lines.len());
    for line in lines {
        assert!((field(line, "BaseDamage:") - 37.8).abs() < 0.06, "{line}");
    }
}

/// Forever's hawk swing before armor is a roll of 10.7 a third either side plus 0.0156 a point of
/// the owner's ranged attack power (the logged 3.35 to 6.71 plus 0.0179, before Unleashed Fury
/// 5/5's 1.15): every swing lies in that range at the ranged attack power its debug line shows,
/// the rolls average the middle, and 1,000 more ranged attack power adds 15.6 to the average.
#[test]
fn a_hawk_swing_is_a_roll_plus_a_share_of_ranged_attack_power() {
    let mut value = prepared(Rules::Forever, 1, true);
    let guardian = hawk_effect(&mut value)["guardian"].clone();
    let average = CREATURE_DPS * 0.0815 * 2.5 / 1.15;
    let coefficient = 0.0179 / 1.15;
    assert!((guardian["swing_average"].as_f64().unwrap() - average).abs() < 1e-9);
    assert!((guardian["attack_power_coefficient"].as_f64().unwrap() - coefficient).abs() < 1e-12);

    let mut means = Vec::new();
    for extra in [0.0, 1000.0] {
        let mut value = value.clone();
        add_ranged_attack_power(&mut value, extra);
        let logs = run(&value)["logs"].as_str().unwrap().to_string();
        let lines = debug_lines(&logs, "[hunter-turret (#1)]", "{SpellID: 1293527, Tag:");
        assert!(lines.len() > 20, "{} swings", lines.len());
        let mut rolls = Vec::new();
        for line in lines {
            let attack_power = field(line, "RAP:");
            let base = field(line, "BaseDamage:");
            let roll = base - coefficient * attack_power;
            assert!(
                roll >= average * 2.0 / 3.0 - 0.06 && roll <= average * 4.0 / 3.0 + 0.06,
                "{roll} outside {} to {} in {line}",
                average * 2.0 / 3.0,
                average * 4.0 / 3.0
            );
            rolls.push(base);
        }
        means.push(mean(&rolls));
        assert!(
            rolls.iter().any(|base| base != &rolls[0]),
            "a roll each swing"
        );
    }
    // 15.6 more a swing for the 1,000 ranged attack power, within the rolls' noise.
    assert!(
        (means[1] - means[0] - coefficient * 1000.0).abs() < 1.5,
        "{means:?}"
    );
}

/// A hawk's crit chance is Ferocity's flat crit (10 at 5/5, on the spell) plus `crit_share` of
/// its owner's crit and offset, less the target's crit suppression: the rate over every hawk
/// swing of 300 fights is 5.2% with no share, near 17.9% at half and 30.6% at the owner's whole
/// crit. The reference's hawk never crits.
#[test]
fn a_hawk_crits_at_ferocity_plus_half_its_owners_crit() {
    let base = prepared(Rules::Forever, 300, false);
    let hawk_spell = base["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"] == json!({"spell_id": 1293527, "tag": 1}))
        .unwrap();
    let ferocity = hawk_spell["bonus_crit_percent"].as_f64().unwrap();
    assert_eq!(ferocity, 10.0);
    let owner_crit = base["player"]["stats"]["PhysicalCritPercent"]
        .as_f64()
        .unwrap();
    let suppression = base["melee"]["melee_crit_suppression"].as_f64().unwrap();

    let rate = |share: f64| -> f64 {
        let mut value = base.clone();
        hawk_effect(&mut value)["guardian"]["crit_share"] = json!(share);
        let result = run(&value);
        let actions = result["raidMetrics"]["parties"][0]["players"][0]["actions"]
            .as_array()
            .unwrap();
        let (mut hits, mut crits) = (0.0, 0.0);
        for action in actions {
            let id = &action["id"];
            if id["spellId"] == 1293527 && id.get("tag").is_some() {
                let target = &action["targets"][0];
                hits += target["hits"].as_f64().unwrap_or(0.0);
                crits += target["crits"].as_f64().unwrap_or(0.0);
            }
        }
        assert!(hits + crits > 1500.0, "{} swings", hits + crits);
        crits / (hits + crits)
    };
    for share in [0.0, 0.5, 1.0] {
        let expected = (ferocity + share * owner_crit) / 100.0 - suppression;
        let got = rate(share);
        assert!(
            (got - expected).abs() < 0.025,
            "share {share}: {got} against {expected}"
        );
    }
    // Half is the middle of the other two, a hawk's crit being linear in the share.
    let middle = (rate(0.0) + rate(1.0)) / 2.0;
    assert!((rate(0.5) - middle).abs() < 0.02);

    // The reference's hawk swing has no crit roll.
    let reference = prepared(Rules::GoReference, 100, false);
    let result = run(&reference);
    for action in result["raidMetrics"]["parties"][0]["players"][0]["actions"]
        .as_array()
        .unwrap()
    {
        let id = &action["id"];
        if id["spellId"] == 1293527 && id.get("tag").is_some() {
            assert_eq!(action["targets"][0]["crits"].as_f64().unwrap_or(0.0), 0.0);
        }
    }
}
