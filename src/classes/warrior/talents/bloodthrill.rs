//! Bloodthrill (1289682), from Go sim/warrior/talents_arms.go `registerBloodthrill`: a landed
//! main hand hit, other than a proc's, on a target bleeding from Rend may open the Overpower
//! window a spell batch window later, for the window's longer duration.

use crate::core::fight::{Agent, AuraRef, DotId, Fight, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bloodthrill {
    pub(crate) trigger: AuraRef,
    pub(crate) proc_chance: f64,
    pub(crate) window_duration: i64,
    pub(crate) overpower_window: AuraRef,
    pub(crate) rend_dot: DotId,
}

/// The trigger's OnSpellHitDealt. `main_hand` is whether the spell's proc mask holds a main
/// hand bit.
pub(crate) fn on_hit<A: Agent>(
    fight: &mut Fight<A>,
    params: Bloodthrill,
    spell: SpellId,
    main_hand: bool,
    result: &SpellResult,
) {
    if fight.spells[spell].flags.proc || !main_hand || !result.landed() {
        return;
    }
    // Go `Rend.Dot(result.Target)`: the dot on the target the hit landed on.
    let rend_dot = fight.dot_on(params.rend_dot, result.target);
    let rend = fight.dots[rend_dot].aura;
    if !fight.aura(rend).active {
        return;
    }
    if params.proc_chance != 1.0 && fight.random_for_aura(params.trigger) > params.proc_chance {
        return;
    }
    fight.schedule_delayed_proc(params.trigger, spell, *result);
}

/// The delayed handler: the window opens and runs for the longer duration, without a
/// tracker reschedule, as Go `UpdateExpires` does.
pub(crate) fn open<A: Agent>(fight: &mut Fight<A>, params: Bloodthrill) {
    fight.activate_aura(params.overpower_window);
    let expires = fight.now + params.window_duration;
    fight.aura_mut(params.overpower_window).expires = expires;
}
