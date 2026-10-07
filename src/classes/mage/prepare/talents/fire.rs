//! Go sim/mage/talents_fire.go.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::dbcenums;
use crate::prepare::sim::{AuraConfig, EventCallbacks, Sim, UnitId, SECOND};
use crate::prepare::spell::{school, DefenseType, DotConfig, ProcMask, SpellConfig, SpellFlag};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};

use super::super::masks;
use super::super::spell_data::spell_data;
use super::super::Mage;
use super::millis;

impl Mage {
    /// Go `registerFireTalents`.
    pub(super) fn register_fire_talents(&self, sim: &mut Sim, unit: UnitId) {
        // Tier 1
        self.register_wake_of_fire(sim, unit);
        self.register_incineration(sim, unit);
        self.register_improved_fireball(sim, unit);

        // Tier 2: Flame Throwing and Impact model nothing in Go either.
        self.register_ignite(sim, unit);

        // Tier 3
        self.register_burning_soul(sim, unit);
        self.register_improved_flamestrike(sim, unit);
        // Pyroblast: pyroblast.go

        // Tier 4: Improved Scorch is in scorch.go, Improved Fire Ward models nothing.
        self.register_hot_streak(sim, unit);
        self.register_master_of_elements(sim, unit);

        // Tier 5
        self.register_critical_mass(sim, unit);
        // Blast Wave: blast_wave.go

        // Tier 6
        self.register_fire_power(sim, unit);

        // Tier 7: Combustion is in combustion.go.
    }

