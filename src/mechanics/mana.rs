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

/// The inputs of Go's mana regeneration rates (sim/core/mana.go).
#[derive(Clone, Copy, Debug)]
pub(crate) struct RegenInputs {
    pub(crate) mp5: f64,
    pub(crate) spirit_regen_per_second: f64,
    pub(crate) spirit_regen_rate_casting: f64,
    pub(crate) force_full_spirit_regen: bool,
    pub(crate) spirit_regen_multiplier: f64,
    pub(crate) mana_regen_multiplier: f64,
}

/// Go `ManaRegenPerSecondWhileCasting`, operation for operation.
pub(crate) fn regen_per_second_casting(inputs: RegenInputs) -> f64 {
    let mut regen = inputs.mp5 / 5.0;
    let mut spirit = 0.0;
    if inputs.spirit_regen_rate_casting != 0.0 || inputs.force_full_spirit_regen {
        spirit = inputs.spirit_regen_per_second * inputs.spirit_regen_multiplier;
        if !inputs.force_full_spirit_regen {
            spirit *= inputs.spirit_regen_rate_casting;
        }
    }
    regen += spirit;
    regen * inputs.mana_regen_multiplier
}

/// Go `ManaRegenPerSecondWhileNotCasting`.
pub(crate) fn regen_per_second_not_casting(inputs: RegenInputs) -> f64 {
    let mut regen = inputs.mp5 / 5.0;
    regen += inputs.spirit_regen_per_second * inputs.spirit_regen_multiplier;
    regen * inputs.mana_regen_multiplier
}
