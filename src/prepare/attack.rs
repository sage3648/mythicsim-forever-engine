//! Go sim/core/attack.go: weapons, auto attacks and attack tables at construction.

use crate::contracts::prepared_v2::ActionId;

use super::character::constants::{
    DEFAULT_ATTACK_POWER_PER_DPS, MAX_MELEE_RANGE, MIN_RANGED_RANGE,
};
use super::character::unit_level_f64;
use super::items::Item;
use super::sim::{AuraConfig, EventCallbacks, Sim, SpellId, UnitId, UnitType, NEVER_EXPIRES};
use super::spell::{school, DefenseType, ProcMask, SpellConfig, SpellFlag};
use std::rc::Rc;

/// Go `Weapon`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Weapon {
    pub base_damage_min: f64,
    pub base_damage_max: f64,
    pub attack_power_per_dps: f64,
    pub swing_speed: f64,
    pub normalized_swing_speed: f64,
    pub spell_school: u8,
    pub min_range: f64,
    pub max_range: f64,
}

impl Weapon {
    /// Go `newWeaponFromUnarmed`.
    pub fn unarmed() -> Weapon {
        Weapon {
            swing_speed: 1.0,
            normalized_swing_speed: 1.0,
            attack_power_per_dps: DEFAULT_ATTACK_POWER_PER_DPS,
            max_range: MAX_MELEE_RANGE,
            ..Weapon::default()
        }
    }

    /// Go `newWeaponFromItem`.
    pub fn from_item(item: &Item, bonus_dps: f64) -> Weapon {
        let normalized = if item.weapon_type == "WeaponTypeDagger" {
            1.7
        } else if item.hand_type == "HandTypeTwoHand" {
            3.3
        } else if item.ranged_weapon_type != "RangedWeaponTypeUnknown" {
            2.8
        } else {
            2.4
        };
        let (min_range, max_range) = match item.ranged_weapon_type.as_str() {
            "RangedWeaponTypeUnknown" => (0.0, MAX_MELEE_RANGE),
            "RangedWeaponTypeWand" => (0.0, 30.0),
            "RangedWeaponTypeThrown" => (MIN_RANGED_RANGE, 30.0),
            _ => (MIN_RANGED_RANGE, 35.0),
        };
        Weapon {
            // Go fuses the bonus into one multiply-add (attack.go 81, 82).
            base_damage_min: bonus_dps.mul_add(
                item.swing_speed,
                item.weapon_damage_min + item.enchant.weapon_damage,
            ),
            base_damage_max: bonus_dps.mul_add(
                item.swing_speed,
                item.weapon_damage_max + item.enchant.weapon_damage,
            ),
            swing_speed: item.swing_speed,
            normalized_swing_speed: normalized,
            attack_power_per_dps: DEFAULT_ATTACK_POWER_PER_DPS,
            min_range,
            max_range,
            spell_school: school::NONE,
        }
    }

    /// Go `GetSpellSchool`.
    pub fn school(&self) -> u8 {
        if self.spell_school == school::NONE {
            school::PHYSICAL
        } else {
            self.spell_school
        }
    }
}

/// Go `AutoAttacks`.
#[derive(Clone, Default)]
pub(crate) struct AutoAttacks {
    pub enabled: bool,
    pub auto_swing_melee: bool,
    pub auto_swing_ranged: bool,
    pub is_dual_wielding: bool,
    pub mh: Weapon,
    pub oh: Weapon,
    pub ranged: Weapon,
    pub mh_config: Option<SpellConfig>,
    pub oh_config: Option<SpellConfig>,
    pub ranged_config: Option<SpellConfig>,
    pub mh_spell: Option<SpellId>,
    pub oh_spell: Option<SpellId>,
    pub ranged_spell: Option<SpellId>,
    pub replace_mh_swing: bool,
}

/// Go `AutoAttackOptions`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AutoAttackOptions {
    pub main_hand: Weapon,
    pub off_hand: Weapon,
    pub ranged: Weapon,
    pub auto_swing_melee: bool,
    pub auto_swing_ranged: bool,
    pub replace_mh_swing: bool,
    pub proc_mask: ProcMask,
}

