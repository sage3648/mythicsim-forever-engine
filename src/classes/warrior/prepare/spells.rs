//! The Warrior's ability registrations: Go sim/warrior `register*` for every ability the class
//! registers in `Initialize`. A closure Go gives a spell config (`ApplyEffects` and the like)
//! only runs in a fight, so what preparation keeps of it is the field that says it is there:
//! `has_extra_cast_condition`, the related buff, the aura callbacks.

use std::rc::Rc;

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::buffs::generated;
use crate::prepare::character::{cooldown_type, MajorCooldown};
use crate::prepare::dbcenums;
use crate::prepare::levels::character_level;
use crate::prepare::sim::{AuraConfig, Cooldown, Rules, Sim, UnitId, NEVER_EXPIRES, SECOND};
use crate::prepare::spell::{
    school, Cast, CastConfig, DefenseType, DotConfig, ProcMask, SpellConfig, SpellFlag,
};
use crate::prepare::stats::Stat;

use super::helpers::*;
use super::masks;
use super::spell_data::spell_data;
use super::Warrior;

/// The reference adds a flat 135 to the threat of every rank of Hamstring. Under Forever's rules
/// its threat is 1.25 times its damage and nothing flat: its crits for 8 make 10 threat, where a
/// flat +1 would make 9 (warrior threat measurements at levels 20 to 24, 7 November 2026).
const HAMSTRING_REFERENCE_FLAT_THREAT: f64 = 135.0;
/// Shield Bash's threat under Forever's rules is 1.5 times its damage, times the stance: its
/// crits for 8, 8, 9, 10 and 11 read 12.0, 12.2, 13.5, 15.0 and 16.7 threat, where 1.5 predicts
/// 12, 12, 13.5, 15 and 16.5 and a flat +2 would predict 10, 10, 11, 12 and 13 (same
/// measurements). The reference gives it the plain damage.
const SHIELD_BASH_THREAT_MULTIPLIER: f64 = 1.5;
/// Go `heroicStrikeThreatPerLevel`: patch 184, upstream's in-game captures.
const HEROIC_STRIKE_THREAT_PER_LEVEL: f64 = 2.54;
/// Go `cleaveThreatPerLevel`.
const CLEAVE_THREAT_PER_LEVEL: f64 = 1.53;
/// Cleave rank 1 (845) and the threat it adds beyond its damage under Forever's rules: 30 on one
/// target and about 15 on each of two, times the stance (warrior threat measurements at levels
/// 20 to 24, 7 November 2026). The per level figure above would give 30.6 at the rank's level 20.
const CLEAVE_RANK_1_ID: i32 = 845;
const CLEAVE_RANK_1_THREAT: f64 = 30.0;
/// Cleave rank 2 (7369, spell level 30) adds 46 under Forever's rules; ranks 3 to 5 carry the
/// 1.5 a spell level ranks 1 and 2 measure, as whole numbers (60, 75 and 90).
const CLEAVE_RANK_2_ID: i32 = 7369;
const CLEAVE_RANK_2_THREAT: f64 = 46.0;
const FOREVER_CLEAVE_THREAT_PER_LEVEL: f64 = 1.5;
/// Heroic Strike ranks 1 to 4 (78, 284, 285, 1608) under Forever's rules: whole numbers, Rank 4
/// measured at 60.08 over 104 clean threat readings on client 1.60.1.70862 (8 November 2026),
/// Rank 3 at 39.99. Ranks 5 and up carry Rank 4's 2.5 a spell level (80 to 150).
const HEROIC_STRIKE_LOW_RANK_THREAT: [(i32, f64); 4] =
    [(78, 17.0), (284, 28.0), (285, 40.0), (1608, 60.0)];
const FOREVER_HEROIC_STRIKE_THREAT_PER_LEVEL: f64 = 2.5;
/// Slam under Forever's rules: 1.5 a spell level on every rank, rounded to a whole number. Rank 1
/// at spell level 20 adds 30 (client 1.60.1.70862, 8 November 2026), rank 2 45. The reference
/// adds a flat 140 to every rank.
const FOREVER_SLAM_THREAT_PER_LEVEL: f64 = 1.5;
const SLAM_REFERENCE_FLAT_THREAT: f64 = 140.0;

