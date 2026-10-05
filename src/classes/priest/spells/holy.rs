//! Holy Fire (14914 to 15261) and Penance (1316995), from Go sim/priest/holy_fire.go and
//! penance.go. Smite is a plain hit, in `direct`.

use crate::core::fight::{Agent, Fight, Outcome, Side, SpellId};

/// Holy Fire: the hit and crit roll on the rank's row; a landed hit applies the snapshotting
/// dot before the hit is dealt.
pub(crate) fn apply_holy_fire<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    if result.landed() {
        let dot = fight.spells[spell].dot.expect("Holy Fire has a dot");
        fight.apply_dot(fight.dot_on(dot, target));
    }
    fight.deal_damage(spell, result, false);
}

/// Penance: a hit roll without a hit count; a landed roll starts the channel and fires the
/// first bolt at once (Go `Dot.TickOnce`, an extra tick that leaves the tick count alone),
/// then the zero-damage outcome reaches hit listeners.
pub(crate) fn apply_penance<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHitNoHitCounter);
    if result.landed() {
        let dot = fight.spells[spell].dot.expect("Penance has a channel");
        let dot = fight.dot_on(dot, target);
        fight.apply_dot(dot);
        fight.snapshot_dot_tick(dot);
    }
    fight.deal_damage(spell, result, false);
}
