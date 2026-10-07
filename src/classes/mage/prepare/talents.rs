//! Go sim/mage/talents.go: the Mage's talents, registered in Go's order.

mod arcane;
mod fire;
mod frost;

use crate::prepare::sim::{Duration, Sim, UnitId, MILLISECOND};

use super::Mage;

/// `time.Millisecond * time.Duration(value)`: the float is cut to whole milliseconds.
pub(super) fn millis(value: f64) -> Duration {
    (value as i64).wrapping_mul(MILLISECOND)
}

impl Mage {
    /// Go `Mage.ApplyTalents`.
    pub(super) fn apply_mage_talents(&self, sim: &mut Sim, unit: UnitId) {
        self.register_arcane_talents(sim, unit);
        self.register_fire_talents(sim, unit);
        self.register_frost_talents(sim, unit);
    }
}
