//! Bloodthirst (23894), from Go sim/warrior/talents_fury.go `registerBloodthirst`: a share
//! of attack power plus the client base, on the special hit table, refunded on a miss.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bloodthirst {
    pub(crate) attack_power_share: f64,
    pub(crate) base_damage: f64,
}

pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    params: Bloodthirst,
) {
    // Go's arm64 build fuses this multiply and add.
    let base = fight
        .melee_attack_power()
        .mul_add(params.attack_power_share, params.base_damage);
    let outcome = PhysicalOutcome::MeleeSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        fight.issue_refund(spell);
    }
}
