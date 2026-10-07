//! Go sim/mage/talents_arcane.go and arcane_charge.go.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::character::constants::CHARACTER_LEVEL;
use crate::prepare::dbcenums;
use crate::prepare::sim::{AuraConfig, EventCallbacks, Sim, UnitId, MILLISECOND};
use crate::prepare::spell::school;
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{SchoolIndex, Stat, Stats};

use super::super::masks;
use super::super::spell_data::spell_data;
use super::super::Mage;

impl Mage {
    /// Go `registerArcaneTalents`.
    pub(super) fn register_arcane_talents(&self, sim: &mut Sim, unit: UnitId) {
        // Tier 1: Wand Specialization and Improved Channeling model nothing in Go either.
        self.register_arcane_focus(sim, unit);

        // Tier 2
        self.register_arcane_subtlety(sim, unit);
        self.register_magic_absorption(sim, unit);
        self.register_arcane_concentration(sim, unit);
        self.register_arcane_resilience(sim, unit);

        // Tier 3: Arcane Geometry models nothing in Go either.
        self.register_arcane_impact(sim, unit);
        // Arcane Blast: arcane_blast.go and arcane_charge.go

        // Tier 4: Arcane Shielding and Improved Counterspell model nothing in Go either.
        self.register_arcane_meditation(sim, unit);
        self.register_missile_barrage(sim, unit);

        // Tier 5: Presence of Mind is in presence_of_mind.go.
        self.register_arcane_mind(sim, unit);

        // Tier 6
        self.register_arcane_instability(sim, unit);

        // Tier 7: Arcane Power is in arcane_power.go.
    }

    fn register_arcane_focus(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("arcane_focus");
        if rank == 0 {
            return;
        }
        sim.unit_mut(unit).pseudo_stats.school_bonus_hit_chance[SchoolIndex::Arcane as usize] +=
            spell_data().arcane_focus.value_at(rank);
    }

