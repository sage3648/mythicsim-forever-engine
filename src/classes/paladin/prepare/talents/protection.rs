//! Go `sim/paladin/talents_protection.go`.

use std::rc::Rc;

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger, PseudoStatField};
use crate::prepare::character::constants::DEFENSE_RATING_PER_DEFENSE_LEVEL;
use crate::prepare::dbcenums;
use crate::prepare::sim::{AuraConfig, Sim, UnitId, SECOND};
use crate::prepare::spell::ProcMask;
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::Stat;

use super::super::masks;
use super::super::spell_data::spell_data;
use super::super::util::spell_action;
use super::super::Paladin;
use super::{apply_weapon_specialization, millis};

impl Paladin {
    /// Go `registerProtectionTalents`.
    pub(in super::super) fn register_protection_talents(&mut self, sim: &mut Sim, unit: UnitId) {
        // Tier 1
        self.apply_toughness(sim, unit);
        self.apply_redoubt(sim, unit);

        // Tier 2
        self.apply_precision(sim, unit);
        // Guardian's Favor changes Blessing of Protection and Blessing of Freedom, which are
        // not modelled.
        self.apply_anticipation(sim, unit);

        // Tier 3
        // Improved Seal of Fury attaches to the seal's shield in seal_of_fury.go
        self.apply_improved_righteous_fury(sim, unit);
        self.apply_shield_specialization(sim, unit);
        self.apply_sacred_duty(sim, unit);

        // Tier 4
        // Swift Judgement registered in registerTalentSpells
        self.apply_one_handed_weapon_specialization(sim, unit);
        // Improved Hammer of Justice shortens a cooldown the sim does not model.

        // Tier 5
        // Templar's Bulwark registered in registerTalentSpells
        self.apply_reckoning(sim, unit);

        // Tier 6
        self.apply_iron_creed(sim, unit);

        // Tier 7
        // Holy Shield registered in registerTalentSpells
    }

    /// Toughness: increases your armor value from items by 2/4/6/8/10%.
    fn apply_toughness(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("toughness");
        if points == 0 {
            return;
        }
        sim.apply_equip_scaling(
            unit,
            Stat::Armor,
            spell_data()
                .toughness
                .effect(dbcenums::A_MOD_BASE_RESISTANCE_PCT, 1)
                .multiplier_at(points),
        );
    }

    /// Redoubt: damaging melee attacks against you have a chance to increase your chance to
    /// block. Lasts 10 sec or 5 blocks.
    fn apply_redoubt(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("redoubt");
        if points == 0 {
            return;
        }
        let rank = spell_data().redoubt_triggered.highest();
        let label = sim.unit(unit).label.clone();

        let redoubt = sim.register_aura(
            unit,
            AuraConfig {
                label: format!("Redoubt{label}"),
                action_id: Some(spell_action(rank.id)),
                duration: rank.duration(),
                max_stacks: i32::from(rank.proc_charges),
                ..AuraConfig::default()
            },
        );
        sim.attach_stat_buff(
            redoubt,
            Stat::BlockPercent,
            spell_data().redoubt.value_at(points),
        );
        sim.attach_proc_trigger(
            redoubt,
            &ProcTrigger {
                callback: CallbackMask::ON_SPELL_HIT_TAKEN,
                outcome: HitOutcome::BLOCK,
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: format!("Redoubt - Trigger{label}"),
                callback: CallbackMask::ON_SPELL_HIT_TAKEN,
                proc_mask: ProcMask::MELEE,
                outcome: HitOutcome::LANDED,
                require_damage_dealt: true,
                // Up with the hit that procs it, like Shield Specialization.
                trigger_immediately: true,
                // The row carries the rank 5 chance on every rank; the beta client's talent
                // curve is 2% a rank.
                proc_chance: 0.02 * f64::from(points),
                ..ProcTrigger::default()
            },
        );
    }

    /// Precision: improves your chance to hit by 1/2/3%, with melee weapons and spells alike.
    fn apply_precision(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("precision");
        if points == 0 {
            return;
        }
        sim.add_stat(
            unit,
            Stat::PhysicalHitPercent,
            spell_data()
                .precision
                .effect(dbcenums::A_MOD_HIT_CHANCE, 0)
                .value_at(points),
        );
        sim.add_stat(
            unit,
            Stat::SpellHitPercent,
            spell_data()
                .precision
                .effect(dbcenums::A_MOD_SPELL_HIT_CHANCE, 0)
                .value_at(points),
        );
    }

    /// Anticipation: increases your Defense Skill by 4/8/12/16/20.
    fn apply_anticipation(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("anticipation");
        if points == 0 {
            return;
        }
        sim.add_stat(
            unit,
            Stat::DefenseRating,
            spell_data().anticipation.value_at(points) * DEFENSE_RATING_PER_DEFENSE_LEVEL,
        );
    }

