//! Go sim/warrior/talents_protection.go.

use std::cell::Cell;
use std::rc::Rc;

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::character::constants::DEFENSE_RATING_PER_DEFENSE_LEVEL;
use crate::prepare::character::cooldown_type;
use crate::prepare::dbcenums;
use crate::prepare::sim::{AuraCallback, AuraConfig, Sim, UnitId};
use crate::prepare::spell::{
    school, CastConfig, DefenseType, ProcMask, SpellConfig, SpellFlag as F,
};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::spelldata::Spell as Row;
use crate::prepare::stats::Stat;

use super::super::helpers::*;
use super::super::masks;
use super::super::spell_data::spell_data;
use super::super::spells::add_cooldown;
use super::super::Warrior;
use super::millis;

impl Warrior {
    /// Go `registerProtectionTalents`.
    pub(in super::super) fn register_protection_talents(&self, sim: &mut Sim, unit: UnitId) {
        // Tier 1
        // Improved Bloodrage: bloodrage.go
        self.register_shield_specialization(sim, unit);
        self.register_iron_will(sim, unit);

        // Tier 2
        self.register_anticipation(sim, unit);
        self.register_improved_revenge(sim, unit);
        self.register_improved_thunder_clap(sim, unit);

        // Tier 3
        self.register_last_stand(sim, unit);
        self.register_master_of_defense(sim, unit);
        self.register_improved_disarm(sim, unit);
        // Defiance: stances.go

        // Tier 4
        self.register_improved_sunder_armor(sim, unit);
        // Vanguard: charge.go
        self.register_improved_shield_bash(sim, unit);

        // Tier 5
        self.register_improved_shield_wall(sim, unit);
        self.register_concussion_blow(sim, unit);
        self.register_focused_rage(sim, unit);

        // Tier 6
        self.register_bastion(sim, unit);

        // Tier 7
        self.register_shield_slam(sim, unit);
    }

    fn register_anticipation(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("anticipation");
        if rank == 0 {
            return;
        }
        sim.add_stat(
            unit,
            Stat::DefenseRating,
            spell_data().anticipation.value_at(rank) * DEFENSE_RATING_PER_DEFENSE_LEVEL,
        );
    }

    fn register_shield_specialization(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("shield_specialization");
        if rank == 0 {
            return;
        }
        let ladder = &spell_data().shield_specialization;
        // BlockPercent is in percent (5 = 5%) since the core's avoidance rework.
        sim.add_stat(
            unit,
            Stat::BlockPercent,
            ladder
                .effect(dbcenums::A_MOD_BLOCK_PERCENT, 0)
                .value_at(rank),
        );
        register_rage_on_avoid(
            sim,
            unit,
            "Shield Specialization",
            ladder.effect_at(2).fraction_at(rank),
            HitOutcome::BLOCK,
        );
    }

    fn register_iron_will(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("iron_will");
        if rank == 0 {
            return;
        }
        let ladder = &spell_data().iron_will;
        let pseudo = &mut sim.unit_mut(unit).pseudo_stats;
        pseudo.fear_duration_multiplier = ladder
            .effect(dbcenums::A_MECHANIC_DURATION_MOD, 1)
            .multiplier_at(rank);
        pseudo.stun_duration_multiplier = ladder
            .effect(dbcenums::A_MECHANIC_DURATION_MOD, 12)
            .multiplier_at(rank);
    }

