//! The target's swing at a tanking player, on a warrior built through the same construction Go's
//! tests use, with the defenses a tank has set the way a class does.

use super::*;
use crate::contracts::request::{Message, Request};
use crate::prepare::agent::{ClassSpellName, PrepAgent};
use crate::prepare::attack::{AutoAttackOptions, Weapon};
use crate::prepare::aura_helpers::AbsorptionAuraConfig;
use crate::prepare::sim::{AuraConfig, Sim, UnitType, NEVER_EXPIRES};
use crate::prepare::stat_auras::{character_stat_auras, stat_auras_effect_reading, stat_layout};
use std::rc::Rc;

struct FakeAgent {
    talents: Message,
}

impl PrepAgent for FakeAgent {
    fn talents(&self) -> &Message {
        &self.talents
    }
    fn class_spells(&self) -> &'static [ClassSpellName] {
        &[]
    }
}

/// A warrior that parries and blocks and has base avoidance of its own.
fn factory(sim: &mut Sim, unit: UnitId, _player: &Message) -> Result<Box<dyn PrepAgent>, Refusal> {
    let pseudo = &mut sim.unit_mut(unit).pseudo_stats;
    pseudo.can_parry = true;
    pseudo.can_block = true;
    pseudo.base_dodge_chance = 0.05;
    pseudo.base_parry_chance = 0.05;
    pseudo.base_block_chance = 0.05;
    sim.enable_auto_attacks(
        unit,
        AutoAttackOptions {
            main_hand: Weapon::unarmed(),
            auto_swing_melee: true,
            ..AutoAttackOptions::default()
        },
    );
    Ok(Box::new(FakeAgent {
        talents: Message::empty("proto.WarriorTalents"),
    }))
}

fn request(race: &str, tanks: bool, targets: usize, target: &str) -> Request {
    let tank_list = if tanks {
        r#""tanks":[{"type":"Player","index":0}],"#
    } else {
        ""
    };
    let tank_index = if tanks { r#""tankIndex":0,"# } else { "" };
    let one = format!(
        r#"{{"level":63,"swingSpeed":2,"minBaseDamage":3000,"damageSpread":0.3333,"canCrush":true,{tank_index}{target}"stats":[]}}"#
    );
    let all = vec![one; targets].join(",");
    let json = format!(
        r#"{{"simOptions":{{"iterations":1,"randomSeed":"100"}},
        "raid":{{{tank_list}"parties":[{{"players":[{{"name":"Tank","class":"ClassWarrior",
          "race":"{race}","buffs":{{}},"consumables":{{}}}}]}}]}},
        "encounter":{{"duration":180,"targets":[{all}]}}}}"#
    );
    Request::from_json(json.as_bytes()).expect("a valid request")
}

fn environment_from(request: &Request) -> Environment {
    match Environment::new(request.message(), factory) {
        Ok(env) => env,
        Err(refusal) => panic!("{refusal}"),
    }
}

fn tank(race: &str) -> Environment {
    environment_from(&request(race, true, 1, ""))
}

fn first_target(env: &Environment) -> UnitId {
    env.encounter.targets[0]
}

fn values(env: &Environment) -> Enemy {
    enemy_values(env, first_target(env)).expect("a physical swing")
}

fn close(left: f64, right: f64) {
    assert!(
        (left - right).abs() < 1e-12,
        "{left} is not within 1e-12 of {right}"
    );
}

#[test]
fn a_tank_registers_reduced_avoidance_and_the_pushback_trigger_in_goes_order() {
    let env = tank("RaceHuman");
    let sim = &env.sim;
    let labels: Vec<&str> = sim
        .unit(env.player)
        .auras
        .iter()
        .map(|aura| sim.aura(*aura).label.as_str())
        .collect();
    let position = |label: &str| labels.iter().position(|l| *l == label).expect(label);
    // Finalize registers the two before the unit finalizes and so before Parry Haste and
    // Movement.
    assert!(position("Chance of Death") < position("Reduced avoidance"));
    assert_eq!(
        position("Pushback trigger"),
        position("Reduced avoidance") + 1
    );
    assert!(position("Pushback trigger") < position("Parry Haste"));
    assert!(position("Parry Haste") < position("Movement"));

    let reduced = sim.get_aura(env.player, "Reduced avoidance").unwrap();
    assert_eq!(
        sim.character(env.player).hardcast_avoidance_aura,
        Some(reduced)
    );
    let reduced = sim.aura(reduced);
    assert_eq!(reduced.tag, "Reduced Avoidance");
    assert_eq!(reduced.duration, NEVER_EXPIRES);
    assert!(!reduced.active);
    assert_eq!(reduced.callback_names(), ["on_gain", "on_expire"]);

    let pushback = sim.aura(sim.get_aura(env.player, "Pushback trigger").unwrap());
    assert!(pushback.active);
    assert_eq!(
        pushback.callback_names(),
        ["on_reset", "on_spell_hit_taken"]
    );
}

