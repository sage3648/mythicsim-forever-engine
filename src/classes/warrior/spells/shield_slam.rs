//! Shield Slam (23925), from Go sim/warrior/talents_protection.go `registerShieldSlam`: the
//! client roll plus the warrior's block value on the special table, struck with the shield's
//! off hand special proc mask, refunded on a miss.

use crate::{
    contracts::prepared_v2::DamageRoll,
    core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId},
};

use super::revenge::roll;

pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    damage: DamageRoll,
) {
    let base = roll(fight, damage) + fight.player_block_damage_reduction();
    let outcome = PhysicalOutcome::MeleeSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        fight.issue_refund(spell);
    }
}
