//! Insect Swarm (24977), from Go sim/druid/insect_swarm.go: a binary hit roll with a hit
//! counter, then the snapshotting dot, whose aura holds the Insect Swarm debuff on the target.

use crate::core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId};

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHit);
    if result.landed() {
        let dot = fight.spells[spell].dot.expect("Insect Swarm has a dot");
        fight.apply_dot(dot);
    }
    fight.deal_damage(spell, result, false);
}

/// The dot aura's OnGain: the debuff, whose effects do not reach the player's spells.
pub(crate) fn on_dot_gain<A: Agent>(fight: &mut Fight<A>, debuff: Option<AuraRef>) {
    if let Some(debuff) = debuff {
        fight.activate_aura(debuff);
    }
}

/// The dot aura's OnExpire.
pub(crate) fn on_dot_expire<A: Agent>(fight: &mut Fight<A>, debuff: Option<AuraRef>) {
    if let Some(debuff) = debuff {
        fight.deactivate_aura(debuff);
    }
}
