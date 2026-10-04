//! Improved Hamstring (12289, root 23694), from Go sim/warrior/talents_arms.go
//! `registerImprovedHamstring`: a landed Hamstring roots the target at a chance, a spell batch
//! window later. The root has no effect in scope beyond its aura.

use crate::core::fight::{Agent, AuraRef, Fight, SpellResult, PRIORITY_DOT};

/// The class periodic tag of the delayed handler.
pub(crate) const PERIODIC_TAG: u32 = 5;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ImprovedHamstring {
    pub(crate) trigger: AuraRef,
    pub(crate) root: AuraRef,
    pub(crate) proc_chance: f64,
    pub(crate) delay: i64,
}

/// The trigger's `OnSpellHitDealt` for a Hamstring hit.
pub(crate) fn on_hamstring<A: Agent>(
    fight: &mut Fight<A>,
    params: ImprovedHamstring,
    result: &SpellResult,
) {
    if !result.landed() {
        return;
    }
    if params.proc_chance != 1.0 && fight.random_for_aura(params.trigger) > params.proc_chance {
        return;
    }
    fight.start_class_periodic(PERIODIC_TAG, params.delay, 1, PRIORITY_DOT);
}

/// The delayed handler.
pub(crate) fn root<A: Agent>(fight: &mut Fight<A>, params: ImprovedHamstring) {
    fight.activate_aura(params.root);
}