/// A rank's flat threat at so much a spell level, as the whole number Forever adds.
fn whole_threat_per_level(per_level: f64, rank: &crate::prepare::spelldata::Spell) -> f64 {
    (per_level * f64::from(rank.spell_level)).round()
}

/// Heroic Strike's flat threat for a rank: the reference's 2.54 a spell level, or Forever's
/// whole numbers.
fn heroic_strike_threat(rules: Rules, rank: &crate::prepare::spelldata::Spell) -> f64 {
    if rules != Rules::Forever {
        return HEROIC_STRIKE_THREAT_PER_LEVEL * f64::from(rank.spell_level);
    }
    HEROIC_STRIKE_LOW_RANK_THREAT
        .iter()
        .find(|(id, _)| *id == rank.id)
        .map_or_else(
            || whole_threat_per_level(FOREVER_HEROIC_STRIKE_THREAT_PER_LEVEL, rank),
            |(_, threat)| *threat,
        )
}

/// Cleave's flat threat for a rank, split between the targets it lands on at run time.
fn cleave_threat(rules: Rules, rank: &crate::prepare::spelldata::Spell) -> f64 {
    if rules != Rules::Forever {
        return CLEAVE_THREAT_PER_LEVEL * f64::from(rank.spell_level);
    }
    match rank.id {
        CLEAVE_RANK_1_ID => CLEAVE_RANK_1_THREAT,
        CLEAVE_RANK_2_ID => CLEAVE_RANK_2_THREAT,
        _ => whole_threat_per_level(FOREVER_CLEAVE_THREAT_PER_LEVEL, rank),
    }
}

/// Slam's flat threat for a rank.
fn slam_threat(rules: Rules, rank: &crate::prepare::spelldata::Spell) -> f64 {
    if rules != Rules::Forever {
        return SLAM_REFERENCE_FLAT_THREAT;
    }
    whole_threat_per_level(FOREVER_SLAM_THREAT_PER_LEVEL, rank)
}
/// Shield Slam's flat threat. The Go reference adds 508 on every rank. Under Forever's rules
/// rank 1 (23922, spell level 40) adds 178: a protection warrior's combat and threat logs at
/// levels 40 to 42 (client 1.60.1.70862, 8 November 2026) held 177.7 over damage from 223 to
/// 400, once on a crit too. Ranks 2 to 4 are not measured: they carry rank 1's 4.45 a spell
/// level as whole numbers, 214, 240 and 267.
const SHIELD_SLAM_REFERENCE_FLAT_THREAT: f64 = 508.0;
const SHIELD_SLAM_RANK_1_ID: i32 = 23922;
const SHIELD_SLAM_RANK_1_THREAT: f64 = 178.0;
const FOREVER_SHIELD_SLAM_THREAT_PER_LEVEL: f64 = 4.45;

pub(super) fn shield_slam_threat(rules: Rules, rank: &crate::prepare::spelldata::Spell) -> f64 {
    if rules != Rules::Forever {
        return SHIELD_SLAM_REFERENCE_FLAT_THREAT;
    }
    match rank.id {
        SHIELD_SLAM_RANK_1_ID => SHIELD_SLAM_RANK_1_THREAT,
        _ => whole_threat_per_level(FOREVER_SHIELD_SLAM_THREAT_PER_LEVEL, rank),
    }
}

/// The hand Shield Slam and Shield Bash strike with. Their rows require the shield and name no
/// hand, and the Go reference gives them the off hand's special mask. Under Forever's rules
/// they are main hand specials: the main hand weapon's chance on hit and enchant hear them and
/// the main hand weapon skill sets their miss and parry. In a protection warrior's combat log
/// of client 1.60.1.70862 the main hand weapon's proc fired on 54 of 80 isolated Shield Slams
/// and 7 of 8 Shield Bashes, against none on 74 Shield Block and 9 Battle Shout casts. Windfury
/// Totem still never rolls on them: their rows carry neither the main hand nor the next swing
/// bit.
pub(super) fn shield_strike_proc_mask(rules: Rules) -> ProcMask {
    match rules {
        Rules::Forever => ProcMask::MELEE_MH_SPECIAL,
        Rules::GoReference => ProcMask::MELEE_OH_SPECIAL,
    }
}

/// Go `revengeThreatMultiplier`: patch 183.
const REVENGE_THREAT_MULTIPLIER: f64 = 2.25;
/// Go `sunderThreatPerAttackPower`: patch 185, a share of the spell's melee attack power that
/// the cast adds to the client's flat threat.
pub(super) const SUNDER_THREAT_PER_ATTACK_POWER: f64 = 0.05;

