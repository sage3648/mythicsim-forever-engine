//! Go sim/core/debuffs.go and sim/core/buffs/debuffs.go: the raid's debuffs on each target, and
//! the helpers classes build their own debuffs with.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::contracts::request::Message;

use super::env::Environment;
use super::periodic_action::PeriodicActionOptions;
use super::sim::{
    AuraConfig, AuraId, Duration, EffectCallback, EffectId, Sim, UnitId, NEVER_EXPIRES, SECOND,
};
use super::stats::{Stat, Stats};
use super::Refusal;

/// Go `applyDebuffEffects`, by way of `buffs.applyDebuffs`: the generated rows, then the one the
/// client states in a shape no manifest row can carry.
pub(crate) fn apply_debuff_effects(
    env: &mut Environment,
    target: UnitId,
    _index: usize,
    debuffs: &Message,
    raid: &Message,
) -> Result<(), Refusal> {
    super::buffs::apply_generated_debuffs(env, target, debuffs, raid)?;
    if debuffs.bool("judgement_of_the_crusader") {
        let aura = super::buffs::paladin::judgement_of_the_crusader_aura(
            &mut env.sim,
            target,
            &super::buffs::paladin::judgement_of_the_crusader_max_rank(),
        );
        env.sim.make_permanent(aura);
    }
    Ok(())
}

/// Go `ScheduledAura`: replaces the aura's reset with one that starts the periodic action, so
/// the aura is not up until the action activates it. The action only runs in a fight.
pub(crate) fn scheduled_aura(sim: &mut Sim, aura: AuraId, options: PeriodicActionOptions) {
    sim.aura_mut(aura).on_reset = Some(Rc::new(move |sim: &mut Sim, aura: AuraId| {
        sim.aura_mut(aura).duration = NEVER_EXPIRES;
        sim.start_periodic_action(options);
    }));
}

/// Go `SlowAura`.
#[allow(dead_code)]
pub(crate) fn slow_aura(sim: &mut Sim, target: UnitId) -> AuraId {
    cast_slow_reduction_aura(sim, target, "Slow", 31589, 1.5, 15 * SECOND)
}

/// Go `castSlowReductionAura`.
#[allow(dead_code)]
pub(crate) fn cast_slow_reduction_aura(
    sim: &mut Sim,
    target: UnitId,
    label: &str,
    spell_id: i32,
    multiplier: f64,
    duration: Duration,
) -> AuraId {
    let aura = sim.get_or_register_aura(
        target,
        AuraConfig {
            label: label.to_string(),
            action_id: Some(ActionId::spell(spell_id)),
            duration,
            ..AuraConfig::default()
        },
    );
    // How far from 1 the applied factor is, which is the scale every member of the category
    // bids on: a 33% slow outbids a 20% one. The factor is 1/multiplier, so a caller stating 1.5
    // is a 33.3% slow.
    sim.new_exclusive_effect(
        aura,
        "CastSpdReduction",
        false,
        1.0 - 1.0 / multiplier,
        Some(speed_callback(target, 1.0 / multiplier, true)),
        Some(speed_callback(target, multiplier, true)),
    );
    aura
}

/// A slow on casts alone, in the category Slow's cast and ranged slow takes: only the strongest
/// applies. Go `CastSpeedReductionEffect`.
#[allow(dead_code)]
pub(crate) fn cast_speed_reduction_effect(
    sim: &mut Sim,
    aura: AuraId,
    cast_time_multiplier: f64,
) -> EffectId {
    let unit = sim.aura(aura).unit;
    sim.new_exclusive_effect(
        aura,
        "CastSpdReduction",
        false,
        1.0 - 1.0 / cast_time_multiplier,
        Some(cast_speed_callback(unit, 1.0 / cast_time_multiplier)),
        Some(cast_speed_callback(unit, cast_time_multiplier)),
    )
}

/// Go `AtkSpeedReductionEffect`: a 20% slow outbids a 10% one. The factor is
/// 1/speedMultiplier, so a helper stating 1.2 is a 16.67% slow.
#[allow(dead_code)]
pub(crate) fn atk_speed_reduction_effect(
    sim: &mut Sim,
    aura: AuraId,
    speed_multiplier: f64,
) -> EffectId {
    let unit = sim.aura(aura).unit;
    sim.new_exclusive_effect(
        aura,
        "AtkSpdReduction",
        false,
        1.0 - 1.0 / speed_multiplier,
        Some(Rc::new(move |sim: &mut Sim, _: EffectId| {
            sim.multiply_attack_speed(unit, 1.0 / speed_multiplier);
        })),
        Some(Rc::new(move |sim: &mut Sim, _: EffectId| {
            sim.multiply_attack_speed(unit, speed_multiplier);
        })),
    )
}

/// The callback of `castSlowReductionAura`: casts and ranged attacks slowed together.
fn speed_callback(unit: UnitId, factor: f64, ranged: bool) -> EffectCallback {
    Rc::new(move |sim: &mut Sim, _: EffectId| {
        sim.multiply_cast_speed(unit, factor);
        if ranged {
            sim.multiply_ranged_speed(unit, factor);
        }
    })
}

fn cast_speed_callback(unit: UnitId, factor: f64) -> EffectCallback {
    speed_callback(unit, factor, false)
}

/// Go `ScreechAura`.
#[allow(dead_code)]
pub(crate) fn screech_aura(sim: &mut Sim, target: UnitId) -> AuraId {
    let mut stats = Stats::default();
    stats[Stat::AttackPower] = -210.0;
    stats_debuff(sim, target, 0, "Screech", 27051, stats, 4 * SECOND)
}

/// Go `SlowedTimeMultiplier`: a slow the client states as a negative speed percentage makes the
/// time between attacks, or a cast time, that much longer: -20 is 20% longer, which divides the
/// speed by 1.2.
#[allow(dead_code)]
pub(crate) fn slowed_time_multiplier(speed_percent: f64) -> f64 {
    1.0 - speed_percent / 100.0
}

/// Go `statsDebuff`.
fn stats_debuff(
    sim: &mut Sim,
    target: UnitId,
    caster_index: i32,
    label: &str,
    spell_id: i32,
    stats: Stats,
    duration: Duration,
) -> AuraId {
    let duration = if duration == 0 { 30 * SECOND } else { duration };
    let mut action_id = ActionId::spell(spell_id);
    if caster_index != 0 {
        action_id.tag = caster_index;
    }
    // Go `GetAuraByID`: the first aura of the same action, tag included.
    let existing = sim
        .unit(target)
        .auras
        .iter()
        .copied()
        .find(|id| sim.aura(*id).action_id.as_ref() == Some(&action_id));
    if let Some(existing) = existing {
        return existing;
    }
    let aura = sim.register_aura(
        target,
        AuraConfig {
            label: label.to_string(),
            action_id: Some(action_id),
            duration,
            ..AuraConfig::default()
        },
    );
    sim.attach_stats_buff(aura, stats)
}
