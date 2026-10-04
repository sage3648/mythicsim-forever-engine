//! Earth Shock, from Go sim/shaman/shocks.go `registerEarthShockSpell`: a binary hit from the
//! highest rank's damage roll, dealt at once.

use crate::core::fight::{Agent, Fight, Side, SpellId};

/// Go `CalcAndDealDamage` with `OutcomeMagicHitAndCrit`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
}
