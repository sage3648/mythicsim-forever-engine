//! Slam (11605), from Go sim/warrior/slam.go: after its cast, the client base on main hand
//! weapon damage, on the weapon special table, refunded on a miss. Without Improved Slam a
//! cast with a cast time stops the swings until a full swing after it ends.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

/// The cast's `ModifyCast`, which runs before any cast check.
pub(crate) fn modify_cast<A: Agent>(fight: &mut Fight<A>, spell: SpellId, stops_swings: bool) {
    let cast_time = fight.spells[spell].cur_cast.cast_time;
    if cast_time > 0 && stops_swings {
        fight.stop_melee_until(fight.now + cast_time);
    }
}

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
