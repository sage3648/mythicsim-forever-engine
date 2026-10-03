//! Overpower's window, from Go sim/warrior/overpower.go: the "Overpower - Trigger" listener
//! activates the window on any dodged hit that is not a proc's. Overpower itself needs Battle
//! Stance.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult, OUTCOME_DODGE};

/// The trigger's OnSpellHitDealt, which acts at once.
pub(crate) fn on_hit<A: Agent>(
    fight: &mut Fight<A>,
    window: AuraRef,
    spell: SpellId,
    result: &SpellResult,
) {
    if fight.spells[spell].flags.proc || result.outcome & OUTCOME_DODGE == 0 {
        return;
    }
    fight.activate_aura(window);
}