impl Sim {
    /// Go `unit.EnableAutoAttacks`.
    pub(crate) fn enable_auto_attacks(&mut self, unit: UnitId, mut options: AutoAttackOptions) {
        if options.main_hand.attack_power_per_dps == 0.0 {
            options.main_hand.attack_power_per_dps = DEFAULT_ATTACK_POWER_PER_DPS;
        }
        if options.off_hand.attack_power_per_dps == 0.0 {
            options.off_hand.attack_power_per_dps = DEFAULT_ATTACK_POWER_PER_DPS;
        }
        let mask = |default: ProcMask| {
            if options.proc_mask == ProcMask::UNKNOWN {
                default
            } else {
                options.proc_mask
            }
        };
        let melee_flags = SpellFlag::MELEE_METRICS
            | SpellFlag::INCLUDE_TARGET_BONUS_DAMAGE
            | SpellFlag::NO_ON_CAST_COMPLETE;
        let config = |action: ActionId,
                      school: u8,
                      defense: DefenseType,
                      mask: ProcMask,
                      flags: SpellFlag| SpellConfig {
            action_id: action,
            spell_school: school,
            defense_type: defense,
            proc_mask: mask,
            flags,
            damage_multiplier: 1.0,
            damage_multiplier_additive: 1.0,
            threat_multiplier: 1.0,
            bonus_coefficient: 1.0,
            ..Default::default()
        };
        let attack = |tag| ActionId {
            other_id: "OtherActionAttack".to_string(),
            tag,
            ..ActionId::default()
        };
        let mh_config = config(
            attack(1),
            options.main_hand.school(),
            DefenseType::Melee,
            mask(ProcMask::MELEE_MH_AUTO),
            melee_flags,
        );
        let oh_config = config(
            attack(2),
            options.off_hand.school(),
            DefenseType::Melee,
            mask(ProcMask::MELEE_OH_AUTO),
            melee_flags,
        );
        let mut ranged_config = config(
            ActionId {
                other_id: "OtherActionShoot".to_string(),
                ..ActionId::default()
            },
            options.ranged.school(),
            DefenseType::Ranged,
            mask(ProcMask::RANGED_AUTO),
            SpellFlag::MELEE_METRICS | SpellFlag::INCLUDE_TARGET_BONUS_DAMAGE,
        );
        ranged_config.missile_speed = 40.0;
        // Go reads the ranges from the auto attacks it has just assigned.
        ranged_config.min_range = options.ranged.min_range;
        ranged_config.max_range = options.ranged.max_range;
        self.unit_mut(unit).auto_attacks = AutoAttacks {
            enabled: true,
            auto_swing_melee: options.auto_swing_melee,
            auto_swing_ranged: options.auto_swing_ranged,
            is_dual_wielding: options.off_hand.swing_speed != 0.0,
            mh: options.main_hand,
            oh: options.off_hand,
            ranged: options.ranged,
            mh_config: Some(mh_config),
            oh_config: Some(oh_config),
            ranged_config: Some(ranged_config),
            mh_spell: None,
            oh_spell: None,
            ranged_spell: None,
            replace_mh_swing: options.replace_mh_swing,
        };
    }

    /// Go `AutoAttacks.finalize`.
    pub(crate) fn finalize_auto_attacks(&mut self, unit: UnitId) {
        let aa = self.unit(unit).auto_attacks.clone();
        if aa.auto_swing_melee {
            let mh = self.get_or_register_spell(unit, aa.mh_config.clone().expect("enabled"));
            let oh = self.get_or_register_spell(unit, aa.oh_config.clone().expect("enabled"));
            let aa = &mut self.unit_mut(unit).auto_attacks;
            aa.mh_spell = Some(mh);
            aa.oh_spell = Some(oh);
        }
        if aa.auto_swing_ranged {
            let ranged =
                self.get_or_register_spell(unit, aa.ranged_config.clone().expect("enabled"));
            self.unit_mut(unit).auto_attacks.ranged_spell = Some(ranged);
        }
    }

