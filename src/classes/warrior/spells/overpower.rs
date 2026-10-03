//! Overpower (11585), from Go sim/warrior/overpower.go. The "Overpower - Trigger" listener
//! opens the window on any dodged hit that is not a proc's; the strike, in Battle Stance while
//! the window is open, deals the client base on normalized main hand damage on a table without
//! dodges, parries or blocks, closes the window and refunds a miss.

use crate::core::fight::{
    melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId, SpellResult, OUTCOME_DODGE,
};

/// The trigger's OnSpellHitDealt, which acts at once.
pub(crate) fn on_hit<A: Agent>(
    fight: &mut Fight<A>,
    window: AuraRef,
    spell: SpellId,
    result: &SpellResult,
) {
    if fight.spells[spell].flags.proc || result.outcome & OUTCOME_DODGE == 0 {
        return;
    }
    fight.activate_aura(window);
}

/// The strike's `ApplyEffects`.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    base_damage: f64,
    window: AuraRef,
) {
    let attack_power = fight.melee_attack_power();
    let base = base_damage + fight.mh_normalized_weapon_damage(attack_power);
    let outcome = PhysicalOutcome::MeleeSpecialNoBlockDodgeParry { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
    fight.deactivate_aura(window);
    if !result.landed() {
        fight.issue_refund(spell);
    }
}
