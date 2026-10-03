//! Fire Blast (10199), from Go sim/mage/fire_blast.go: an instant Fire hit on a cooldown.
//! Only the highest rank is registered.

use crate::core::fight::{Agent, Fight, Side, SpellId};

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
}
