//! Flurry (12319, buff 12966), from Go sim/warrior/talents_fury.go `registerFlurry`: a landed
//! melee crit, other than Whirlwind's off hand strike, grants melee speed for a number of
//! charges, and each landed white hit spends one.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Flurry {
    pub(crate) aura: AuraRef,
    pub(crate) melee_speed_multiplier: f64,
    pub(crate) charges: i32,
}

/// The trigger's OnSpellHitDealt, which acts at once.
pub(crate) fn on_hit<A: Agent>(
    fight: &mut Fight<A>,
    params: Flurry,
    spell: SpellId,
    result: &SpellResult,
) {
    let state = &fight.spells[spell];
    if state.flags.proc || !state.melee_proc || !result.landed() {
        return;
    }
    if state.class_spell.as_deref() == Some("whirlwind_off_hand") {
        return;
    }
    if result.crit() {
        fight.activate_aura(params.aura);
        fight.set_stacks(params.aura, params.charges);
        return;
    }
    if fight.aura(params.aura).active && fight.spells[spell].white_hit {
        fight.remove_stack(params.aura);
    }
}

/// Go `AttachMultiplyMeleeSpeed`: the gain multiplies, the expiry multiplies by the
/// reciprocal.
pub(crate) fn on_gain<A: Agent>(fight: &mut Fight<A>, params: Flurry) {
    fight.multiply_melee_speed(params.melee_speed_multiplier);
}

pub(crate) fn on_expire<A: Agent>(fight: &mut Fight<A>, params: Flurry) {
    fight.multiply_melee_speed(1.0 / params.melee_speed_multiplier);
}
