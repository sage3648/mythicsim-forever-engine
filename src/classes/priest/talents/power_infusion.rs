//! Power Infusion (10060), from Go sim/priest/talents_discipline.go `applyPowerInfusion`: a
//! cooldown the priest casts on itself, activating the client-parsed Power Infusions aura. What
//! the aura multiplies, the damage of the schools its mask names and the healing the priest
//! deals, belongs to every copy of it and is the fight's (`core/fight/power_infusion.rs`).

use crate::core::fight::{AuraRef, Fight};

use super::super::agent::PriestAgent;

#[derive(Clone, Debug)]
pub(crate) struct PowerInfusion {
    pub(crate) aura: AuraRef,
}

pub(crate) fn bind(fight: &mut Fight<PriestAgent>, aura: &str) -> Result<PowerInfusion, String> {
    Ok(PowerInfusion {
        aura: fight.player_aura(aura)?,
    })
}

impl PowerInfusion {
    /// The cast's `ApplyEffects`.
    pub(crate) fn apply(&self, fight: &mut Fight<PriestAgent>) {
        fight.activate_aura(self.aura);
    }
}
