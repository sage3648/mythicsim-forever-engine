//! Blade Flurry (13877), from Go sim/rogue/talents_combat.go `registerBladeFlurry`: the cast
//! activates an aura that multiplies attack speed while it lasts. Its extra hit on a second
//! target never fires against the one target in scope, and its listener returns before any
//! draw.

use crate::core::fight::{Agent, AuraRef, Fight};

#[derive(Clone, Copy, Debug)]
pub(crate) struct BladeFlurry {
    pub(crate) aura: AuraRef,
    pub(crate) attack_speed_multiplier: f64,
}

impl BladeFlurry {
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
    }

    /// Go `AttachMultiplyAttackSpeed`.
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_attack_speed(self.attack_speed_multiplier);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_attack_speed(1.0 / self.attack_speed_multiplier);
    }
}