#[test]
fn a_player_nothing_swings_at_has_neither() {
    let env = environment_from(&request("RaceHuman", false, 1, ""));
    assert!(!env.tanking());
    assert!(env.sim.get_aura(env.player, "Reduced avoidance").is_none());
    assert!(env.sim.get_aura(env.player, "Pushback trigger").is_none());
    assert!(env
        .sim
        .character(env.player)
        .hardcast_avoidance_aura
        .is_none());
}

#[test]
fn every_copy_of_the_target_swings_at_the_tank() {
    let env = environment_from(&request("RaceHuman", true, 3, ""));
    assert!(env.tanking());
    for target in &env.encounter.targets {
        assert_eq!(env.sim.unit(*target).current_target, Some(env.player));
    }
    let first = encode(&values(&env));
    for target in &env.encounter.targets {
        let copy = enemy_values(&env, *target).expect("a physical swing");
        assert_eq!(encode(&copy), first);
    }
}

#[test]
fn the_swing_reads_the_defenders_table_and_stats() {
    let env = tank("RaceHuman");
    let enemy = values(&env);
    assert_eq!(enemy.school, school::PHYSICAL);
    assert_eq!(enemy.swing_speed, 2.0);
    assert_eq!(enemy.base_damage_min, 3000.0);
    assert_eq!(enemy.damage_spread, 0.3333);
    assert_eq!(enemy.attack_power_coefficient, 0.00052);
    assert_eq!(enemy.melee_haste_multiplier, 1.0);
    assert_eq!(enemy.attacker_multiplier, 1.0);
    assert_eq!(enemy.threat_multiplier, 1.0);
    assert_eq!(enemy.rolls.len(), 1);
    let rolls = &enemy.rolls[0];
    // A level 63 attacker against a level 60 defender: 4.4% to miss, the defender's 5% base
    // dodge less 0.6%, and 5% base parry and block on top of 4.4%.
    close(rolls.miss_chance, 0.044);
    close(rolls.dodge_chance, 0.044);
    close(rolls.parry_chance, 0.094);
    close(rolls.block_chance, 0.094);
    // The target's 5.6% crit, and a boss crushes for 15%.
    close(rolls.crit_chance, 0.056);
    close(rolls.crush_chance, 0.15);
    assert_eq!(rolls.block_reduction, 0.0);
    assert_eq!(rolls.target_multiplier, 1.0);
    assert_eq!(rolls.school_damage_taken_multiplier, Some(1.0));
    assert_eq!(rolls.table_damage_taken_multiplier, Some(1.0));
    // Armor reduces physical damage by armor / (armor + 400 + 85 x the attacker's level),
    // never below a quarter.
    let armor = env.sim.stat(env.player, Stat::Armor);
    assert!(armor > 0.0);
    close(
        rolls.armor_multiplier,
        1.0 - armor / (armor + 400.0 + 85.0 * 63.0),
    );
    assert!(enemy.changing_auras.is_empty());
}

#[test]
fn reduced_avoidance_takes_dodge_parry_and_block_away() {
    let mut env = tank("RaceHuman");
    let reduced = env
        .sim
        .character(env.player)
        .hardcast_avoidance_aura
        .unwrap();
    let before = values(&env).rolls[0].clone();
    env.sim.activate(reduced);
    assert!(env.sim.unit(env.player).pseudo_stats.stunned);
    assert!(!env.sim.unit(env.player).pseudo_stats.incapacitated);
    let during = values(&env).rolls[0].clone();
    assert_eq!(
        (
            during.dodge_chance,
            during.parry_chance,
            during.block_chance
        ),
        (0.0, 0.0, 0.0)
    );
    assert_eq!(during.miss_chance, before.miss_chance);
    assert_eq!(during.crit_chance, before.crit_chance);
    env.sim.deactivate(reduced);
    assert!(!env.sim.unit(env.player).pseudo_stats.stunned);
    assert_eq!(values(&env).rolls[0].dodge_chance, before.dodge_chance);
}

