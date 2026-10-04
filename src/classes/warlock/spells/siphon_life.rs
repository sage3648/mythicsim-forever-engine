//! Siphon Life (18881), from Go sim/warlock/siphon_life.go: Corruption's shape, a binary
//! shadow dot whose every tick heals the warlock for the damage it dealt.

use crate::core::fight::{Agent, DotId, Fight};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SiphonLife {
    pub(crate) dot: DotId,
    /// Go `PseudoStats.SelfHealingMultiplier`, which no aura changes in scope.
    self_healing_multiplier: f64,
    health_metrics: usize,
}

impl SiphonLife {
    pub(crate) fn new(dot: DotId, self_healing_multiplier: f64, health_metrics: usize) -> Self {
        Self {
            dot,
            self_healing_multiplier,
            health_metrics,
        }
    }

    /// `OnTick`: the snapshot tick, then the heal.
    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>) {
        let result = fight.snapshot_dot_tick_result(self.dot);
        fight.gain_health(
            result.damage * self.self_healing_multiplier,
            self.health_metrics,
        );
    }
}
