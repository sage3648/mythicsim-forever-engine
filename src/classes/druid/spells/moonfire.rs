//! Moonfire (9835), from Go sim/druid/moonfire.go. The hit casts the tagged dot spell when it
//! lands, before the hit's own damage is dealt; the dot spell always hits, without a hit
//! counter, and its zero-damage outcome reaches hit listeners.

use crate::core::fight::{Agent, Fight, Outcome, Side, SpellId};

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
        fight.cast(dot_spell, target);
    }
    fight.deal_damage(spell, result, false);
}

/// The dot spell's `ApplyEffects`.
pub(crate) fn apply_dot<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::AlwaysHitNoHitCounter);
    let dot = fight.spells[spell]
        .dot
        .expect("Moonfire's dot spell has a dot");
    fight.apply_dot(dot);
    fight.deal_damage(spell, result, false);
}
