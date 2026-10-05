//! Faerie Fire (9907), from Go sim/druid/faerie_fire.go: a magic hit roll that activates the
//! target's Faerie Fire on a landed outcome. The debuff's armor reduction, an exclusive effect,
//! applies while it is up when it holds its category alone, and never when another aura holds
//! the category for the whole fight; the gate admits only those two readings.

use crate::core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct FaerieFire {
    pub(crate) aura: AuraRef,
    /// The armor change while the debuff holds its category, or none.
    pub(crate) armor: Option<f64>,
}

impl FaerieFire {
    /// The exclusive effect's gain, before the aura's.
    pub(crate) fn on_exclusive_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        if let Some(armor) = self.armor {
            fight.add_target_armor(Side::Target, armor);
        }
    }

    /// The exclusive effect's expiry.
    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        if let Some(armor) = self.armor {
            fight.add_target_armor(Side::Target, -armor);
        }
    }

    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        let result = fight.calc_outcome(spell, target, Outcome::MagicHit);
        fight.deal_damage(spell, result, false);
        if result.landed() {
            fight.activate_aura(self.aura);
        }
    }
}
