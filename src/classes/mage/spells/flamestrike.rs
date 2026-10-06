//! Flamestrike (10216), from Go sim/mage/flamestrike.go: a cast whose rolled hit lands on
//! each target in turn, then an area dot on the mage. Each tick deals the triggered spell's
//! amount of the same rank on current spell power to each target in turn, rolling to hit and
//! to crit (the tick row lacks Cannot Crit).

use crate::core::fight::{Agent, DotId, Fight, Outcome, Side, SpellId};

/// Go `ApplyEffects`: `CalcAndDealAoeDamageWithVariance`, then `AOEDot().Apply`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId) {
    fight.calc_and_deal_aoe_damage_with_variance(
        spell,
        |fight| fight.roll_damage_effect(spell),
        Fight::calc_damage,
    );
    let dot = fight.spells[spell]
        .dot
        .expect("Flamestrike has an area dot");
    fight.apply_dot(dot);
}

/// Go `OnTick`: `CalcAndDealPeriodicAoeDamage` with `OutcomeTickMagicHitAndCrit`, each target
/// calculated and dealt in turn.
pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, dot: DotId, tick_base: f64) {
    for position in 0..fight.targets.len() {
        fight.periodic_damage_tick_with(
            dot,
            Side::target(position),
            tick_base,
            Outcome::TickMagicHitAndCrit,
        );
    }
}
