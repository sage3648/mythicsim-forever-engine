//! Devouring Plague's ticks, from Go sim/priest/devouring_plague.go: each heals the priest
//! for the damage it deals, under the rank's action ID with a tag. The cast is in
//! `periodic`.

use crate::core::fight::{Agent, DotId, Fight};

/// One tick: the snapshot damage, then the heal.
pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, dot: DotId, health_metrics: usize) {
    let result = fight.snapshot_dot_tick_result(dot);
    fight.gain_health(result.damage, health_metrics);
}
