//! Go sim/priest/talents.go: the Priest's talents, registered in Go's order.

mod discipline;
mod holy;
mod shadow;

use crate::prepare::sim::{Duration, Sim, UnitId, MILLISECOND};

use super::Priest;

/// `time.Millisecond * time.Duration(value)`: the float is cut to whole milliseconds.
pub(super) fn millis(value: f64) -> Duration {
    (value as i64).wrapping_mul(MILLISECOND)
}

impl Priest {
    /// Go `Priest.ApplyTalents`.
    pub(super) fn apply_priest_talents(&mut self, sim: &mut Sim, unit: UnitId) {
        self.register_discipline_talents(sim, unit);
        self.register_holy_talents(sim, unit);
        self.register_shadow_talents(sim, unit);
    }
}