    /// Go `unit.applyParryHaste`.
    pub(crate) fn apply_parry_haste(&mut self, unit: UnitId) {
        if !self.unit(unit).pseudo_stats.parry_haste
            || !self.unit(unit).auto_attacks.auto_swing_melee
        {
            return;
        }
        self.register_aura(
            unit,
            AuraConfig {
                label: "Parry Haste".to_string(),
                duration: NEVER_EXPIRES,
                on_reset: Some(Rc::new(|sim: &mut Sim, aura| sim.activate(aura))),
                events: EventCallbacks {
                    on_spell_hit_taken: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
    }

    /// Go `unit.initMovement`.
    pub(crate) fn init_movement(&mut self, unit: UnitId) {
        let noop: super::sim::AuraCallback = Rc::new(|_: &mut Sim, _| {});
        self.get_or_register_aura(
            unit,
            AuraConfig {
                label: "Movement".to_string(),
                action_id: Some(ActionId {
                    other_id: "OtherActionMove".to_string(),
                    ..ActionId::default()
                }),
                duration: NEVER_EXPIRES,
                max_stacks: 100,
                on_gain: Some(noop.clone()),
                on_expire: Some(noop),
                ..Default::default()
            },
        );
        self.get_or_register_spell(
            unit,
            SpellConfig {
                action_id: ActionId {
                    other_id: "OtherActionMove".to_string(),
                    ..ActionId::default()
                },
                flags: SpellFlag::MELEE_METRICS,
                ..Default::default()
            },
        );
    }
}

/// Go `AttackTable` fields the export reads.
#[derive(Clone, Debug, Default)]
pub(crate) struct AttackTable {
    pub base_miss_chance: f64,
    pub base_spell_miss_chance: f64,
    pub base_block_chance: f64,
    pub base_dodge_chance: f64,
    pub base_parry_chance: f64,
    pub base_glance_chance: f64,
    pub base_crush_chance: f64,
    pub glance_multiplier: f64,
    pub glance_spread: f64,
    pub melee_crit_suppression: f64,
    pub spell_crit_suppression: f64,
    pub hit_suppression: f64,
    pub crit_multiplier: f64,
    pub damage_dealt_multiplier: f64,
    pub damage_taken_multiplier: f64,
    pub healing_dealt_multiplier: f64,
    pub ignore_armor: bool,
    pub armor_ignore_factor: f64,
    pub bonus_spell_crit_percent: f64,
    pub ranged_damage_taken_multiplier: f64,
    pub damage_done_by_caster: bool,
    /// Go `MobTypeBonusStats`: stats against mobs of a type, by `proto.MobType` name. The
    /// export does not list them; the fight reads them.
    pub mob_type_bonus_stats: std::collections::BTreeMap<String, super::stats::Stats>,
}

/// Go `NewAttackTable`.
pub(crate) fn new_attack_table(
    attacker_level: i32,
    defender_type: UnitType,
    defender_level: i32,
) -> AttackTable {
    let mut table = AttackTable {
        crit_multiplier: 1.0,
        damage_dealt_multiplier: 1.0,
        damage_taken_multiplier: 1.0,
        ranged_damage_taken_multiplier: 1.0,
        healing_dealt_multiplier: 1.0,
        ..AttackTable::default()
    };
    let level = defender_level;
    if defender_type == UnitType::Enemy {
        table.base_spell_miss_chance = unit_level_f64(level, 0.02, 0.04, 0.05, 0.06, 0.17);
        table.base_miss_chance = unit_level_f64(level, 0.04, 0.05, 0.055, 0.06, 0.08);
        table.base_block_chance = 0.05;
        table.base_dodge_chance = unit_level_f64(level, 0.04, 0.05, 0.055, 0.06, 0.065);
        table.base_parry_chance = unit_level_f64(level, 0.04, 0.05, 0.055, 0.06, 0.14);
        table.base_glance_chance = unit_level_f64(level, 0.0, 0.10, 0.20, 0.30, 0.40);
        table.glance_multiplier = unit_level_f64(level, 0.95, 0.95, 0.95, 0.85, 0.65);
        table.glance_spread = unit_level_f64(level, 0.04, 0.04, 0.04, 0.05, 0.10);
        table.hit_suppression = unit_level_f64(level, 0.0, 0.0, 0.0, 0.0, 0.01);
        table.melee_crit_suppression = unit_level_f64(level, 0.0, 0.0, 0.01, 0.02, 0.048);
    } else {
        let level = attacker_level;
        table.base_spell_miss_chance = 0.05;
        table.base_miss_chance = unit_level_f64(level, 0.054, 0.05, 0.048, 0.046, 0.044);
        table.base_block_chance = unit_level_f64(level, 0.054, 0.05, 0.048, 0.046, 0.044);
        table.base_dodge_chance = unit_level_f64(level, 0.004, 0.0, -0.002, -0.004, -0.006);
        table.base_parry_chance = unit_level_f64(level, 0.054, 0.05, 0.048, 0.046, 0.044);
        table.base_crush_chance = unit_level_f64(level, 0.0, 0.0, 0.0, 0.0, 0.15);
    }
    table
}