    /// Improved Righteous Fury: while Righteous Fury is active, all damage taken is reduced by
    /// 2/4/6%.
    fn apply_improved_righteous_fury(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("improved_righteous_fury");
        if points == 0 {
            return;
        }
        // The client states this as a negative percentage per rank: -2 / -4 / -6, so
        // multiplier_at gives 0.98 / 0.96 / 0.94 and the minus is never written here.
        let multiplier = spell_data()
            .improved_righteous_fury
            .effect(dbcenums::A_ADD_FLAT_MODIFIER, dbcenums::SPELLMOD_EFFECT2)
            .multiplier_at(points);

        sim.on_spell_registered(
            unit,
            Rc::new(move |sim: &mut Sim, spell| {
                if sim.spell(spell).matches(masks::RIGHTEOUS_FURY) {
                    if let Some(buff) = sim.spell(spell).related_self_buff {
                        sim.attach_multiplicative_pseudo_stat_buff(
                            buff,
                            PseudoStatField::DamageTakenMultiplier,
                            multiplier,
                        );
                    }
                }
            }),
        );
    }

    /// Shield Specialization: increases the amount of damage absorbed by your shield, and
    /// gives your blocks a chance to restore a share of your maximum Mana. May only occur once
    /// every 3 sec.
    fn apply_shield_specialization(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("shield_specialization");
        if points == 0 {
            return;
        }
        sim.unit_mut(unit).pseudo_stats.block_value_multiplier *= spell_data()
            .shield_specialization
            .effect(dbcenums::A_MOD_BLOCK_VALUE_PCT, 0)
            .multiplier_at(points);

        let label = sim.unit(unit).label.clone();
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: format!("Shield Specialization{label}"),
                callback: CallbackMask::ON_SPELL_HIT_TAKEN,
                outcome: HitOutcome::BLOCK,
                proc_chance: spell_data()
                    .shield_specialization
                    .effect_at(2)
                    .fraction_at(points),
                icd: 3 * SECOND,
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    /// Sacred Duty: increases your total Stamina and reduces the cooldown of your Templar's
    /// Bulwark spell.
    fn apply_sacred_duty(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("sacred_duty");
        if points == 0 {
            return;
        }
        sim.unit_mut(unit).sdm.multiply_stat(
            Stat::Stamina,
            spell_data()
                .sacred_duty
                .effect(dbcenums::A_MOD_TOTAL_STAT_PERCENTAGE, 0)
                .multiplier_at(points),
        );
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::TEMPLARS_BULWARK,
                kind: SpellModType::CooldownFlat,
                time_value: millis(spell_data().sacred_duty.effect_at(2).value_at(points)),
                ..SpellModConfig::default()
            },
        );
    }

    /// One-Handed Weapon Specialization: increases the damage you deal with one-handed melee
    /// weapons by 3/7/10%.
    fn apply_one_handed_weapon_specialization(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("one_handed_weapon_specialization");
        if points == 0 {
            return;
        }
        // Go's GetMainHandType answers one hand for everything but a two-hander.
        let one_hand = !self.main_hand_is_two_hand(sim, unit);
        apply_weapon_specialization(
            sim,
            unit,
            spell_data()
                .one_handed_weapon_specialization
                .fraction_at(points),
            one_hand,
        );
    }

    /// Reckoning: gives you a chance to gain an extra attack after Blocking a melee attack and
    /// after being the victim of a non-periodic critical strike.
    fn apply_reckoning(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("reckoning");
        if points == 0 {
            return;
        }
        let block_chance = spell_data().reckoning.fraction_at(points);
        let crit_chance = block_chance * 2.5;
        let label = sim.unit(unit).label.clone();

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: format!("Reckoning - Block{label}"),
                callback: CallbackMask::ON_SPELL_HIT_TAKEN,
                outcome: HitOutcome::BLOCK,
                proc_chance: block_chance,
                ..ProcTrigger::default()
            },
        );
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: format!("Reckoning - Crit{label}"),
                callback: CallbackMask::ON_SPELL_HIT_TAKEN,
                outcome: HitOutcome::CRIT,
                proc_chance: crit_chance,
                ..ProcTrigger::default()
            },
        );
    }

    /// Iron Creed: increases the threat generated by your Holy Strike ability. While
    /// Righteous Fury is active, Holy Strike also reduces your damage taken for 6 sec.
    fn apply_iron_creed(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("iron_creed");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::HOLY_STRIKE,
                kind: SpellModType::ThreatMultiplierPct,
                float_value: spell_data()
                    .iron_creed
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_THREAT)
                    .fraction_at(points),
                ..SpellModConfig::default()
            },
        );

        let rank = spell_data().iron_creed_triggered.highest();
        let reduction = spell_data()
            .iron_creed
            .effect(dbcenums::A_PROC_TRIGGER_SPELL_WITH_VALUE, 0)
            .fraction_at(points);
        let label = sim.unit(unit).label.clone();

        let iron_creed = sim.register_aura(
            unit,
            AuraConfig {
                label: format!("Iron Creed{label}"),
                action_id: Some(spell_action(rank.id)),
                duration: rank.duration(),
                ..AuraConfig::default()
            },
        );
        sim.attach_multiplicative_pseudo_stat_buff(
            iron_creed,
            PseudoStatField::DamageTakenMultiplier,
            1.0 - reduction,
        );

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: format!("Iron Creed - Trigger{label}"),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                class_spell_mask: masks::HOLY_STRIKE,
                outcome: HitOutcome::LANDED,
                ..ProcTrigger::default()
            },
        );
    }
}
