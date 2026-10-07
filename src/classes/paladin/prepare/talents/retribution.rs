//! Go `sim/paladin/talents_retribution.go`.

use std::rc::Rc;

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::character::constants::CHARACTER_LEVEL;
use crate::prepare::dbcenums;
use crate::prepare::sim::{AuraConfig, Sim, UnitId, NEVER_EXPIRES};
use crate::prepare::spell::{school, ProcMask, SpellConfig, SpellFlag};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::Stat;

use super::super::masks;
use super::super::spell_data::spell_data;
use super::super::util::{new_enemy_aura_array, spell_action, tagged_action};
use super::super::Paladin;
use super::{apply_weapon_specialization, millis, new_passive_movement_speed_aura};
use crate::prepare::aura_helpers::PseudoStatField;

/// Go `vindicationProcChance`: the Forever beta client states the chance as 100%.
const VINDICATION_PROC_CHANCE: f64 = 1.0;

/// Go `echoOfCommandID` and its siblings: the Echoes Twist of Light names, one per seal.
const ECHO_IDS: [i32; 4] = [1311703, 1311701, 1311704, 1311705];

impl Paladin {
    /// Go `registerRetributionTalents`.
    pub(in super::super) fn register_retribution_talents(&mut self, sim: &mut Sim, unit: UnitId) {
        // Tier 1
        self.apply_deflection(sim, unit);
        self.apply_benediction(sim, unit);

        // Tier 2
        self.apply_improved_judgement(sim, unit);
        self.apply_holy_conduit(sim, unit);
        self.apply_conviction(sim, unit);

        // Tier 3
        self.apply_vindication(sim, unit);
        self.apply_sanctified_judgement(sim, unit);
        // Seal of Command registered in registerTalentSpells
        self.apply_pursuit_of_justice(sim, unit);

        // Tier 4
        self.apply_eye_for_an_eye(sim, unit);
        self.apply_sacred_arbiter(sim, unit);

        // Tier 5
        self.apply_two_handed_weapon_specialization(sim, unit);
        self.apply_vengeance(sim, unit);

        // Tier 6
        self.apply_champion_of_the_light(sim, unit);
        self.apply_instrument_of_law(sim, unit);

        // Tier 7
        self.apply_twist_of_light(sim, unit);
    }

    /// Deflection: increases your Parry chance by 1/2/3/4/5%.
    fn apply_deflection(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("deflection");
        if points == 0 {
            return;
        }
        sim.unit_mut(unit).pseudo_stats.base_parry_chance +=
            spell_data().deflection.fraction_at(points);
    }

