//! Berserker Rage (18499), from Go sim/warrior/berserker_rage.go: Improved Berserker Rage's
//! rage, then an aura that doubles rage from damage taken, a Go literal. Its survival major
//! cooldown fires only when the rage fits.

use crate::core::fight::{Agent, AuraRef, Fight};

/// berserker_rage.go `berserkerRageDamageTakenRageMultiplier`.
pub(crate) const DAMAGE_TAKEN_RAGE_MULTIPLIER: f64 = 2.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct BerserkerRage {
    pub(crate) aura: AuraRef,
    pub(crate) rage_gain: f64,
    /// Go `NewRageMetrics(actionID)`.
    pub(crate) metrics: usize,
}

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, params: BerserkerRage) {
    if params.rage_gain > 0.0 {
        fight.add_rage(params.rage_gain, params.metrics);
    }
    fight.activate_aura(params.aura);
}

/// The major cooldown's ShouldActivate.
pub(crate) fn should_activate<A: Agent>(fight: &Fight<A>, params: BerserkerRage) -> bool {
    params.rage_gain > 0.0 && fight.current_rage() + params.rage_gain <= fight.maximum_rage()
}

/// The aura's OnGain and OnExpire.
pub(crate) fn on_gain<A: Agent>(fight: &mut Fight<A>) {
    fight.multiply_damage_taken_rage(DAMAGE_TAKEN_RAGE_MULTIPLIER);
}

pub(crate) fn on_expire<A: Agent>(fight: &mut Fight<A>) {
    fight.multiply_damage_taken_rage(1.0 / DAMAGE_TAKEN_RAGE_MULTIPLIER);
}