    fn register_arcane_subtlety(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("arcane_subtlety");
        if rank == 0 {
            return;
        }
        let data = &spell_data().arcane_subtlety;
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::ARCANE,
                float_value: data
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_THREAT)
                    .fraction_at(rank),
                kind: SpellModType::ThreatMultiplierPct,
                ..SpellModConfig::default()
            },
        );

        // The client states the penetration as a negative target resistance.
        sim.add_stat(
            unit,
            Stat::SpellPiercing,
            -data
                .effect(dbcenums::A_MOD_TARGET_RESISTANCE, 126)
                .value_at(rank),
        );
    }

    /// Only the resistance half; the mana returned on a resisted spell is not modelled.
    fn register_magic_absorption(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("magic_absorption");
        if rank == 0 {
            return;
        }
        let resist = spell_data()
            .magic_absorption
            .effect(dbcenums::A_MOD_RESISTANCE, 124)
            .value_at(rank);
        sim.add_stats(
            unit,
            &Stats::from_pairs(&[
                (Stat::ArcaneResistance, resist),
                (Stat::FireResistance, resist),
                (Stat::FrostResistance, resist),
                (Stat::NatureResistance, resist),
                (Stat::ShadowResistance, resist),
            ]),
        );
    }

    fn register_arcane_concentration(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("arcane_concentration");
        if rank == 0 {
            return;
        }
        let data = spell_data();
        let clearcasting_rank = data.arcane_concentration_triggered.highest();

        sim.register_aura(
            unit,
            AuraConfig {
                label: "Clearcasting".to_string(),
                action_id: Some(ActionId {
                    spell_id: clearcasting_rank.id,
                    ..ActionId::default()
                }),
                duration: clearcasting_rank.duration(),
                on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.unit_mut(unit).pseudo_stats.spell_cost_percent_modifier -= 100;
                })),
                on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.unit_mut(unit).pseudo_stats.spell_cost_percent_modifier += 100;
                })),
                events: EventCallbacks {
                    on_cast_complete: true,
                    ..EventCallbacks::default()
                },
                ..AuraConfig::default()
            },
        );

        // Forever states a flat SpellAuraOptions.ProcChance of 100 on the talent spell and puts
        // the real per-rank chance on the effect. Spells another spell triggers do not proc it:
        // Arcane Missiles' missiles and Blizzard's ticks.
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Arcane Concentration".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                class_spell_mask: masks::ALL_DAMAGING & !masks::ARCANE_MISSILES_TICK,
                outcome: HitOutcome::LANDED,
                proc_chance: data.arcane_concentration.effect_at(1).fraction_at(rank),
                icd: data.arcane_concentration.highest().icd(),
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    fn register_arcane_resilience(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("arcane_resilience");
        if rank == 0 {
            return;
        }
        sim.unit_mut(unit).sdm.add_stat_dependency(
            Stat::Intellect,
            Stat::Armor,
            spell_data().arcane_resilience.fraction_at(rank),
        );
    }

    /// Every arcane spell of the mage's, not TBC's Arcane Blast and Arcane Explosion only.
    fn register_arcane_impact(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("arcane_impact");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::ARCANE,
                float_value: spell_data().arcane_impact.value_at(rank),
                kind: SpellModType::BonusCritPercent,
                ..SpellModConfig::default()
            },
        );
    }

    fn register_arcane_meditation(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("arcane_meditation");
        if rank == 0 {
            return;
        }
        sim.unit_mut(unit).pseudo_stats.spirit_regen_rate_casting +=
            spell_data().arcane_meditation.fraction_at(rank);
    }

    /// Arcane Blast landing (40%), or Fireball, Frostbolt or Frostfire Bolt (20%), can make the
    /// next Arcane Missiles free and fire its missiles every 0.5 sec instead of every second.
    fn register_missile_barrage(&self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("missile_barrage") {
            return;
        }
        let data = spell_data();
        let buff = data.missile_barrage_triggered.highest();

        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Missile Barrage".to_string(),
                action_id: Some(ActionId {
                    spell_id: 44404,
                    ..ActionId::default()
                }),
                duration: buff.duration(),
                events: EventCallbacks {
                    on_cast_complete: true,
                    ..EventCallbacks::default()
                },
                ..AuraConfig::default()
            },
        );
        sim.attach_spell_mod(
            aura,
            SpellModConfig {
                class_mask: masks::ARCANE_MISSILES_CAST,
                float_value: buff
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_COST)
                    .average(CHARACTER_LEVEL)
                    / 100.0,
                kind: SpellModType::PowerCostPctAdd,
                ..SpellModConfig::default()
            },
        );
        sim.attach_spell_mod(
            aura,
            SpellModConfig {
                class_mask: masks::ARCANE_MISSILES_CAST,
                time_value: (buff
                    .effect(
                        dbcenums::A_ADD_FLAT_MODIFIER,
                        dbcenums::SPELLMOD_ACTIVATION_TIME,
                    )
                    .average(CHARACTER_LEVEL) as i64)
                    .wrapping_mul(MILLISECOND),
                kind: SpellModType::DotTickLengthFlat,
                ..SpellModConfig::default()
            },
        );

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Missile Barrage Trigger".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                class_spell_mask: masks::ARCANE_BLAST
                    | masks::FIREBALL
                    | masks::FROSTBOLT
                    | masks::FROSTFIRE_BOLT,
                outcome: HitOutcome::LANDED,
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    fn register_arcane_mind(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("arcane_mind");
        if rank == 0 {
            return;
        }
        let data = &spell_data().arcane_mind;
        sim.unit_mut(unit).sdm.multiply_stat(
            Stat::Intellect,
            data.effect(dbcenums::A_MOD_TOTAL_STAT_PERCENTAGE, 0)
                .multiplier_at(rank),
        );
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::ARCANE,
                float_value: data
                    .effect(
                        dbcenums::A_ADD_PCT_MODIFIER,
                        dbcenums::SPELLMOD_CRIT_DAMAGE_BONUS,
                    )
                    .fraction_at(rank),
                kind: SpellModType::CritMultiplierFlat,
                ..SpellModConfig::default()
            },
        );
    }

    fn register_arcane_instability(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("arcane_instability");
        if rank == 0 {
            return;
        }
        let data = &spell_data().arcane_instability;
        let damage = data
            .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_DAMAGE)
            .fraction_at(rank);
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL & !masks::FROSTFIRE_BOLT,
                float_value: damage,
                kind: SpellModType::DamageDoneFlat,
                ..SpellModConfig::default()
            },
        );
        // The SPELLMOD_DOT mask leaves out Frostfire Bolt: its hit takes the bonus, its DoT does
        // not.
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::FROSTFIRE_BOLT,
                float_value: damage,
                kind: SpellModType::DirectDamageDoneFlat,
                ..SpellModConfig::default()
            },
        );
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                float_value: data.effect(dbcenums::A_MOD_CRIT_PCT, 0).value_at(rank),
                kind: SpellModType::BonusCritPercent,
                ..SpellModConfig::default()
            },
        );
    }

    /// Go `Mage.registerArcaneCharges`: Forever's Arcane Blast buff (400573). Each stack raises
    /// the damage of the mage's other spells and the cost of Arcane Blast itself.
    pub(crate) fn register_arcane_charges(&self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("arcane_blast") {
            return;
        }
        let buff_rank = spell_data().arcane_blast_triggered.highest();
        let damage_per_stack = buff_rank
            .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_DAMAGE)
            .average(CHARACTER_LEVEL)
            / 100.0;
        let cost_per_stack = buff_rank
            .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_COST)
            .average(CHARACTER_LEVEL)
            / 100.0;

        // 400573's damage mask names every mage damage spell but Arcane Blast, Arcane Missiles,
        // Blizzard and Flamestrike. Its DoT mask names Fireball and Frostfire Bolt only, so
        // Pyroblast's hit takes the bonus and its DoT does not.
        let damage_mod = sim.add_dynamic_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL_DAMAGING
                    & !(masks::ARCANE_BLAST
                        | masks::ARCANE_MISSILES
                        | masks::BLIZZARD
                        | masks::FLAMESTRIKE
                        | masks::PYROBLAST),
                kind: SpellModType::DamageDoneFlat,
                ..SpellModConfig::default()
            },
        );
        let pyroblast_mod = sim.add_dynamic_mod(
            unit,
            SpellModConfig {
                class_mask: masks::PYROBLAST,
                kind: SpellModType::DirectDamageDoneFlat,
                ..SpellModConfig::default()
            },
        );
        let cost_mod = sim.add_dynamic_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ARCANE_BLAST,
                kind: SpellModType::PowerCostPctAdd,
                ..SpellModConfig::default()
            },
        );

        sim.register_aura(
            unit,
            AuraConfig {
                label: "Arcane Blast".to_string(),
                action_id: Some(ActionId {
                    spell_id: buff_rank.id,
                    ..ActionId::default()
                }),
                duration: buff_rank.duration(),
                max_stacks: i32::from(buff_rank.max_stack),
                on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.activate_spell_mod(damage_mod);
                    sim.activate_spell_mod(pyroblast_mod);
                    sim.activate_spell_mod(cost_mod);
                })),
                on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.deactivate_spell_mod(damage_mod);
                    sim.deactivate_spell_mod(pyroblast_mod);
                    sim.deactivate_spell_mod(cost_mod);
                })),
                on_stacks_change: Some(Rc::new(move |sim: &mut Sim, _, _, new_stacks| {
                    let stacks = f64::from(new_stacks);
                    sim.update_spell_mod_float_value(damage_mod, damage_per_stack * stacks);
                    sim.update_spell_mod_float_value(pyroblast_mod, damage_per_stack * stacks);
                    sim.update_spell_mod_float_value(cost_mod, cost_per_stack * stacks);
                })),
                events: EventCallbacks {
                    on_cast_complete: true,
                    ..EventCallbacks::default()
                },
                ..AuraConfig::default()
            },
        );
    }
}