    /// Benediction: reduces the Mana cost of all instant cast spells and abilities.
    fn apply_benediction(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("benediction");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::BENEDICTION,
                kind: SpellModType::PowerCostPctAdd,
                float_value: spell_data().benediction.fraction_at(points),
                ..SpellModConfig::default()
            },
        );
    }

    /// Improved Judgement: decreases the cooldown of your Judgement ability by 1/2 sec.
    fn apply_improved_judgement(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("improved_judgement");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::JUDGEMENT,
                kind: SpellModType::CooldownFlat,
                time_value: millis(spell_data().improved_judgement.value_at(points)),
                ..SpellModConfig::default()
            },
        );
    }

    /// Holy Conduit: reduces the mana cost of your Consecration, Holy Wrath, Exorcism, and
    /// Hammer of Wrath spells.
    fn apply_holy_conduit(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("holy_conduit");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::CONSECRATION
                    | masks::HOLY_WRATH
                    | masks::EXORCISM
                    | masks::HAMMER_OF_WRATH,
                kind: SpellModType::PowerCostPctAdd,
                float_value: spell_data().holy_conduit.fraction_at(points),
                ..SpellModConfig::default()
            },
        );
    }

    /// Conviction: improves your chance to get a critical strike with melee attacks.
    fn apply_conviction(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("conviction");
        if points == 0 {
            return;
        }
        sim.add_stat(
            unit,
            Stat::PhysicalCritPercent,
            spell_data().conviction.value_at(points),
        );
    }

    /// Vindication: gives your damaging melee attacks a chance to reduce the target's Attack
    /// Power and increase your Attack Power for 30 sec.
    fn apply_vindication(&mut self, sim: &mut Sim, unit: UnitId) {
        let talent_points = self.talents.i32("vindication");
        if talent_points == 0 {
            return;
        }

        let rank = spell_data().vindication_triggered.highest();
        let points = spell_data()
            .vindication
            .effect_at(1)
            .value_at(talent_points);
        // The tooltip's ${$m1/-3*$440667m1}: the trigger's -201 scaled by the points over three.
        let target_attack_power = rank
            .effect(dbcenums::A_MOD_ATTACK_POWER, 0)
            .average(CHARACTER_LEVEL)
            * points
            / 3.0;

        let label = sim.unit(unit).label.clone();
        new_enemy_aura_array(sim, |sim, target| {
            let aura = sim.get_or_register_aura(
                target,
                AuraConfig {
                    label: format!("Vindication{label}"),
                    action_id: Some(spell_action(rank.id)),
                    duration: rank.duration(),
                    ..AuraConfig::default()
                },
            );
            sim.attach_stat_buff(aura, Stat::AttackPower, target_attack_power)
        });

        let attack_power_dep =
            sim.new_dynamic_multiply_stat(unit, Stat::AttackPower, 1.0 + points / 100.0);
        let self_aura = sim.register_aura(
            unit,
            AuraConfig {
                label: format!("Vindication{label}"),
                action_id: Some(tagged_action(rank.id, 1)),
                duration: rank.duration(),
                ..AuraConfig::default()
            },
        );
        sim.attach_stat_dependency(self_aura, attack_power_dep);

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: format!("Vindication - Trigger{label}"),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                proc_mask: ProcMask::MELEE,
                outcome: HitOutcome::LANDED,
                proc_chance: VINDICATION_PROC_CHANCE,
                ..ProcTrigger::default()
            },
        );
    }

    /// Sanctified Judgement: gives your Judgement ability a chance to return a share of the
    /// Mana cost of the judged seal.
    fn apply_sanctified_judgement(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("sanctified_judgement");
        if points == 0 {
            return;
        }
        let label = sim.unit(unit).label.clone();
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: format!("Sanctified Judgement{label}"),
                callback: CallbackMask::ON_CAST_COMPLETE,
                class_spell_mask: masks::JUDGEMENT,
                proc_chance: spell_data()
                    .sanctified_judgement
                    .effect_at(1)
                    .fraction_at(points),
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    /// Eye for an Eye: all critical strikes against you cause a share of the damage taken to
    /// the attacker as well.
    fn apply_eye_for_an_eye(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("eye_for_an_eye");
        if points == 0 {
            return;
        }
        let rank = spell_data().eye_for_an_eye.highest();

        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::HOLY,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::BINARY | SpellFlag::PASSIVE_SPELL | SpellFlag::IGNORE_MODIFIERS,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                ..SpellConfig::default()
            },
        );

        let label = sim.unit(unit).label.clone();
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: format!("Eye for an Eye{label}"),
                callback: CallbackMask::ON_SPELL_HIT_TAKEN,
                outcome: HitOutcome::CRIT,
                require_damage_dealt: true,
                ..ProcTrigger::default()
            },
        );
    }

    /// Pursuit of Justice: increases movement speed and mounted movement speed. This does not
    /// stack with other movement speed increasing effects.
    fn apply_pursuit_of_justice(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("pursuit_of_justice");
        if points == 0 {
            return;
        }
        let rank = spell_data().pursuit_of_justice.highest();
        new_passive_movement_speed_aura(
            sim,
            unit,
            "Pursuit of Justice",
            spell_action(rank.id),
            spell_data()
                .pursuit_of_justice
                .effect(dbcenums::A_MOD_INCREASE_SPEED, 0)
                .fraction_at(points),
        );
    }

    /// Sacred Arbiter: increases the damage of your Holy Strike ability and causes it to
    /// refresh all Judgement effects on the target.
    fn apply_sacred_arbiter(&mut self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("sacred_arbiter") {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::HOLY_STRIKE,
                kind: SpellModType::DamageDoneFlat,
                float_value: spell_data().sacred_arbiter.fraction_at(1),
                ..SpellModConfig::default()
            },
        );

        let label = sim.unit(unit).label.clone();
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: format!("Sacred Arbiter{label}"),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                class_spell_mask: masks::HOLY_STRIKE,
                outcome: HitOutcome::LANDED,
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    /// Two-Handed Weapon Specialization: increases the damage you deal with two-handed melee
    /// weapons.
    fn apply_two_handed_weapon_specialization(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("two_handed_weapon_specialization");
        if points == 0 {
            return;
        }
        let two_hand = self.main_hand_is_two_hand(sim, unit);
        apply_weapon_specialization(
            sim,
            unit,
            spell_data()
                .two_handed_weapon_specialization
                .fraction_at(points),
            two_hand,
        );
    }

    /// Vengeance: increases your Physical and Holy damage dealt after landing a non-periodic
    /// critical strike. Stacks up to 3 times.
    fn apply_vengeance(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("vengeance");
        if points == 0 {
            return;
        }
        let rank = spell_data().vengeance_triggered.highest();
        let per_stack = spell_data().vengeance.fraction_at(points);

        // 20050 is damage done (A79), which never raises a heal.
        let damage_mod = sim.add_dynamic_mod(
            unit,
            SpellModConfig {
                school: school::HOLY | school::PHYSICAL,
                proc_mask: ProcMask(!ProcMask::SPELL_HEALING.0),
                kind: SpellModType::DamageDonePct,
                float_value: per_stack,
                ..SpellModConfig::default()
            },
        );

        let label = sim.unit(unit).label.clone();
        sim.register_aura(
            unit,
            AuraConfig {
                label: format!("Vengeance{label}"),
                action_id: Some(spell_action(rank.id)),
                duration: rank.duration(),
                max_stacks: i32::from(rank.max_stack),
                on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.activate_spell_mod(damage_mod)
                })),
                on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.deactivate_spell_mod(damage_mod)
                })),
                on_stacks_change: Some(Rc::new(move |sim: &mut Sim, _, _, new_stacks| {
                    sim.update_spell_mod_float_value(damage_mod, per_stack * f64::from(new_stacks))
                })),
                ..AuraConfig::default()
            },
        );

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: format!("Vengeance - Trigger{label}"),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                outcome: HitOutcome::CRIT,
                // 20049 carries the bit: seal crits count.
                can_proc_from_procs: true,
                ..ProcTrigger::default()
            },
        );
    }

    /// Champion of the Light: increases your spell damage by up to 20/40/60% of your
    /// Intellect.
    fn apply_champion_of_the_light(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("champion_of_the_light");
        if points == 0 {
            return;
        }
        let share = spell_data()
            .champion_of_the_light
            .effect(dbcenums::A_MOD_SPELL_DAMAGE_OF_STAT_PERCENT, 126)
            .fraction_at(points);
        sim.unit_mut(unit)
            .sdm
            .add_stat_dependency(Stat::Intellect, Stat::SpellDamage, share);
    }

    /// Instrument of Law: reduces the cast time of your Hammer of Wrath, and reduces all
    /// threat you generate while Righteous Fury is not active.
    fn apply_instrument_of_law(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("instrument_of_law");
        if points == 0 {
            return;
        }
        let rank = spell_data().instrument_of_law.highest();

        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::HAMMER_OF_WRATH,
                kind: SpellModType::CastTimeFlat,
                time_value: millis(spell_data().instrument_of_law.effect_at(1).value_at(points)),
                ..SpellModConfig::default()
            },
        );

        // The client states the threat reduction as a positive number Righteous Fury zeroes.
        let label = sim.unit(unit).label.clone();
        let threat = sim.register_aura(
            unit,
            AuraConfig {
                label: format!("Instrument of Law{label}"),
                action_id: Some(spell_action(rank.id)),
                ..AuraConfig::default()
            },
        );
        sim.attach_multiplicative_pseudo_stat_buff(
            threat,
            PseudoStatField::ThreatMultiplier,
            1.0 - spell_data()
                .instrument_of_law
                .effect(dbcenums::A_MOD_THREAT, 127)
                .fraction_at(points),
        );
        sim.make_permanent(threat);

        sim.on_spell_registered(
            unit,
            Rc::new(move |sim: &mut Sim, spell| {
                if sim.spell(spell).matches(masks::RIGHTEOUS_FURY) {
                    if let Some(buff) = sim.spell(spell).related_self_buff {
                        sim.apply_on_gain(
                            buff,
                            Rc::new(move |sim: &mut Sim, _| sim.deactivate(threat)),
                        );
                        sim.apply_on_expire(
                            buff,
                            Rc::new(move |sim: &mut Sim, _| sim.activate(threat)),
                        );
                    }
                }
            }),
        );
    }

    /// Twist of Light: reduces the mana cost of your Seal spells by 20%. When you replace a
    /// seal with a different one, gain an Echo; your next melee attack applies the replaced
    /// seal's effects.
    fn apply_twist_of_light(&mut self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("twist_of_light") {
            return;
        }

        // The discount is an A_ADD_PCT_MODIFIER on the cost, so it joins the additive bucket
        // the way Swift Judgement's does.
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL_SEALS,
                kind: SpellModType::PowerCostPctAdd,
                float_value: spell_data()
                    .twist_of_light
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_COST)
                    .fraction_at(1),
                ..SpellModConfig::default()
            },
        );

        let label = sim.unit(unit).label.clone();
        for id in ECHO_IDS {
            sim.register_aura(
                unit,
                AuraConfig {
                    label: format!("Echo ({id}){label}"),
                    action_id: Some(spell_action(id)),
                    duration: NEVER_EXPIRES,
                    ..AuraConfig::default()
                },
            );
        }

        // One permanent trigger consumes every Echo that is up, in a fixed order.
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: format!("Twist of Light{label}"),
                ..AuraConfig::default()
            },
        );
        sim.attach_proc_trigger(
            aura,
            &ProcTrigger {
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                proc_mask: ProcMask::MELEE_WHITE_HIT,
                outcome: HitOutcome::LANDED,
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
        sim.make_permanent(aura);
    }
}
