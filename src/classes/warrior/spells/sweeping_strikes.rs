//! Sweeping Strikes (12723, row 12292), from Go sim/warrior/talents_arms.go
//! `registerSweepingStrikes`: a major cooldown that needs Battle Stance and activates an aura
//! with the row's charges. The aura's handler copies a hit to a second target and spends a
//! charge only while two targets are active, so with the one target in scope the aura only
//! runs its duration.

use crate::core::fight::{Agent, AuraRef, Fight};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SweepingStrikes {
    pub(crate) aura: AuraRef,
    pub(crate) charges: i32,
}

/// The cast's `ApplyEffects`: activate the aura, then set its stacks to the charges.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, params: SweepingStrikes) {
    fight.activate_aura(params.aura);
    fight.set_stacks(params.aura, params.charges);
}
