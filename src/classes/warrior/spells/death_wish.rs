//! Death Wish (12328), from Go sim/warrior/talents_fury.go `registerDeathWish`: an aura that
//! multiplies physical damage dealt, and a pause of the rotation for a default GCD. Its damage
//! taken increase has no effect in scope.

use crate::core::fight::{Agent, AuraRef, Fight};

/// Go `stats.SchoolIndexPhysical`.
const SCHOOL_INDEX_PHYSICAL: usize = 1;

#[derive(Clone, Copy, Debug)]
pub(crate) struct DeathWish {
    pub(crate) aura: AuraRef,
    pub(crate) physical_multiplier: f64,
    pub(crate) wait: i64,
}

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, params: DeathWish) {
    fight.activate_aura(params.aura);
    let ready = fight.now + params.wait;
    fight.wait_until(ready);
}

/// Go `AttachMultiplicativePseudoStatBuff`: the gain multiplies, the expiry divides.
pub(crate) fn on_gain<A: Agent>(fight: &mut Fight<A>, params: DeathWish) {
    fight.multiply_school_damage_dealt(SCHOOL_INDEX_PHYSICAL, params.physical_multiplier);
}

pub(crate) fn on_expire<A: Agent>(fight: &mut Fight<A>, params: DeathWish) {
    fight.divide_school_damage_dealt(SCHOOL_INDEX_PHYSICAL, params.physical_multiplier);
}
