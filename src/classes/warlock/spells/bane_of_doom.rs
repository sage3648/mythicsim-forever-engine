//! Bane of Doom (603), from Go sim/warlock/doom.go: a hit roll without a hit counter, then,
//! when it lands, the bane slot is taken and the one-tick snapshot dot applied.

use crate::{
    classes::warlock::agent::WarlockAgent,
    core::fight::{DotId, Fight, Outcome, Side, SpellId},
};

use super::take_bane_slot;

pub(crate) fn apply(fight: &mut Fight<WarlockAgent>, spell: SpellId, target: Side, dot: DotId) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHitNoHitCounter);
    if result.landed() {
        let dot = fight.dot_on(dot, target);
        let aura = fight.dots[dot].aura;
        take_bane_slot(fight, aura);
        fight.apply_dot(dot);
    }
    fight.deal_damage(spell, result, false);
}
