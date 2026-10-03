//! Anger Management (12296), from Go sim/warrior/talents_arms.go `registerAngerManagement`:
//! a reset effect starts a periodic action that grants rage once the fight has begun.

use crate::core::fight::{Agent, Fight, PRIORITY_GCD};

/// The class periodic tag of Anger Management.
pub(crate) const PERIODIC_TAG: u32 = 1;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AngerManagement {
    pub(crate) rage: f64,
    pub(crate) period: i64,
    /// Go `NewRageMetrics(actionID)`.
    pub(crate) metrics: usize,
}

/// The reset effect: ticks forever, a period apart from the reset.
pub(crate) fn reset<A: Agent>(fight: &mut Fight<A>, params: AngerManagement) {
    fight.start_class_periodic(PERIODIC_TAG, params.period, 0, PRIORITY_GCD);
}

pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, params: AngerManagement) {
    if fight.now > 0 {
        fight.add_rage(params.rage, params.metrics);
    }
}
