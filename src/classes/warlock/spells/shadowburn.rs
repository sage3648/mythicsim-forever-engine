//! Shadowburn (18871), from Go sim/warlock/shadowburn.go: an instant binary Shadow hit on a
//! cooldown, dealt at once.

use crate::core::fight::{Agent, Fight, Side, SpellId};

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
}
