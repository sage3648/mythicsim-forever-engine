//! Unbridled Wrath (12322, rage from 12964), from Go sim/warrior/talents_fury.go
//! `registerUnbridledWrath`: a proc trigger on landed white hits that dealt damage, which
//! grants rage a spell batch window later, doubled for a two-handed weapon.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct UnbridledWrath {
    pub(crate) trigger: AuraRef,
    pub(crate) proc_chance: f64,
    pub(crate) rage: f64,
    pub(crate) two_handed: bool,
    /// Go `NewRageMetrics(actionID)`.
    pub(crate) metrics: usize,
}

/// The trigger's OnSpellHitDealt.
pub(crate) fn on_hit<A: Agent>(
    fight: &mut Fight<A>,
    params: UnbridledWrath,
    spell: SpellId,
    result: &SpellResult,
) {
    let state = &fight.spells[spell];
    if state.flags.proc || !state.white_hit || !result.landed() || result.damage == 0.0 {
        return;
    }
    if params.proc_chance != 1.0 && fight.random_for_aura(params.trigger) > params.proc_chance {
        return;
    }
    fight.schedule_delayed_proc(params.trigger, spell, *result);
}

/// The delayed handler.
pub(crate) fn grant<A: Agent>(fight: &mut Fight<A>, params: UnbridledWrath) {
    let rage = params.rage * if params.two_handed { 2.0 } else { 1.0 };
    fight.add_rage(rage, params.metrics);
}
