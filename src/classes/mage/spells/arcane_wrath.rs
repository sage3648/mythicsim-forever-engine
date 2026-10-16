//! Arcane Wrath (1326079 to 1326082), from Go sim/mage/arcane_wrath.go: an Arcane talent of
//! client 70425, a 3.5 sec Arcane bolt on a 6 sec cooldown whose hit adds 5% of the mage's
//! maximum mana as spell damage, at the damage effect's coefficient, read at cast.

use crate::core::fight::{Agent, Fight, Side, SpellId};

/// Go `ApplyEffects`: the rolled hit plus the mana's share, dealt after travel.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    (coefficient, mana_share): (f64, f64),
) {
    let roll = fight.roll_damage_effect(spell);
    // Go fuses the multiply and add on arm64.
    let base = (coefficient * mana_share).mul_add(fight.player.powers.max_mana, roll);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage_after_travel(spell, result);
}
