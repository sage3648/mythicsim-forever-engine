//! Rage of the Farseer (talent 425336), from Go sim/shaman/talents_enhancement.go
//! `applyRageOfTheFarseer`: a major cooldown whose aura multiplies melee speed.

use crate::core::fight::{Agent, AuraRef, Fight};

#[derive(Clone, Copy, Debug)]
pub(crate) struct RageOfTheFarseer {
    pub(crate) aura: AuraRef,
    speed: f64,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    aura: &str,
    speed: f64,
) -> Result<RageOfTheFarseer, String> {
    Ok(RageOfTheFarseer {
        aura: fight.player_aura(aura)?,
        speed,
    })
}

impl RageOfTheFarseer {
    /// The aura's `AttachMultiplyMeleeSpeed`.
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_melee_speed(self.speed);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_melee_speed(1.0 / self.speed);
    }
}
