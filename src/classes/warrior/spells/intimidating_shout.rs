//! Intimidating Shout (5246), from Go sim/warrior/intimidating_shout.go: an always hit outcome
//! with no damage on the target it is cast at, unlike Challenging Shout, which reaches every
//! target. The fork has no fear, so the spell changes no target's aim or swing, and its threat is
//! the spell's flat bonus of none. The rage cost, global cooldown, cooldown and range are the
//! common cast's.

use crate::core::fight::{Agent, Fight, Outcome, Side, SpellId};

/// The spell's `ApplyEffects`: Go `CalcAndDealOutcome` with `OutcomeAlwaysHit` on the target.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::AlwaysHit);
    fight.deal_damage(spell, result, false);
}
