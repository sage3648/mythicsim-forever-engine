//! Starfire (every rank, 2912 to 25298), from Go sim/druid/starfire.go: a direct Arcane hit.

use crate::core::fight::{Agent, Fight, Side, SpellId};

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
}