impl Warrior {
    /// Go `registerRecklessness`.
    pub(super) fn register_recklessness(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().recklessness.highest();
        let action = spell_action(rank.id);
        let crit = rank
            .effect(dbcenums::A_MOD_CRIT_PCT, 0)
            .average(character_level());
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Recklessness".to_string(),
                action_id: Some(action.clone()),
                duration: rank.duration(),
                ..AuraConfig::default()
            },
        );
        let mut stats = crate::prepare::stats::Stats::default();
        stats[Stat::PhysicalCritPercent] = crit;
        stats[Stat::SpellCritPercent] = crit;
        sim.attach_stats_buff(aura, stats);
        sim.attach_multiplicative_pseudo_stat_buff(
            aura,
            crate::prepare::aura_helpers::PseudoStatField::DamageTakenMultiplier,
            1.0 + rank
                .effect(dbcenums::A_MOD_DAMAGE_PERCENT_TAKEN, 127)
                .percent(),
        );
        attach_fear_immunity(sim, aura);

        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: action,
                defense_type: DefenseType::Melee,
                flags: SpellFlag::APL | SpellFlag::CAST_WHILE_INCAPACITATED,
                class_spell_mask: masks::RECKLESSNESS,
                cast: CastConfig {
                    default_cast: Cast {
                        gcd: rank.gcd(),
                        ..Cast::default()
                    },
                    cd,
                    ..CastConfig::default()
                },
                has_extra_cast_condition: true,
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
        add_cooldown(sim, unit, spell, cooldown_type::DPS);
    }

    /// Go `registerShieldWall`.
    pub(super) fn register_shield_wall(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().shield_wall.highest();
        let action = spell_action(rank.id);
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Shield Wall".to_string(),
                action_id: Some(action.clone()),
                duration: rank.duration(),
                ..AuraConfig::default()
            },
        );
        sim.attach_multiplicative_pseudo_stat_buff(
            aura,
            crate::prepare::aura_helpers::PseudoStatField::DamageTakenMultiplier,
            1.0 + rank
                .effect(dbcenums::A_MOD_DAMAGE_PERCENT_TAKEN, 127)
                .percent(),
        );
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: action,
                defense_type: DefenseType::Melee,
                class_spell_mask: masks::SHIELD_WALL,
                cast: cast_config(rank.gcd(), true, cd),
                has_extra_cast_condition: true,
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
        deactivate_without_shield(sim, unit, aura);
        add_cooldown(sim, unit, spell, cooldown_type::SURVIVAL);
    }

    /// Go `registerRetaliation`.
    pub(super) fn register_retaliation(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().retaliation.highest();
        let hit = spell_data().retaliation_triggered.highest();
        let action = spell_action(rank.id);

        sim.register_spell(
            unit,
            SpellConfig {
                class_spell_mask: masks::RETALIATION_HIT,
                action_id: spell_action(hit.id),
                spell_school: hit.spell_school(),
                defense_type: hit.defense_type_core(),
                proc_mask: ProcMask::MELEE_MH,
                flags: SpellFlag::MELEE_METRICS,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                ..SpellConfig::default()
            },
        );
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Retaliation".to_string(),
                action_id: Some(action.clone()),
                duration: rank.duration(),
                max_stacks: i32::from(rank.proc_charges),
                events: crate::prepare::sim::EventCallbacks {
                    on_spell_hit_taken: true,
                    ..Default::default()
                },
                ..AuraConfig::default()
            },
        );
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: action,
                defense_type: DefenseType::Melee,
                class_spell_mask: masks::RETALIATION,
                cast: cast_config(rank.gcd(), false, cd),
                has_extra_cast_condition: true,
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
        add_cooldown(sim, unit, spell, cooldown_type::DPS);
    }

    /// Go `registerBerserkerRage`.
    pub(super) fn register_berserker_rage(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().berserker_rage.highest();
        let action = spell_action(rank.id);
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Berserker Rage".to_string(),
                action_id: Some(action.clone()),
                duration: rank.duration(),
                on_gain: Some(noop()),
                on_expire: Some(noop()),
                ..AuraConfig::default()
            },
        );
        attach_fear_immunity(sim, aura);
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: action,
                class_spell_mask: masks::BERSERKER_RAGE,
                flags: SpellFlag::APL | SpellFlag::CAST_WHILE_INCAPACITATED,
                cast: cast_config(rank.gcd(), true, cd),
                has_extra_cast_condition: true,
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
        add_cooldown(sim, unit, spell, cooldown_type::SURVIVAL);
    }

    /// Go `registerBloodrage`.
    pub(super) fn register_bloodrage(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().bloodrage.highest();
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                cast: CastConfig {
                    cd,
                    ..non_empty_cast(false)
                },
                ..SpellConfig::default()
            },
        );
        add_cooldown(sim, unit, spell, cooldown_type::UNKNOWN);
    }

    /// Go `registerCharge`.
    pub(super) fn register_charge(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().charge.highest();
        let action = spell_action(rank.id);
        let charge_cd = cooldown_of(rank);
        register_dash_aura(sim, unit, "Charge", &action, charge_cd);
        let cd = new_cooldown(sim, unit, charge_cd);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: action,
                spell_school: school::PHYSICAL,
                flags: SpellFlag::APL,
                class_spell_mask: masks::CHARGE,
                min_range: f64::from(rank.min_range),
                max_range: f64::from(rank.max_range),
                cast: CastConfig {
                    cd,
                    ignore_haste: true,
                    ..CastConfig::default()
                },
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerIntercept`.
    pub(super) fn register_intercept(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().intercept.highest();
        let action = spell_action(rank.id);
        let intercept_cd = cooldown_of(rank);
        register_dash_aura(sim, unit, "Intercept", &action, intercept_cd);
        let cd = new_cooldown(sim, unit, intercept_cd);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: action,
                spell_school: school::PHYSICAL,
                flags: SpellFlag::APL,
                class_spell_mask: masks::INTERCEPT,
                min_range: f64::from(rank.min_range),
                max_range: f64::from(rank.max_range),
                cost: rage_cost(rank),
                cast: CastConfig {
                    cd,
                    ignore_haste: true,
                    ..CastConfig::default()
                },
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerPummel`.
    pub(super) fn register_pummel(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().pummel.highest();
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                class_spell_mask: masks::PUMMEL,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: cast_config(rank.gcd(), true, cd),
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerHamstring`.
    pub(super) fn register_hamstring(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().hamstring.highest();
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                class_spell_mask: masks::HAMSTRING,
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: cast_config(rank.gcd(), true, Cooldown::default()),
                damage_multiplier: 1.0,
                threat_multiplier: 1.25,
                flat_threat_bonus: if sim.rules == Rules::Forever {
                    0.0
                } else {
                    HAMSTRING_REFERENCE_FLAT_THREAT
                },
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerDisarm`.
    pub(super) fn register_disarm(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().disarm.highest();
        let action = spell_action(rank.id);
        let label = format!("Disarm-{}", sim.unit(unit).label);
        let auras = new_enemy_aura_array(sim, |sim, target| {
            sim.get_or_register_aura(
                target,
                AuraConfig {
                    label: label.clone(),
                    action_id: Some(action.clone()),
                    duration: rank.duration(),
                    ..AuraConfig::default()
                },
            )
        });
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        let related = aura_array_to_map(sim, &auras);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: action,
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: SpellFlag::APL,
                class_spell_mask: masks::DISARM,
                max_range: f64::from(rank.max_range),
                cost: rage_cost_with_refund(rank),
                cast: cast_config(rank.gcd(), true, cd),
                threat_multiplier: 1.0,
                has_extra_cast_condition: true,
                related_aura_arrays: related,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerTaunt`.
    pub(super) fn register_taunt(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().taunt.highest();
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Magic,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::APL,
                class_spell_mask: masks::TAUNT,
                max_range: f64::from(rank.max_range),
                cast: CastConfig {
                    cd,
                    ..non_empty_cast(true)
                },
                threat_multiplier: 1.0,
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerRend`.
    pub(super) fn register_rend(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().rend.highest();
        let tick = rank.periodic_effect();
        let tick_length = tick.period();
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                class_spell_mask: masks::REND,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: SpellFlag::NO_ON_CAST_COMPLETE | SpellFlag::APL,
                cost: rage_cost_with_refund(rank),
                cast: cast_config(rank.gcd(), true, Cooldown::default()),
                has_extra_cast_condition: true,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                dot: DotConfig {
                    aura: AuraConfig {
                        label: "Rend".to_string(),
                        ..AuraConfig::default()
                    },
                    number_of_ticks: (rank.duration() / tick_length) as i32,
                    tick_length,
                    ..DotConfig::default()
                },
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerSunderArmor`.
    pub(super) fn register_sunder_armor(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().sunder_armor.highest();
        let auras = new_enemy_aura_array(sim, |sim, target| {
            generated::SUNDER_ARMOR.aura(sim, target, true, 0, 0.0)
        });
        let related = aura_array_to_map(sim, &auras);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                class_spell_mask: masks::SUNDER_ARMOR,
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: cast_config(rank.gcd(), true, Cooldown::default()),
                has_extra_cast_condition: true,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                flat_threat_bonus: rank
                    .find_effect(dbcenums::E_THREAT, 0, 0)
                    .average(character_level()),
                related_aura_arrays: related,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerHeroicStrike`.
    pub(super) fn register_heroic_strike(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().heroic_strike.highest_for(sim.rules);
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::MELEE_MH,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::NO_ON_CAST_COMPLETE,
                class_spell_mask: masks::HEROIC_STRIKE,
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: CastConfig {
                    default_cast: Cast {
                        non_empty: true,
                        ..Cast::default()
                    },
                    ..CastConfig::default()
                },
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                // Not in the client table (no E_THREAT effect). Patch 184: 2.54 a spell level
                // (152.4 at Rank 9); whole numbers under Forever's rules.
                flat_threat_bonus: heroic_strike_threat(sim.rules, rank),
                ..SpellConfig::default()
            },
        );
        self.make_queue_spells_and_aura(sim, unit, spell);
    }

    /// Go `registerCleave`.
    pub(super) fn register_cleave(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().cleave.highest();
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::MELEE_MH,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::NO_ON_CAST_COMPLETE,
                class_spell_mask: masks::CLEAVE,
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: CastConfig {
                    default_cast: Cast {
                        non_empty: true,
                        ..Cast::default()
                    },
                    ..CastConfig::default()
                },
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                // Not in the client table (no E_THREAT effect). Patch 184: 1.53 a spell level
                // (91.8 at Rank 5), split between the targets the cleave lands on at run time;
                // whole numbers under Forever's rules.
                flat_threat_bonus: cleave_threat(sim.rules, rank),
                ..SpellConfig::default()
            },
        );
        self.make_queue_spells_and_aura(sim, unit, spell);
    }

    /// Go `makeQueueSpellsAndAura`.
    fn make_queue_spells_and_aura(
        &self,
        sim: &mut Sim,
        unit: UnitId,
        source: crate::prepare::sim::SpellId,
    ) {
        let source_action = sim.spell(source).action_id.clone();
        let source_defense = sim.spell(source).defense_type;
        let queue_action = with_tag(&source_action, 1);
        sim.register_aura(
            unit,
            AuraConfig {
                label: format!("HS/Cleave Queue Aura-{}", action_id_string(&source_action)),
                action_id: Some(queue_action.clone()),
                duration: NEVER_EXPIRES,
                on_reset: Some(noop()),
                on_gain: Some(noop()),
                on_expire: Some(noop()),
                ..AuraConfig::default()
            },
        );
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: queue_action,
                spell_school: school::PHYSICAL,
                defense_type: source_defense,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL | SpellFlag::NO_METRICS,
                cast: CastConfig {
                    default_cast: Cast {
                        non_empty: true,
                        ..Cast::default()
                    },
                    ..CastConfig::default()
                },
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerOverpower`.
    pub(super) fn register_overpower(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().overpower.highest();
        let window = spell_data().offensive_state_triggered.highest();
        let cd = new_cooldown(sim, unit, cooldown_of(rank));

        sim.register_aura(
            unit,
            AuraConfig {
                label: "Overpower Aura".to_string(),
                action_id: Some(spell_action(window.id)),
                duration: window.duration(),
                ..AuraConfig::default()
            },
        );
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Overpower - Trigger".to_string(),
                trigger_immediately: true,
                outcome: HitOutcome::DODGE,
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                ..ProcTrigger::default()
            },
        );
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                class_spell_mask: masks::OVERPOWER,
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: cast_config(rank.gcd(), true, cd),
                damage_multiplier: 1.0,
                threat_multiplier: 0.75,
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerSlam`.
    pub(super) fn register_slam(&self, sim: &mut Sim, unit: UnitId) {
        let rank = crate::prepare::hotfix_113554::row(sim.rules, spell_data().slam.highest().id);
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                class_spell_mask: masks::SLAM,
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: CastConfig {
                    default_cast: Cast {
                        gcd: rank.gcd(),
                        cast_time: rank.cast_time(),
                        ..Cast::default()
                    },
                    cd,
                    ignore_haste: true,
                    ..CastConfig::default()
                },
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                flat_threat_bonus: slam_threat(sim.rules, rank),
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerWhirlwind`.
    pub(super) fn register_whirlwind(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().whirlwind.highest();
        let action = spell_action(rank.id);
        // Hotfix 112347: a warrior with an off hand weapon strikes with it too.
        if sim.oh_weapon(unit).is_some() {
            sim.register_spell(
                unit,
                SpellConfig {
                    action_id: with_tag(&action, 2),
                    spell_school: school::PHYSICAL,
                    defense_type: DefenseType::Melee,
                    proc_mask: ProcMask::MELEE_OH_SPECIAL,
                    class_spell_mask: masks::WHIRLWIND_OH,
                    flags: SpellFlag::MELEE_METRICS
                        | SpellFlag::PASSIVE_SPELL
                        | SpellFlag::NO_ON_CAST_COMPLETE,
                    damage_multiplier: 1.0,
                    threat_multiplier: 1.25,
                    ..SpellConfig::default()
                },
            );
        }
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: with_tag(&action, 1),
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                class_spell_mask: masks::WHIRLWIND,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                cost: rage_cost(rank),
                cast: cast_config(rank.gcd(), true, cd),
                damage_multiplier: 1.0,
                threat_multiplier: 1.25,
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerExecute`.
    pub(super) fn register_execute(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().execute.highest();
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                class_spell_mask: masks::EXECUTE,
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: cast_config(rank.gcd(), true, Cooldown::default()),
                damage_multiplier: 1.0,
                threat_multiplier: 1.25,
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerThunderClap`.
    pub(super) fn register_thunder_clap(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().thunder_clap.highest();
        let auras = new_enemy_aura_array(sim, |sim, target| {
            // The clap's bid in the attack speed category slows the target when the aura is
            // activated, which the exporter's `enemy` section reads in a reset simulation.
            generated::THUNDER_CLAP.aura(sim, target, true, 0, 0.0)
        });
        let related = aura_array_to_map(sim, &auras);
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::RANGED_SPECIAL,
                flags: SpellFlag::APL | SpellFlag::BINARY | SpellFlag::IGNORE_RESISTS,
                class_spell_mask: masks::THUNDER_CLAP,
                cost: rage_cost(rank),
                cast: cast_config(rank.gcd(), true, cd),
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                has_extra_cast_condition: true,
                related_aura_arrays: related,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerRevenge`.
    pub(super) fn register_revenge(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().revenge.highest_for(sim.rules);
        let action = spell_action(rank.id);
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Revenge".to_string(),
                duration: 5 * SECOND,
                action_id: Some(action.clone()),
                ..AuraConfig::default()
            },
        );
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Revenge - Trigger".to_string(),
                trigger_immediately: true,
                outcome: HitOutcome::BLOCK | HitOutcome::DODGE | HitOutcome::PARRY,
                callback: CallbackMask::ON_SPELL_HIT_TAKEN,
                ..ProcTrigger::default()
            },
        );
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: action,
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                class_spell_mask: masks::REVENGE,
                max_range: MELEE_RANGE,
                cast: cast_config(rank.gcd(), true, cd),
                cost: rage_cost_with_refund(rank),
                damage_multiplier: 1.0,
                // Patch 183: 2.25 times its damage with no flat bonus (it was damage x1 plus 120).
                threat_multiplier: REVENGE_THREAT_MULTIPLIER,
                flat_threat_bonus: 0.0,
                has_extra_cast_condition: true,
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerShieldBlock`.
    pub(super) fn register_shield_block(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().shield_block.highest();
        let action = spell_action(rank.id);
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Shield Block".to_string(),
                action_id: Some(action.clone()),
                duration: rank.duration(),
                max_stacks: i32::from(rank.proc_charges),
                ..AuraConfig::default()
            },
        );
        sim.attach_stat_buff(
            aura,
            Stat::BlockPercent,
            rank.effect(dbcenums::A_MOD_BLOCK_PERCENT, 0).base_value(),
        );
        sim.attach_proc_trigger(
            aura,
            &ProcTrigger {
                name: "Shield Block - Consume".to_string(),
                trigger_immediately: true,
                outcome: HitOutcome::BLOCK,
                callback: CallbackMask::ON_SPELL_HIT_TAKEN,
                ..ProcTrigger::default()
            },
        );
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: action,
                spell_school: school::PHYSICAL,
                class_spell_mask: masks::SHIELD_BLOCK,
                flags: SpellFlag::APL | SpellFlag::HELPFUL,
                cost: rage_cost(rank),
                cast: CastConfig {
                    cd,
                    ..non_empty_cast(true)
                },
                has_extra_cast_condition: true,
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
        deactivate_without_shield(sim, unit, aura);
    }

    /// Go `registerShieldBash`.
    pub(super) fn register_shield_bash(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().shield_bash.highest();
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                class_spell_mask: masks::SHIELD_BASH,
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: shield_strike_proc_mask(sim.rules),
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                max_range: MELEE_RANGE,
                cost: rage_cost_with_refund(rank),
                cast: cast_config(rank.gcd(), true, cd),
                damage_multiplier: 1.0,
                threat_multiplier: if sim.rules == Rules::Forever {
                    SHIELD_BASH_THREAT_MULTIPLIER
                } else {
                    1.0
                },
                flat_threat_bonus: 0.0,
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerMockingBlow`.
    pub(super) fn register_mocking_blow(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().mocking_blow.highest();
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                class_spell_mask: masks::MOCKING_BLOW,
                max_range: f64::from(rank.max_range),
                cost: rage_cost_with_refund(rank),
                cast: cast_config(rank.gcd(), true, cd),
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerVictoryRush`.
    pub(super) fn register_victory_rush(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().victory_rush.highest();
        let victorious = spell_data().victory_rush_triggered.highest();
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Victorious".to_string(),
                action_id: Some(spell_action(victorious.id)),
                duration: victorious.duration(),
                ..AuraConfig::default()
            },
        );
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                class_spell_mask: masks::VICTORY_RUSH,
                max_range: f64::from(rank.max_range),
                cast: cast_config(rank.gcd(), true, cd),
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                has_extra_cast_condition: true,
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
    }
}

/// `warrior.AddMajorCooldown(core.MajorCooldown{Spell: spell, Type: cooldown_type})`.
pub(super) fn add_cooldown(
    sim: &mut Sim,
    unit: UnitId,
    spell: crate::prepare::sim::SpellId,
    cooldown_type: u32,
) {
    sim.add_major_cooldown(
        unit,
        MajorCooldown {
            spell,
            priority: 0,
            cooldown_type,
            allow_spell_queueing: false,
            timings: Vec::new(),
        },
    );
}

/// Go `registerDashAura`: the aura a Charge or Intercept leaves while the warrior runs, whose
/// gain multiplies the movement speed and whose expiry restores it and runs the dash's end.
fn register_dash_aura(
    sim: &mut Sim,
    unit: UnitId,
    label: &str,
    action: &crate::contracts::prepared_v2::ActionId,
    duration: crate::prepare::sim::Duration,
) {
    sim.register_aura(
        unit,
        AuraConfig {
            label: label.to_string(),
            action_id: Some(action.clone()),
            duration,
            on_gain: Some(Rc::new(|sim: &mut Sim, aura| {
                let unit = sim.aura(aura).unit;
                sim.multiply_movement_speed(unit, 3.0);
            })),
            on_expire: Some(Rc::new(|sim: &mut Sim, aura| {
                let unit = sim.aura(aura).unit;
                sim.multiply_movement_speed(unit, 1.0 / 3.0);
            })),
            ..AuraConfig::default()
        },
    );
}

/// Go `deactivateWithoutShield`: the aura falls at an off hand swap that leaves no shield.
fn deactivate_without_shield(sim: &mut Sim, unit: UnitId, aura: crate::prepare::sim::AuraId) {
    sim.register_item_swap_callback(
        unit,
        &[crate::prepare::items::slot::OFF_HAND],
        crate::prepare::item_swap::SwapCallback::WithoutShield { aura },
    );
}
