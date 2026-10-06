//! Drain Life (11700), from Go sim/warlock/drain_life.go: a channeled shadow dot. Each tick's
//! snapshot damage is scaled by Soul Siphon before it is dealt, then heals the warlock.
//!
//! Go counts Soul Siphon's effects with `GetAurasWithTag`, which lists every registered
//! Affliction aura on the target, active or not, so the multiplier is fixed for a build.

use crate::core::fight::{Agent, DotId, Fight};

#[derive(Clone, Copy, Debug)]
pub(crate) struct DrainLife {
    pub(crate) dot: DotId,
    soul_siphon: f64,
    self_healing_multiplier: f64,
    health_metrics: usize,
}

impl DrainLife {
    pub(crate) fn new(
        dot: DotId,
        soul_siphon: f64,
        self_healing_multiplier: f64,
        health_metrics: usize,
    ) -> Self {
        Self {
            dot,
            soul_siphon,
            self_healing_multiplier,
            health_metrics,
        }
    }

    /// `OnTick` of a target's dot: the snapshot damage, Soul Siphon, the tick dealt, then the
    /// heal.
    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>, dot: DotId) {
        let mut result = fight.snapshot_dot_tick_calc(dot);
        result.damage *= self.soul_siphon;
        let spell = fight.dots[dot].spell;
        fight.deal_damage(spell, result, true);
        fight.gain_health(
            result.damage * self.self_healing_multiplier,
            self.health_metrics,
        );
    }
}
