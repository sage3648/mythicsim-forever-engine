//! Adrenaline Rush (13750), from Go sim/rogue/talents_combat.go `registerAdrenalineRush`:
//! the cast activates an aura that multiplies energy regeneration while it lasts. As a major
//! cooldown it fires only at or below an energy threshold.

use crate::core::fight::{Agent, AuraRef, Fight};

#[derive(Clone, Copy, Debug)]
pub(crate) struct AdrenalineRush {
    pub(crate) aura: AuraRef,
    pub(crate) regen_multiplier: f64,
    pub(crate) energy_threshold: f64,
}

impl AdrenalineRush {
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
    }

    /// Go `MajorCooldown.ShouldActivate`.
    pub(crate) fn should_activate<A: Agent>(&self, fight: &Fight<A>) -> bool {
        fight.energy_bar().current <= self.energy_threshold
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_energy_regen_speed(self.regen_multiplier);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_energy_regen_speed(1.0 / self.regen_multiplier);
    }
}
