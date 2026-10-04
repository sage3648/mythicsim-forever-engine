//! Revenge (25288), from Go sim/warrior/revenge.go. "Revenge - Trigger" opens the Revenge
//! aura at once on a hit taken that the warrior blocks, dodges or parries; the strike, in
//! Defensive Stance while the aura is up, rolls the client row plus a quarter of attack power,
//! a Go literal, on the special table, closes the aura and refunds a miss.

use crate::{
    contracts::prepared_v2::DamageRoll,
    core::fight::{
        melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId, SpellResult, OUTCOME_BLOCK,
        OUTCOME_DODGE, OUTCOME_PARRY,
    },
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Revenge {
    pub(crate) aura: AuraRef,
    pub(crate) damage: DamageRoll,
    pub(crate) attack_power_share: f64,
}

/// Go spelldata `Effect.Roll` of the client row.
pub(crate) fn roll<A: Agent>(fight: &mut Fight<A>, damage: DamageRoll) -> f64 {
    fight.effect_roll(damage.average, damage.variance)
}

/// The trigger's OnSpellHitTaken, which acts at once.
pub(crate) fn on_hit_taken<A: Agent>(fight: &mut Fight<A>, params: Revenge, result: &SpellResult) {
    if result.outcome & (OUTCOME_BLOCK | OUTCOME_DODGE | OUTCOME_PARRY) != 0 {
        fight.activate_aura(params.aura);
    }
}

/// The strike's `ApplyEffects`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, params: Revenge) {
    let roll = roll(fight, params.damage);
    // Go's arm64 build fuses this multiply and add.
    let base = params
        .attack_power_share
        .mul_add(fight.melee_attack_power(), roll);
    let outcome = PhysicalOutcome::MeleeSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
    fight.deactivate_aura(params.aura);
    if !result.landed() {
        fight.issue_refund(spell);
    }
}