#[test]
fn only_the_auras_that_change_the_swing_are_listed_and_by_how() {
    // The dwarf's Stoneform takes physical damage down, and a hardcast's reduced avoidance
    // changes the rolls themselves.
    let env = tank("RaceDwarf");
    let found = enemy_at_reset(&env, &[]).expect("a physical swing");
    assert_eq!(found.school_damage_taken, ["player:Stoneform"]);
    assert_eq!(found.changing, ["player:Reduced avoidance"]);
    assert!(found.damage_taken.is_empty());
    assert!(found.speed.is_empty());
    assert!(found.attack_power.is_empty());
}

#[test]
fn a_stat_aura_is_read_with_and_without_it_and_with_a_hardcast() {
    let env = tank("RaceOrc");
    let layout = stat_layout(&env);
    let labels = character_stat_auras(&env);
    assert_eq!(labels, ["Blood Fury"]);
    let mut combos = EnemyCombos::new(&env, &layout).expect("a physical swing");
    let mut read =
        |mask: usize, fresh: &mut Environment, exact: bool| combos.read(mask, fresh, exact);
    stat_auras_effect_reading(&env, &layout, Some(&mut read)).expect("a physical swing");
    assert_eq!(combos.rolls.len(), 2);
    assert_eq!(combos.reduced_rolls.len(), 2);
    assert_eq!(combos.changed, 0);
    assert_eq!(combos.reduced_changed, 0);
    // Blood Fury leaves the damage taken multiplier alone.
    assert_eq!(combos.alone_damage_taken, [1.0]);
    for rolls in &combos.reduced_rolls {
        assert_eq!(rolls.dodge_chance, 0.0);
    }
    let mut unrepresented = Vec::new();
    let enemy = export_enemy(&env, &labels, Some(combos), &[], &mut unrepresented)
        .expect("a physical swing");
    assert!(unrepresented.is_empty(), "{unrepresented:?}");
    assert_eq!(enemy.rolls.len(), 2);
    assert_eq!(enemy.reduced_avoidance_rolls.len(), 2);
}

#[test]
fn without_stat_auras_the_hardcast_is_read_in_a_simulation_of_its_own() {
    let env = tank("RaceHuman");
    let mut unrepresented = Vec::new();
    let enemy = export_enemy(&env, &[], None, &[], &mut unrepresented).expect("a physical swing");
    assert!(unrepresented.is_empty(), "{unrepresented:?}");
    assert_eq!(enemy.rolls.len(), 1);
    assert_eq!(enemy.reduced_avoidance_rolls.len(), 1);
    assert_eq!(enemy.reduced_avoidance_rolls[0].dodge_chance, 0.0);
    assert!(enemy.rolls[0].dodge_chance > 0.0);
}

#[test]
fn a_swing_that_is_not_physical_is_refused_not_approximated() {
    let env = environment_from(&request(
        "RaceHuman",
        true,
        1,
        r#""spellSchool":"SpellSchoolFire","#,
    ));
    let refusal = enemy_values(&env, first_target(&env)).expect_err("a fire swing");
    assert_eq!(refusal.code, "tanking");
}

#[test]
fn a_shield_is_a_modifier_that_acts_only_while_its_aura_is_up() {
    // The aura helper records the shield's aura beside the modifier it counts.
    let mut sim = Sim::new();
    let unit = sim.add_unit(crate::prepare::sim::Unit::new(
        UnitType::Player,
        "Shielded".to_string(),
    ));
    let shield = sim.new_damage_absorption_aura(
        unit,
        AbsorptionAuraConfig {
            aura: AuraConfig {
                label: "Shield".to_string(),
                ..AuraConfig::default()
            },
            shield_strength_calculator: Some(Rc::new(|_: &Sim, _| 100.0)),
            ..AbsorptionAuraConfig::default()
        },
    );
    assert_eq!(sim.unit(unit).absorption_auras, [shield.aura]);
    assert_eq!(sim.unit(unit).dynamic_damage_taken_modifiers, 1);

    // A hit at reset is changed by the shields whose aura is active then.
    let mut env = tank("RaceHuman");
    assert_eq!(acting_damage_taken_modifiers(&env), 0);
    let down = env.sim.get_aura(env.player, "Reduced avoidance").unwrap();
    let up = env.sim.get_aura(env.player, "Chance of Death").unwrap();
    assert!(env.sim.aura(up).active && !env.sim.aura(down).active);
    env.sim.unit_mut(env.player).absorption_auras = vec![down];
    assert_eq!(acting_damage_taken_modifiers(&env), 0);
    env.sim.unit_mut(env.player).absorption_auras = vec![down, up];
    assert_eq!(acting_damage_taken_modifiers(&env), 1);
}