    /// The Fire Blast cooldown half only.
    fn register_wake_of_fire(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("wake_of_fire");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::FIRE_BLAST,
                time_value: millis(
                    spell_data()
                        .wake_of_fire
                        .effect(dbcenums::A_ADD_FLAT_MODIFIER, dbcenums::SPELLMOD_COOLDOWN)
                        .value_at(rank),
                ),
                kind: SpellModType::CooldownFlat,
                ..SpellModConfig::default()
            },
        );
    }

    /// Arcane Blast and Ice Lance as well as TBC's Fire Blast and Scorch.
    fn register_incineration(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("incineration");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ARCANE_BLAST
                    | masks::FIRE_BLAST
                    | masks::ICE_LANCE
                    | masks::SCORCH,
                float_value: spell_data().incineration.value_at(rank),
                kind: SpellModType::BonusCritPercent,
                ..SpellModConfig::default()
            },
        );
    }

    fn register_improved_fireball(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("improved_fireball");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                // Client mask 11069: Fireball and Frostfire Bolt.
                class_mask: masks::FIREBALL | masks::FROSTFIRE_BOLT,
                time_value: millis(
                    spell_data()
                        .improved_fireball
                        .effect(
                            dbcenums::A_ADD_FLAT_MODIFIER,
                            dbcenums::SPELLMOD_CASTING_TIME,
                        )
                        .value_at(rank),
                ),
                kind: SpellModType::CastTimeFlat,
                ..SpellModConfig::default()
            },
        );
    }

    /// Ignite pays out a share of the fire critical strike that lit it over its ticks.
    fn register_ignite(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("ignite");
        if rank == 0 {
            return;
        }
        let data = spell_data();
        let ignite_rank = data.ignite_triggered.highest();
        let tick_length = 2 * SECOND;
        let number_of_ticks = (ignite_rank.duration() / tick_length) as i32;

        sim.register_spell(
            unit,
            SpellConfig {
                action_id: ActionId {
                    spell_id: ignite_rank.id,
                    ..ActionId::default()
                },
                spell_school: school::FIRE,
                defense_type: DefenseType::Magic,
                proc_mask: ProcMask::SPELL_DAMAGE,
                class_spell_mask: masks::IGNITE,
                flags: SpellFlag::IGNORE_MODIFIERS
                    | SpellFlag::NO_SPELL_MODS
                    | SpellFlag::NO_ON_CAST_COMPLETE
                    | SpellFlag::IGNORE_RESISTS
                    | SpellFlag::PROC,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                dot: DotConfig {
                    aura: AuraConfig {
                        label: "Ignite".to_string(),
                        ..AuraConfig::default()
                    },
                    number_of_ticks,
                    tick_length,
                    ..DotConfig::default()
                },
                ..SpellConfig::default()
            },
        );

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Ignite Talent".to_string(),
                // Forever's 11119 lacks the bit (Era's ranks carry it).
                can_proc_from_procs: data.ignite.highest().can_proc_from_procs(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                proc_mask: ProcMask::SPELL_DAMAGE,
                outcome: HitOutcome::CRIT,
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    /// The threat half only; the pushback protection has nothing to act on in the sim.
    fn register_burning_soul(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("burning_soul");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::FIRE,
                float_value: spell_data()
                    .burning_soul
                    .effect(dbcenums::A_MOD_THREAT, 4)
                    .fraction_at(rank),
                kind: SpellModType::ThreatMultiplierPct,
                ..SpellModConfig::default()
            },
        );
    }

    fn register_improved_flamestrike(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("improved_flamestrike");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::FLAMESTRIKE,
                float_value: spell_data().improved_flamestrike.value_at(rank),
                kind: SpellModType::BonusCritPercent,
                ..SpellModConfig::default()
            },
        );
    }

    /// Heating Up: Fireball, Frostfire Bolt, Fire Blast and Scorch crits each take 25% off
    /// Pyroblast's cast time, stacking 3 times.
    fn register_hot_streak(&self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("heating_up") {
            return;
        }
        let buff = spell_data().heating_up_triggered.highest();
        let per_stack = buff
            .effect(
                dbcenums::A_ADD_PCT_MODIFIER,
                dbcenums::SPELLMOD_CASTING_TIME,
            )
            .percent();

        let cast_time_mod = sim.add_dynamic_mod(
            unit,
            SpellModConfig {
                class_mask: masks::PYROBLAST,
                kind: SpellModType::CastTimePct,
                ..SpellModConfig::default()
            },
        );

        sim.register_aura(
            unit,
            AuraConfig {
                label: "Heating Up".to_string(),
                action_id: Some(ActionId {
                    spell_id: buff.id,
                    ..ActionId::default()
                }),
                duration: buff.duration(),
                max_stacks: i32::from(buff.max_stack),
                on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.activate_spell_mod(cast_time_mod)
                })),
                on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.deactivate_spell_mod(cast_time_mod)
                })),
                on_stacks_change: Some(Rc::new(move |sim: &mut Sim, _, _, new_stacks| {
                    sim.update_spell_mod_float_value(
                        cast_time_mod,
                        per_stack * f64::from(new_stacks),
                    )
                })),
                events: EventCallbacks {
                    on_cast_complete: true,
                    ..EventCallbacks::default()
                },
                ..AuraConfig::default()
            },
        );

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Heating Up Trigger".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                class_spell_mask: masks::FIREBALL
                    | masks::FROSTFIRE_BOLT
                    | masks::FIRE_BLAST
                    | masks::SCORCH,
                outcome: HitOutcome::CRIT,
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    fn register_master_of_elements(&self, sim: &mut Sim, unit: UnitId) {
        if self.talents.i32("master_of_elements") == 0 {
            return;
        }
        // 29074's 9 ms ProcCategoryRecovery: an area spell that crits several targets refunds
        // once.
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Master of Elements".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                class_spell_mask: masks::ALL,
                outcome: HitOutcome::CRIT,
                icd: spell_data().master_of_elements.highest().icd(),
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    fn register_critical_mass(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("critical_mass");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::FIRE,
                float_value: spell_data().critical_mass.value_at(rank),
                kind: SpellModType::BonusCritPercent,
                ..SpellModConfig::default()
            },
        );
    }

    /// Ignite is left out by its NoSpellMods flag.
    fn register_fire_power(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("fire_power");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::FIRE,
                float_value: spell_data()
                    .fire_power
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_DAMAGE)
                    .fraction_at(rank),
                kind: SpellModType::DamageDoneFlat,
                ..SpellModConfig::default()
            },
        );
    }
}
