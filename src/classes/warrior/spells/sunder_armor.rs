//! The warrior's own Sunder Armor (11597), from Go sim/warrior/sunder_armor.go: a special hit
//! roll without damage; a landed hit activates the debuff and adds a stack, a miss refunds
//! rage. The debuff bids its stacks in the target's major armor category, which sets the
//! target's armor; the cast needs the warrior's own debuff up or the category empty.

use crate::core::fight::{melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SunderArmor {
    pub(crate) aura: AuraRef,
    /// The target's major armor category.
    pub(crate) category: usize,
}

/// Go `CanApplySunderAura`.
pub(crate) fn condition<A: Agent>(fight: &Fight<A>, params: SunderArmor) -> bool {
    fight.aura(params.aura).active || fight.exclusive_active(params.category).is_none()
}

/// The cast's `ApplyEffects`.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    params: SunderArmor,
) {
    let outcome = PhysicalOutcome::MeleeSpecialHit { count: true };
    let result = fight.calc_physical_outcome(spell, target, outcome);
    if result.landed() {
        fight.activate_aura(params.aura);
        if fight.aura(params.aura).active {
            fight.add_stack(params.aura);
        }
    } else {
        fight.issue_refund(spell);
    }
    fight.deal_damage(spell, result, false);
}
