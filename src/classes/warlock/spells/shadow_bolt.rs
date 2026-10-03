//! Shadow Bolt, every rank, from Go sim/warlock/shadowbolt.go: the client damage roll and
//! the magic hit and crit at cast completion, dealt when the bolt arrives.

use crate::core::fight::{Agent, Fight, Side, SpellId};

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage_after_travel(spell, result);
}
