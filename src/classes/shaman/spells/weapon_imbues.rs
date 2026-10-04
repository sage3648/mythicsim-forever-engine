//! Flametongue and Frostbrand Weapon, from Go sim/shaman/weapon_imbues.go: weapon procs on
//! landed hits that cast an imbue hit at once, a fixed base on the magic hit table.

use crate::core::fight::{Agent, AuraRef, Fight, Side, SpellId, SpellResult};

/// An imbue hit's ApplyEffects: Go `CalcAndDealDamage` with `OutcomeMagicHitAndCrit`.
pub(crate) fn hit<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, base: f64) {
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
}

/// The Frostbrand trigger: a landed hit its proc manager hears rolls the hand's chance.
pub(crate) fn frostbrand_trigger<A: Agent>(
    fight: &mut Fight<A>,
    trigger: AuraRef,
    hit: SpellId,
    result: &SpellResult,
    chance: Option<f64>,
) {
    let Some(chance) = chance else {
        return;
    };
    if !result.landed() {
        return;
    }
    if fight.proc_for_aura(chance, trigger) {
        fight.cast(hit, result.target);
    }
}
