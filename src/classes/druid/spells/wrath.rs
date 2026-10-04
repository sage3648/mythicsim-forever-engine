//! Wrath (every rank, 5176 to 9912), from Go sim/druid/wrath.go: a Nature hit rolled at
//! the cast and dealt after travel.

use crate::core::fight::{Agent, Fight, Side, SpellId};

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage_after_travel(spell, result);
}
