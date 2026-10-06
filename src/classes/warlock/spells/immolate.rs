//! Immolate (25309), from Go sim/warlock/immolate.go: a Fire hit whose landing applies the
//! snapshot dot of its related spell (25309 tag 1) before the hit's damage is dealt.

use crate::core::fight::{Agent, DotId, Fight, Side, SpellId};

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, dot: DotId) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    if result.landed() {
        fight.apply_dot(fight.dot_on(dot, target));
    }
    fight.deal_damage(spell, result, false);
}
