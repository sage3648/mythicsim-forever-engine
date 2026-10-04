//! Enrage (12317, buff 12880), from Go sim/warrior/talents_fury.go `registerEnrage`: a landed
//! hit taken that dealt damage enrages at a chance, a spell batch window later. The aura
//! raises physical damage done through a spell modifier while it holds its own exclusive
//! category.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, SpellResult, PRIORITY_DOT};

/// The class periodic tag of the delayed handler.
pub(crate) const PERIODIC_TAG: u32 = 3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Enrage {
    pub(crate) trigger: AuraRef,
    pub(crate) aura: AuraRef,
    pub(crate) proc_chance: f64,
    /// Go `AttachSpellMod`: physical damage done.
    pub(crate) damage_mod: ModId,
}

/// The trigger's OnSpellHitTaken.
pub(crate) fn on_hit_taken<A: Agent>(fight: &mut Fight<A>, params: Enrage, result: &SpellResult) {
    if !result.landed() || result.damage == 0.0 {
        return;
    }
    if params.proc_chance != 1.0 && fight.random_for_aura(params.trigger) > params.proc_chance {
        return;
    }
    fight.start_class_periodic(
        PERIODIC_TAG,
        crate::core::fight::SPELL_BATCH_WINDOW,
        1,
        PRIORITY_DOT,
    );
}

/// The delayed handler.
pub(crate) fn enrage<A: Agent>(fight: &mut Fight<A>, params: Enrage) {
    fight.activate_aura(params.aura);
}
