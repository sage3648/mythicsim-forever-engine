//! Challenging Shout (1161), from Go sim/warrior/challenging_shout.go: an always hit outcome
//! with no damage on every active target in unit index order. The fork's shout does not taunt,
//! so it changes no target's aim or swing, and its threat is the spell's flat bonus of none.
//! The rage cost, global cooldown and the shout's own cooldown are the common cast's.

use crate::core::fight::{Agent, Fight, Outcome, SpellId};

/// The shout's `ApplyEffects`: Go `CalcAndDealOutcome` with `OutcomeAlwaysHit` on each active
/// target, calculated and dealt in turn.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId) {
    let sides: Vec<_> = fight.target_sides().collect();
    for side in sides {
        let result = fight.calc_outcome(spell, side, Outcome::AlwaysHit);
        fight.deal_damage(spell, result, false);
    }
}
