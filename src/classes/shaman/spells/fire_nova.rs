//! Fire Nova, from Go sim/shaman/fire_totems.go `registerFireNovaSpell`: one hit on each
//! target from the nova's fixed base, all resolved before any is dealt. The runtime has one
//! target.

use crate::core::fight::{Agent, Fight, Side, SpellId};

/// Go `CalcAoeDamage` followed by `DealBatchedAoeDamage`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, base_damage: f64) {
    let result = fight.calc_damage(spell, Side::Target, base_damage);
    fight.deal_damage(spell, result, false);
}
