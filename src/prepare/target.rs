//! Go sim/core/target.go and target_ai.go: an encounter target as `NewTarget` builds it.

use crate::contracts::request::Message;

use super::attack::{AutoAttackOptions, Weapon};
use super::character::constants::CHARACTER_LEVEL;
use super::character::{add_rating_conversions, unit_level_f64};
use super::sim::{Sim, Unit, UnitId, UnitType};
use super::spell::school;
use super::stats::{Stat, Stats};
use super::Refusal;

/// Go `BossGCD`.
const BOSS_GCD: i64 = 1_620_000_000;

/// Go `NewTarget`.
pub(crate) fn new_target(sim: &mut Sim, options: &Message, index: i32) -> Result<UnitId, Refusal> {
    if options.has("target_inputs") || options.bool("disabled_at_start") {
        return Err(Refusal::new(
            "target",
            "target inputs and disabled targets are unsupported".to_string(),
        ));
    }
    let mut unit = Unit::new(UnitType::Enemy, format!("Target {}", index + 1));
    unit.index = index;
    unit.level = options.i32("level");
    unit.mob_type = options.enum_name("mob_type");
    unit.stats = Stats::from_proto_array(&options.f64s("stats"));
    unit.reaction_time = BOSS_GCD;
    unit.enabled = true;
    if unit.level == 0 {
        unit.level = CHARACTER_LEVEL + 3;
    }
    let id = sim.add_unit(unit);
    let gcd = sim.new_timer(id);
    let rotation = sim.new_timer(id);
    let level = sim.unit(id).level;
    let u = sim.unit_mut(id);
    u.gcd = Some(gcd);
    u.rotation_timer = Some(rotation);
    u.stats[Stat::PhysicalCritPercent] += unit_level_f64(level, 4.6, 5.0, 5.2, 5.4, 5.6);
    u.stats[Stat::BlockValue] = 54.0;
    add_rating_conversions(sim, id);
    let u = sim.unit_mut(id);
    if u.level == 63 && options.bool("suppress_dodge") {
        u.pseudo_stats.dodge_reduction += 0.2;
        u.pseudo_stats.increased_miss_chance -= 0.05;
    }
    u.pseudo_stats.can_block = true;
    u.pseudo_stats.can_parry = true;
    u.pseudo_stats.can_crush = options.bool("can_crush");
    u.pseudo_stats.parry_haste = options.bool("parry_haste");
    u.pseudo_stats.in_front_of_target = true;
    u.pseudo_stats.damage_spread = options.f64("damage_spread");
    if super::presets::preset_target_has_ai(options.i32("id")) {
        return Err(Refusal::new(
            "target",
            "target AI is unsupported".to_string(),
        ));
    }
    Ok(id)
}

/// Go `Target.initialize`.
pub(crate) fn initialize_target(sim: &mut Sim, target: UnitId, options: &Message) {
    if options.f64("swing_speed") > 0.0 {
        let main_hand = Weapon {
            base_damage_min: options.f64("min_base_damage"),
            swing_speed: options.f64("swing_speed"),
            spell_school: spell_school_from_proto(&options.enum_name("spell_school")),
            ..Weapon::default()
        };
        let mut aa = AutoAttackOptions {
            main_hand,
            auto_swing_melee: true,
            ..AutoAttackOptions::default()
        };
        if options.bool("dual_wield") {
            aa.off_hand = aa.main_hand;
            if !options.bool("dual_wield_penalty") {
                sim.unit_mut(target).pseudo_stats.disable_dw_miss_penalty = true;
            }
        }
        sim.enable_auto_attacks(target, aa);
    }
}

/// Go `SpellSchoolFromProto`.
pub(crate) fn spell_school_from_proto(name: &str) -> u8 {
    match name {
        "SpellSchoolArcane" => school::ARCANE,
        "SpellSchoolFire" => school::FIRE,
        "SpellSchoolFrost" => school::FROST,
        "SpellSchoolHoly" => school::HOLY,
        "SpellSchoolNature" => school::NATURE,
        "SpellSchoolShadow" => school::SHADOW,
        _ => school::PHYSICAL,
    }
}
