//! Slam (11605), from Go sim/warrior/slam.go: after its cast, the client base on main hand
//! weapon damage, on the weapon special table, refunded on a miss. Without Improved Slam the
//! cast stops the swings, which the gate keeps out of scope.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    base_damage: f64,
) {
    let attack_power = fight.melee_attack_power();
    let base = base_damage + fight.mh_weapon_damage(attack_power);
    let outcome = PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        fight.issue_refund(spell);
    }
}
