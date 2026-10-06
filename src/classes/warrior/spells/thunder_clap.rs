//! Thunder Clap (11581), from Go sim/warrior/thunder_clap.go: the row's average plus a share
//! of attack power, a Go literal, on the magic hit and crit table, against each target up to
//! the row's cap from the cast target on. A landed clap activates its debuff on its target,
//! whose exclusive effect slows the target's melee speed by the clap's bid while it holds the
//! attack speed category, which holds only the clap.

use super::sweeping_strikes::{self, SweepingStrikes};
use crate::core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ThunderClap {
    pub(crate) aura: AuraRef,
    pub(crate) base_damage: f64,
    pub(crate) attack_power_share: f64,
    /// The row's target cap.
    pub(crate) max_targets: usize,
    /// The clap's bid in the attack speed category: how far from 1 its speed factor is.
    pub(crate) bid: f64,
}

/// The clap's `ApplyEffects`: Go `CalcCleaveDamage`, a Sweeping Strikes attack, then for each
/// result the debuff on a landed clap and the damage.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    params: ThunderClap,
    sweeping: Option<SweepingStrikes>,
) {
    // Go's arm64 build fuses the multiply and add of thunderClapBaseDamage+apShare*AP.
    let base = params
        .attack_power_share
        .mul_add(fight.melee_attack_power(), params.base_damage);
    let results: Vec<_> = fight
        .cleave_targets(target, params.max_targets)
        .into_iter()
        .map(|hit| fight.calc_damage_with(spell, hit, base, Outcome::MagicHitAndCrit))
        .collect();
    sweeping_strikes::cast_normalized(fight, sweeping, &results);
    for result in results {
        if result.landed() {
            let aura = fight.aura_on(params.aura, result.target);
            fight.activate_aura(aura);
        }
        fight.deal_damage(spell, result, false);
    }
}

/// The bid's OnGain: Go `MultiplyMeleeSpeed` by one less the bid.
pub(crate) fn on_gain<A: Agent>(fight: &mut Fight<A>, aura: AuraRef, params: ThunderClap) {
    fight.multiply_enemy_melee_speed(aura.side, 1.0 - params.bid);
}

/// The bid's OnExpire: the inverse factor.
pub(crate) fn on_expire<A: Agent>(fight: &mut Fight<A>, aura: AuraRef, params: ThunderClap) {
    fight.multiply_enemy_melee_speed(aura.side, 1.0 / (1.0 - params.bid));
}
