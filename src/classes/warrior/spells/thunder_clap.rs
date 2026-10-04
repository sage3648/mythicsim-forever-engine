//! Thunder Clap (11581), from Go sim/warrior/thunder_clap.go: the row's average plus a share
//! of attack power, a Go literal, on the magic hit and crit table. A landed clap activates its
//! debuff on the target, whose exclusive effect slows the target's melee speed by the clap's
//! bid while it holds the attack speed category, which holds only the clap.

use crate::core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ThunderClap {
    pub(crate) aura: AuraRef,
    pub(crate) base_damage: f64,
    pub(crate) attack_power_share: f64,
    /// The clap's bid in the attack speed category: how far from 1 its speed factor is.
    pub(crate) bid: f64,
}

/// The clap's `ApplyEffects` on its one target: Go `CalcCleaveDamage`, the debuff on a landed
/// clap, then the damage.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    params: ThunderClap,
) {
    // Go's arm64 build fuses the multiply and add of thunderClapBaseDamage+apShare*AP.
    let base = params
        .attack_power_share
        .mul_add(fight.melee_attack_power(), params.base_damage);
    let result = fight.calc_damage_with(spell, target, base, Outcome::MagicHitAndCrit);
    if result.landed() {
        fight.activate_aura(params.aura);
    }
    fight.deal_damage(spell, result, false);
}

/// The bid's OnGain: Go `MultiplyMeleeSpeed` by one less the bid.
pub(crate) fn on_gain<A: Agent>(fight: &mut Fight<A>, params: ThunderClap) {
    fight.multiply_enemy_melee_speed(1.0 - params.bid);
}

/// The bid's OnExpire: the inverse factor.
pub(crate) fn on_expire<A: Agent>(fight: &mut Fight<A>, params: ThunderClap) {
    fight.multiply_enemy_melee_speed(1.0 / (1.0 - params.bid));
}
