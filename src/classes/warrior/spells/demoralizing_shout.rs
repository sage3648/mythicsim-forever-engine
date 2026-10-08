//! Demoralizing Shout (11556), from Go sim/warrior/demoralizing_shout.go: a magic hit roll on
//! every target in unit index order, each landed one activating that target's debuff, whose
//! attack power cut the target's swing at a tank reads while it is up.

use crate::core::fight::{Agent, AuraRef, Fight, Outcome, SpellId};

/// The shout's `ApplyEffects`: Go `CalcAndDealOutcome` with `OutcomeMagicHit` on each active
/// target, then the target's debuff when the roll landed. The debuff is activated after the
/// outcome is dealt, as Go does.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, aura: AuraRef) {
    let sides: Vec<_> = fight.target_sides().collect();
    for side in sides {
        let result = fight.calc_outcome(spell, side, Outcome::MagicHit);
        fight.deal_damage(spell, result, false);
        if result.landed() {
            let debuff = fight.aura_on(aura, side);
            fight.activate_aura(debuff);
        }
    }
}
