//! Slice and Dice, from Go sim/rogue/slice_and_dice.go: a finisher whose metrics split by the
//! combo points spent. It applies the finisher, then restarts its aura with the duration of
//! the points spent; the aura multiplies melee speed while it lasts.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId};

use super::finisher::Finisher;

#[derive(Clone, Debug)]
pub(crate) struct SliceAndDice {
    pub(crate) aura: AuraRef,
    /// The duration at each combo point count, zero to five.
    pub(crate) durations: Vec<i64>,
    pub(crate) melee_speed_multiplier: f64,
}

impl SliceAndDice {
    pub(crate) fn apply<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        finisher: &Finisher,
    ) {
        let points = fight.energy_bar().combo_points as usize;
        finisher.apply(fight, spell);
        fight.deactivate_aura(self.aura);
        fight.aura_mut(self.aura).duration = self.durations[points];
        fight.activate_aura(self.aura);
    }

    /// The aura's OnGain: Go reads the bonus on gain and undoes the same multiplier on expiry.
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_melee_speed(self.melee_speed_multiplier);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_melee_speed(1.0 / self.melee_speed_multiplier);
    }
}
