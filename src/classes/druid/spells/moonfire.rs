//! Moonfire (9835), from Go sim/druid/moonfire.go. The hit applies the tagged dot spell's dot
//! when it lands, before the hit's own damage is dealt. Moonfire is one client spell and one
//! hit event, so the dot is applied directly and nothing else rolls off it (community #688).

use crate::core::fight::{Agent, Fight, Side, SpellId};

/// The impact: `dot_spell` is the tagged spell holding the dot.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    dot_spell: SpellId,
) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    if result.landed() {
        apply_dot(fight, dot_spell, target);
    }
    fight.deal_damage(spell, result, false);
}

/// The dot spell's `ApplyEffects`: the dot's `Apply`, with no outcome of its own.
pub(crate) fn apply_dot<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let dot = fight.spells[spell]
        .dot
        .expect("Moonfire's dot spell has a dot");
    fight.apply_dot(fight.dot_on(dot, target));
}
