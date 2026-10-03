//! Bloodrage (2687), from Go sim/warrior/bloodrage.go: instant rage, a share of base health,
//! then rage each period from a periodic action. Its major cooldown waits for rage below a
//! Go literal.

use crate::core::fight::{Agent, Fight, PRIORITY_GCD};

/// The class periodic tag of Bloodrage's rage over time.
pub(crate) const PERIODIC_TAG: u32 = 2;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bloodrage {
    pub(crate) instant_rage: f64,
    pub(crate) rage_per_tick: f64,
    pub(crate) ticks: i32,
    pub(crate) period: i64,
    pub(crate) health_cost: f64,
    pub(crate) rage_threshold: f64,
    /// Go `NewRageMetrics(actionID)`.
    pub(crate) metrics: usize,
}

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, params: Bloodrage) {
    fight.add_rage(params.instant_rage, params.metrics);
    fight.remove_health(params.health_cost);
    fight.start_class_periodic(PERIODIC_TAG, params.period, params.ticks, PRIORITY_GCD);
}

/// A tick of the rage over time.
pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, params: Bloodrage) {
    fight.add_rage(params.rage_per_tick, params.metrics);
}

/// The major cooldown's ShouldActivate.
pub(crate) fn should_activate<A: Agent>(fight: &Fight<A>, params: Bloodrage) -> bool {
    fight.current_rage() < params.rage_threshold
}
