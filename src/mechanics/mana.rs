//! Mana ticks, spending and the five-second rule for the current static model.

use crate::{contracts::Caster, core::time::SECOND};

pub(crate) fn regenerate(caster: &Caster, mana: f64, time: u64, five_second_rule: u64) -> f64 {
    let regen = if time < five_second_rule {
        caster.regen_casting_per_second
    } else {
        caster.regen_idle_per_second
    };
    (mana + 2.0 * regen).min(caster.max_mana)
}

pub(crate) fn spend(mana: &mut f64, cost: f64, time: u64, five_second_rule: &mut u64) {
    *mana -= cost;
    if cost > 0.0 {
        *five_second_rule = time + 5 * SECOND;
    }
}

/// Preserve the reaction-time grid when only a mana tick can restore readiness.
pub(crate) fn next_ready_after_tick(time: u64, reaction_ns: u64) -> u64 {
    let next_tick = (time / (2 * SECOND) + 1) * 2 * SECOND;
    let polls = (next_tick - time).div_ceil(reaction_ns);
    time + polls * reaction_ns
}
