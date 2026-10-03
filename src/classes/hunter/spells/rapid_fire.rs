//! Rapid Fire (3045), from Go sim/hunter/rapid_fire.go: while its aura is down the cast
//! activates it, and the aura multiplies ranged and melee attack speed (Go
//! `MultiplyAttackSpeed`) until it fades. As a major cooldown it activates only while the aura
//! is down.

use crate::core::fight::{Agent, AuraRef, Fight};

/// The bound Rapid Fire.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RapidFire {
    pub(crate) aura: AuraRef,
    haste: f64,
}

impl RapidFire {
    pub(crate) fn bind<A: Agent>(fight: &Fight<A>, aura: &str, haste: f64) -> Result<Self, String> {
        Ok(RapidFire {
            aura: fight.player_aura(aura)?,
            haste,
        })
    }

    pub(crate) fn changed<A: Agent>(&self, fight: &mut Fight<A>, gained: bool) {
        fight.multiply_attack_speed(if gained { self.haste } else { 1.0 / self.haste });
    }
}