    fn register_last_stand(&self, sim: &mut Sim, unit: UnitId) {
        if !self.has_talent("last_stand") {
            return;
        }
        let rank = spell_data().last_stand.highest();
        let buff = spell_data().last_stand_triggered.highest();
        let action = spell_action(rank.id);
        let bonus = Rc::new(Cell::new(0.0));
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Last Stand".to_string(),
                action_id: Some(action.clone()),
                duration: buff.duration(),
                on_gain: Some(last_stand_on_gain(buff, bonus.clone())),
                on_expire: Some(last_stand_on_expire(bonus)),
                ..AuraConfig::default()
            },
        );
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: action,
                class_spell_mask: masks::LAST_STAND,
                cast: CastConfig {
                    cd,
                    ..CastConfig::default()
                },
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
        add_cooldown(sim, unit, spell, cooldown_type::SURVIVAL);
    }

    fn register_improved_sunder_armor(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("improved_sunder_armor");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::SUNDER_ARMOR,
                kind: SpellModType::PowerCostFlat,
                int_value: spell_data().improved_sunder_armor.tenths_at(rank) as i32,
                ..SpellModConfig::default()
            },
        );
    }

    fn register_improved_shield_wall(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("improved_shield_wall");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::SHIELD_WALL,
                kind: SpellModType::CooldownFlat,
                time_value: millis(spell_data().improved_shield_wall.value_at(rank)),
                ..SpellModConfig::default()
            },
        );
    }

    fn register_concussion_blow(&self, sim: &mut Sim, unit: UnitId) {
        if !self.has_talent("concussion_blow") {
            return;
        }
        let rank = spell_data().concussion_blow.highest();
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                class_spell_mask: masks::CONCUSSION_BLOW,
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: F::MELEE_METRICS | F::APL,
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: CastConfig {
                    cd,
                    ..non_empty_cast(true)
                },
                ..SpellConfig::default()
            },
        );
    }

    fn register_shield_slam(&self, sim: &mut Sim, unit: UnitId) {
        if !self.has_talent("shield_slam") {
            return;
        }
        let rank = spell_data().shield_slam.highest();
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                class_spell_mask: masks::SHIELD_SLAM,
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::MELEE_OH_SPECIAL,
                flags: F::MELEE_METRICS | F::APL,
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: cast_config(rank.gcd(), true, cd),
                has_extra_cast_condition: true,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                flat_threat_bonus: 508.0,
                ..SpellConfig::default()
            },
        );
    }

    fn register_focused_rage(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("focused_rage");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::FOCUSED_RAGE,
                kind: SpellModType::PowerCostFlat,
                int_value: spell_data().focused_rage.tenths_at(rank) as i32,
                ..SpellModConfig::default()
            },
        );
    }

    fn register_master_of_defense(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("master_of_defense");
        if rank == 0 {
            return;
        }
        register_rage_on_avoid(
            sim,
            unit,
            "Master of Defense",
            spell_data().master_of_defense.fraction_at(rank),
            HitOutcome::DODGE | HitOutcome::PARRY,
        );
    }

    fn register_improved_revenge(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("improved_revenge");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::REVENGE,
                kind: SpellModType::DamageDoneFlat,
                float_value: spell_data().improved_revenge.fraction_at(rank),
                ..SpellModConfig::default()
            },
        );
    }

    fn register_improved_disarm(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("improved_disarm");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::DISARM,
                kind: SpellModType::CooldownFlat,
                time_value: millis(spell_data().improved_disarm.value_at(rank)),
                ..SpellModConfig::default()
            },
        );
    }

    fn register_improved_shield_bash(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("improved_shield_bash");
        if rank == 0 {
            return;
        }
        let silence = spell_data().improved_shield_bash_triggered.highest();
        // Nothing in the sim reads a silence on an enemy, so the aura only shows in metrics.
        let label = "Shield Bash - Silence".to_string();
        new_enemy_aura_array(sim, |sim, target| {
            sim.get_or_register_aura(
                target,
                AuraConfig {
                    label: label.clone(),
                    action_id: Some(spell_action(silence.id)),
                    duration: silence.duration(),
                    ..AuraConfig::default()
                },
            )
        });
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Improved Shield Bash".to_string(),
                proc_chance: spell_data().improved_shield_bash.fraction_at(rank),
                trigger_immediately: true,
                class_spell_mask: masks::SHIELD_BASH,
                outcome: HitOutcome::LANDED,
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                ..ProcTrigger::default()
            },
        );
    }

    fn register_bastion(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("bastion");
        if rank == 0 {
            return;
        }
        let damage_mod = sim.add_dynamic_mod(
            unit,
            SpellModConfig {
                school: school::PHYSICAL,
                kind: SpellModType::DamageDonePct,
                float_value: spell_data().bastion.fraction_at(rank),
                ..SpellModConfig::default()
            },
        );
        if sim.unit(unit).pseudo_stats.can_block {
            sim.activate_spell_mod(damage_mod);
        }
    }

    fn register_improved_thunder_clap(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talent("improved_thunder_clap");
        if rank == 0 {
            return;
        }
        // The slowing effect is thunder_clap.go's.
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::THUNDER_CLAP,
                kind: SpellModType::PowerCostFlat,
                int_value: spell_data().improved_thunder_clap.tenths_at(rank) as i32,
                ..SpellModConfig::default()
            },
        );
    }
}

/// Go `registerRageOnAvoid`: rage at a chance on a hit taken that the outcome names.
fn register_rage_on_avoid(
    sim: &mut Sim,
    unit: UnitId,
    name: &str,
    chance: f64,
    outcome: HitOutcome,
) {
    sim.make_proc_trigger_aura(
        unit,
        &ProcTrigger {
            name: name.to_string(),
            proc_chance: chance,
            trigger_immediately: true,
            outcome,
            callback: CallbackMask::ON_SPELL_HIT_TAKEN,
            ..ProcTrigger::default()
        },
    );
}

/// The aura's gain: maximum health rises by the buff's share, which heals that much too.
fn last_stand_on_gain(buff: &'static Row, bonus: Rc<Cell<f64>>) -> AuraCallback {
    let share = buff.effect(dbcenums::A_MOD_MAX_HEALTH, 0).percent();
    Rc::new(move |sim: &mut Sim, aura| {
        let unit = sim.aura(aura).unit;
        bonus.set(sim.stat(unit, Stat::Health) * share);
        sim.update_max_health(unit, bonus.get());
    })
}

/// The aura's expiry takes the bonus back.
fn last_stand_on_expire(bonus: Rc<Cell<f64>>) -> AuraCallback {
    Rc::new(move |sim: &mut Sim, aura| {
        let unit = sim.aura(aura).unit;
        sim.update_max_health(unit, -bonus.get());
    })
}
