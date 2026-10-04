//! Spearing Strike (1310222), from Go sim/warrior/talents_arms.go `registerSpearingStrike`: a
//! share of normalized main hand damage, raised against giants and dragonkin, on the weapon
//! special table in Battle Stance, refunded on a miss.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpearingStrike {
    pub(crate) weapon_share: f64,
    /// 1 unless the target is a giant or dragonkin.
    pub(crate) mob_multiplier: f64,
}

pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    params: SpearingStrike,
) {
    let attack_power = fight.melee_attack_power();
    let mut base = params.weapon_share * fight.mh_normalized_weapon_damage(attack_power);
    if params.mob_multiplier != 1.0 {
        base *= params.mob_multiplier;
    }
    let outcome = PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        fight.issue_refund(spell);
    }
}
