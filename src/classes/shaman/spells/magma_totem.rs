//! Magma Totem's pulse, from Go sim/shaman/fire_totems.go `registerMagmaTotemSpell`: every two
//! seconds the area dot on the shaman hits each target from a fixed base on the magic hit and
//! crit tick table, each result calculated before any is dealt.

use crate::core::fight::{Agent, DotId, Fight, Outcome};

/// The dot's `OnTick`: Go `CalcPeriodicAoeDamage` followed by `DealBatchedPeriodicDamage`.
pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, dot: DotId, base: f64) {
    let spell = fight.dots[dot].spell;
    let targets: Vec<_> = fight.target_sides().collect();
    let results: Vec<_> = targets
        .into_iter()
        .map(|target| fight.calc_periodic_damage(dot, target, base, Outcome::TickMagicHitAndCrit))
        .collect();
    fight.deal_batched_aoe_damage(spell, &results, true);
}
