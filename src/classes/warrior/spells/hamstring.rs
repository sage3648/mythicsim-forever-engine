//! Hamstring (7373), from Go sim/warrior/hamstring.go: a fixed client base on the special
//! hit table, refunded on a miss. Its snare has no effect in scope.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    base_damage: f64,
) {
    let outcome = PhysicalOutcome::MeleeSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base_damage, outcome);
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        fight.issue_refund(spell);
    }
}
