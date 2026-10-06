//! Fire Nova, from Go sim/shaman/fire_totems.go `registerFireNovaSpell`: one hit on each
//! target from the nova's fixed base, all resolved before any is dealt.

use crate::core::fight::{Agent, Fight, SpellId};

/// Go `CalcAoeDamage` followed by `DealBatchedAoeDamage`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, base_damage: f64) {
    let targets: Vec<_> = fight.target_sides().collect();
    let results: Vec<_> = targets
        .into_iter()
        .map(|target| fight.calc_damage(spell, target, base_damage))
        .collect();
    fight.deal_batched_aoe_damage(spell, &results, false);
}
