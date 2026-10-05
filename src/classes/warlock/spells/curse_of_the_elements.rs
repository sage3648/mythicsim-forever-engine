//! Curse of the Elements (1311680), from Go sim/warlock/curse_of_elements.go and core's
//! debuff in buffs/debuffs_auto_gen.go. The cast rolls to hit without a hit counter and, when
//! it lands, takes the curse slot from Curse of Recklessness and activates the debuff on the
//! target. The debuff lowers resistances by a flat amount and
//! multiplies school damage taken, as spelldata parse_effects_table.go attaches them: a stat
//! delta added on gain and removed on expiry, and a factor multiplied on gain and its
//! reciprocal multiplied on expiry.

use std::collections::BTreeMap;

use crate::{
    classes::warlock::agent::WarlockAgent,
    core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId},
};

use super::take_curse_slot;

/// The debuff's changes, by Go school index.
#[derive(Clone, Debug)]
pub(crate) struct CurseOfTheElements {
    pub(crate) aura: AuraRef,
    resistance: Vec<(usize, f64)>,
    damage_taken: Vec<(usize, f64)>,
    /// The raid's own curse holds the exclusive category, so activation never succeeds.
    blocked: bool,
}

/// Go school names as the exporter writes them, by school index.
const SCHOOLS: [&str; 8] = [
    "none", "physical", "arcane", "fire", "frost", "holy", "nature", "shadow",
];

fn by_school(values: &BTreeMap<String, f64>) -> Result<Vec<(usize, f64)>, String> {
    values
        .iter()
        .map(|(name, value)| {
            SCHOOLS
                .iter()
                .position(|school| school == name)
                .filter(|&index| index >= 2)
                .map(|index| (index, *value))
                .ok_or_else(|| format!("Curse of the Elements names unknown school {name}"))
        })
        .collect()
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    aura: &str,
    resistance: &BTreeMap<String, f64>,
    damage_taken: &BTreeMap<String, f64>,
    blocked: bool,
) -> Result<CurseOfTheElements, String> {
    let index = fight.trackers[Side::Target.index()]
        .find(aura)
        .ok_or_else(|| format!("target aura {aura} is not registered"))?;
    Ok(CurseOfTheElements {
        aura: AuraRef {
            side: Side::Target,
            index,
        },
        resistance: by_school(resistance)?,
        damage_taken: by_school(damage_taken)?,
        blocked,
    })
}

impl CurseOfTheElements {
    /// `ApplyEffects`.
    pub(crate) fn apply(&self, fight: &mut Fight<WarlockAgent>, spell: SpellId, target: Side) {
        let result = fight.calc_outcome(spell, target, Outcome::MagicHitNoHitCounter);
        if result.landed() {
            take_curse_slot(fight, self.aura);
            if self.blocked {
                // Go Aura.Activate counts the proc before the exclusive effect blocks it.
                fight.aura_mut(self.aura).procs += 1;
            } else {
                fight.activate_aura(self.aura);
            }
        }
        fight.deal_damage(spell, result, false);
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        for &(index, delta) in &self.resistance {
            fight.add_target_resistance(Side::Target, index, delta);
        }
        for &(index, factor) in &self.damage_taken {
            fight.multiply_target_school_damage_taken(Side::Target, index, factor);
        }
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        for &(index, delta) in &self.resistance {
            fight.add_target_resistance(Side::Target, index, -delta);
        }
        for &(index, factor) in &self.damage_taken {
            fight.multiply_target_school_damage_taken(Side::Target, index, 1.0 / factor);
        }
    }
}
