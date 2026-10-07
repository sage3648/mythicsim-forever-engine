//! Arcane Missiles (channel 25345, missile 25346), from Go sim/mage/arcane_missiles.go.
//! The channel applies a dot on the target whose every tick casts the missile spell; the
//! missile rolls partial resistance, hit and crit, then travels. The missiles carry the Arcane
//! Blast stacks the channel spent as it started, an additive damage bonus each (community #715).

use crate::core::fight::{Agent, Fight, Side, SpellId};

/// The channel cast: apply the channel's dot on the target.
pub(crate) fn apply_channel<A: Agent>(fight: &mut Fight<A>, spell: SpellId) {
    let dot = fight.spells[spell]
        .dot
        .expect("Arcane Missiles has a channel dot");
    fight.apply_dot(dot);
}

/// One missile of the channel, with the channel's Arcane Blast bonus added to the spell's
/// additive multiplier for its damage and taken off again, as Go does.
pub(crate) fn apply_missile<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    arcane_blast_bonus: f64,
) {
    fight.spells[spell].damage_multiplier_additive += arcane_blast_bonus;
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.spells[spell].damage_multiplier_additive -= arcane_blast_bonus;
    fight.deal_damage_after_travel(spell, result);
}
