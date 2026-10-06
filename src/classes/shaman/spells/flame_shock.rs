//! Flame Shock, from Go sim/shaman/shocks.go: the hit resolves, a landed hit casts the
//! periodic half, which applies a dot that snapshots its tick base, and then the hit is dealt.

use crate::core::fight::{Agent, Fight, Side, SpellId};

/// Go `registerFlameShockSpell` ApplyEffects.
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

/// The periodic half's ApplyEffects: `Dot.Apply` of the dot on the cast target.
pub(crate) fn apply_dot<A: Agent>(fight: &mut Fight<A>, dot_spell: SpellId, target: Side) {
    let dot = fight.spells[dot_spell]
        .dot
        .expect("the periodic half has a dot");
    let dot = fight.dot_on(dot, target);
    fight.apply_dot(dot);
}
