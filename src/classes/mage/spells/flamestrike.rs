//! Flamestrike (10216), from Go sim/mage/flamestrike.go: a cast whose rolled hit lands on
//! each target, then an area dot on the mage. Each tick deals the triggered spell's amount
//! of the same rank on current spell power, rolling to hit and never to crit. The runtime
//! has one target.

use crate::core::fight::{Agent, DotId, Fight, Outcome, Side, SpellId};

/// Go `ApplyEffects`: `CalcAndDealAoeDamageWithVariance`, then `AOEDot().Apply`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, Side::Target, base);
    fight.deal_damage(spell, result, false);
    let dot = fight.spells[spell]
        .dot
        .expect("Flamestrike has an area dot");
    fight.apply_dot(dot);
}

/// Go `OnTick`: `CalcAndDealPeriodicAoeDamage` with `OutcomeTickMagicHit`.
pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, dot: DotId, tick_base: f64) {
    fight.periodic_damage_tick_with(dot, Side::Target, tick_base, Outcome::TickMagicHit);
}
