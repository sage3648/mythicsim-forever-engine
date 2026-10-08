//! Taunt (355), from Go sim/warrior/taunt.go: an always hit outcome with no damage on the
//! target. The fork has no taunt, so the spell changes no target's aim or swing, and its threat
//! is the spell's flat bonus of none. It needs Defensive Stance (the agent's cast condition), and
//! has no rage cost and no global cooldown; the spell's own cooldown and range are the common
//! cast's.

use crate::core::fight::{Agent, Fight, Outcome, Side, SpellId};

/// The spell's `ApplyEffects`: Go `CalcAndDealOutcome` with `OutcomeAlwaysHit` on the target.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::AlwaysHit);
    fight.deal_damage(spell, result, false);
}
