//! Corruption (25311), from Go sim/warlock/corruption.go: a hit roll without a hit counter,
//! then the snapshot dot when it lands, then the empty outcome is dealt.

use crate::core::fight::{Agent, DotId, Fight, Outcome, Side, SpellId};

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, dot: DotId) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHitNoHitCounter);
    if result.landed() {
        fight.apply_dot(dot);
    }
    fight.deal_damage(spell, result, false);
}
