//! Faerie Fire (9907), from Go sim/druid/faerie_fire.go: a magic hit roll that activates the
//! target's Faerie Fire on a landed outcome. The gate admits it only where another aura holds
//! the armor reduction category for the whole fight, so the debuff's own armor never applies.

use crate::core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct FaerieFire {
    pub(crate) aura: AuraRef,
}

impl FaerieFire {
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        let result = fight.calc_outcome(spell, target, Outcome::MagicHit);
        fight.deal_damage(spell, result, false);
        if result.landed() {
            fight.activate_aura(self.aura);
        }
    }
}
