//! Shield Slam (23925), from Go sim/warrior/talents_protection.go `registerShieldSlam`: the
//! client roll plus the warrior's block value on the special table, refunded on a miss. The Go
//! reference strikes with the shield's off hand special proc mask and adds the block value
//! once.
//!
//! Under Forever's rules it is a main hand special and adds twice the block value, from
//! protection warrior combat logs of client 1.60.1.70862 (8 November 2026): the main hand
//! weapon's equip effect (Dreadfrost Saber) fired on 54 of 80 isolated Shield Slams (and on
//! none of 74 Shield Blocks), and at a block value of 28 the mean hit sits about 28 above one
//! block value's worth (481 against 453). Weapon procs do not hear it (prepare/spells.rs
//! `shield_strike_flags`).

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
    twice_block_value: bool,
) {
    let block_value = fight.player_block_damage_reduction();
    let base = if twice_block_value {
        roll(fight, damage) + 2.0 * block_value
    } else {
        roll(fight, damage) + block_value
    };
    let outcome = PhysicalOutcome::MeleeSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        fight.issue_refund(spell);
    }
}
