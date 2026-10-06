//! Incinerate (1293813), from Go sim/warlock/incinerate.go: the client damage roll, raised on
//! a target burning with Immolate, then the magic hit and crit, dealt when it arrives.

use crate::core::fight::{Agent, DotId, Fight, Side, SpellId};

pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    immolate: Option<DotId>,
    immolate_bonus: f64,
) {
    let mut base = fight.roll_damage_effect(spell);
    if immolate.is_some_and(|dot| {
        fight
            .aura(fight.dots[fight.dot_on(dot, target)].aura)
            .active
    }) {
        base *= immolate_bonus;
    }
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage_after_travel(spell, result);
}
