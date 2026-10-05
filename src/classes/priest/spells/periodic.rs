//! Shadow Word: Pain (589 to 10894), Devouring Plague (2944 to 19280) and Mind Flay (15407
//! to 18807), from Go sim/priest/shadow_word_pain.go, devouring_plague.go and
//! talents_shadow.go. Each rolls its hit once when cast, without a hit count: a landed roll
//! applies the dot, then the zero-damage outcome reaches hit listeners. A tick rolls only the
//! crit the client's Periodic Can Crit attribute allows (Go `priestTickOutcome`). Mind Flay is
//! binary and channeled.

use crate::core::fight::{Agent, Fight, Outcome, Side, SpellId};

/// The cast's `ApplyEffects`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHitNoHitCounter);
    if result.landed() {
        let dot = fight.spells[spell]
            .dot
            .expect("a priest dot spell has a dot");
        fight.apply_dot(fight.dot_on(dot, target));
    }
    fight.deal_damage(spell, result, false);
}
