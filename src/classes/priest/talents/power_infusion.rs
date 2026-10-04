//! Power Infusion (10060), from Go sim/priest/talents_discipline.go `applyPowerInfusion`: a
//! cooldown the priest casts on itself, activating the client-parsed Power Infusions aura. While
//! up, the aura multiplies the damage of the schools its mask names and the healing the priest
//! deals, which Holy Nova's heal reads.

use crate::core::fight::{AuraRef, Fight};

use super::super::agent::PriestAgent;

#[derive(Clone, Debug)]
pub(crate) struct PowerInfusion {
    pub(crate) aura: AuraRef,
    damage_multiplier: f64,
    schools: Vec<usize>,
    healing_multiplier: f64,
}

pub(crate) fn bind(
    fight: &mut Fight<PriestAgent>,
    aura: &str,
    damage_multiplier: f64,
    schools: &[usize],
    healing_multiplier: f64,
) -> Result<PowerInfusion, String> {
    let aura = fight.player_aura(aura)?;
    if let Some(&school) = schools.iter().find(|&&school| school >= 8) {
        return Err(format!("Power Infusion names school index {school}"));
    }
    Ok(PowerInfusion {
        aura,
        damage_multiplier,
        schools: schools.to_vec(),
        healing_multiplier,
    })
}

impl PowerInfusion {
    /// The cast's `ApplyEffects`.
    pub(crate) fn apply(&self, fight: &mut Fight<PriestAgent>) {
        fight.activate_aura(self.aura);
    }

    /// The exclusive effects' gain, healing first: Go multiplies the pseudo stats.
    pub(crate) fn on_gain(&self, fight: &mut Fight<PriestAgent>) {
        fight.agent.healing_dealt_multiplier *= self.healing_multiplier;
        for &school in &self.schools {
            fight.multiply_school_damage_dealt(school, self.damage_multiplier);
        }
    }

    /// The expiry divides them back.
    pub(crate) fn on_expire(&self, fight: &mut Fight<PriestAgent>) {
        fight.agent.healing_dealt_multiplier /= self.healing_multiplier;
        for &school in &self.schools {
            fight.divide_school_damage_dealt(school, self.damage_multiplier);
        }
    }
}
