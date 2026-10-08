//! Power Infusion (10060), the generated aura Go's `buffs.PowerInfusionsAura` builds for the
//! priest's own cast (sim/priest/talents_discipline.go) and for the external cooldown any class
//! receives (buffs/drivers.go `drivePowerInfusions`). The client row multiplies the damage of the
//! schools its mask names and the healing the player deals. Go parses each into a pseudo stat
//! multiplier under an exclusive category of its own, so two copies of the aura do not stack:
//! the effect that holds a category applies its multiplier, and when the holder changes the old
//! effect divides it back before the new one multiplies. Each category is only tracked, since
//! none holds a single aura, and the holder's change drives the multiplier.

use crate::contracts::prepared_v2::Effect;

use super::{Agent, AuraRef, Fight};

/// The category of the healing dealt multiplier, Go's per stat name for the pseudo stat.
const HEALING_CATEGORY: &str = "PowerInfusionHealingDealtMultiplierMul";

/// The category of each school's damage dealt multiplier.
const DAMAGE_CATEGORY: &str = "PowerInfusionSchoolDamageDealtMultiplierMul";

/// What every copy of the aura multiplies.
#[derive(Clone, Debug)]
pub(crate) struct PowerInfusion {
    damage_multiplier: f64,
    schools: Vec<usize>,
    healing_multiplier: f64,
}

impl<A: Agent> Fight<A> {
    /// Bind the multipliers of the Power Infusion effects, one for each copy of the aura, which
    /// read the same client row.
    pub(crate) fn bind_power_infusion(&mut self, effects: &[Effect]) -> Result<(), String> {
        for effect in effects {
            let Effect::PowerInfusion {
                damage_multiplier,
                schools,
                healing_multiplier,
                ..
            } = effect
            else {
                continue;
            };
            if let Some(&school) = schools.iter().find(|&&school| school >= 8) {
                return Err(format!("Power Infusion names school index {school}"));
            }
            let bound = PowerInfusion {
                damage_multiplier: *damage_multiplier,
                schools: schools.clone(),
                healing_multiplier: *healing_multiplier,
            };
            match &self.power_infusion {
                Some(existing)
                    if existing.damage_multiplier != bound.damage_multiplier
                        || existing.schools != bound.schools
                        || existing.healing_multiplier != bound.healing_multiplier =>
                {
                    return Err("the copies of Power Infusion differ".into());
                }
                _ => self.power_infusion = Some(bound),
            }
        }
        Ok(())
    }

    /// The exclusive effect of a Power Infusion aura gained or lost a tracked category: the
    /// old holder's effect expires, then the new holder's gains. The healing category is
    /// first among an aura's effects, and each category moves on its own.
    pub(crate) fn power_infusion_category_change(
        &mut self,
        category: usize,
        old: Option<AuraRef>,
        new: Option<AuraRef>,
    ) {
        let Some(bound) = self.power_infusion.clone() else {
            return;
        };
        match self.exclusive_tracking[category].name.as_str() {
            HEALING_CATEGORY => {
                if old.is_some() {
                    self.healing_dealt_factor /= bound.healing_multiplier;
                }
                if new.is_some() {
                    self.healing_dealt_factor *= bound.healing_multiplier;
                }
            }
            DAMAGE_CATEGORY => {
                if old.is_some() {
                    for &school in &bound.schools {
                        self.divide_school_damage_dealt(school, bound.damage_multiplier);
                    }
                }
                if new.is_some() {
                    for &school in &bound.schools {
                        self.multiply_school_damage_dealt(school, bound.damage_multiplier);
                    }
                }
            }
            _ => {}
        }
    }
}
