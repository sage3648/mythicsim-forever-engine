//! Retaliation (20230), from Go sim/warrior/retaliation.go: in Battle Stance, an aura with the
//! row's charges; each landed melee hit taken that dealt damage casts the strike back (20240),
//! the row's base on main hand weapon damage on the special table, and spends a charge.

use crate::core::fight::{
    melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId, SpellResult,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Retaliation {
    pub(crate) aura: AuraRef,
    pub(crate) hit: SpellId,
    pub(crate) charges: i32,
    pub(crate) hit_base_damage: f64,
}

/// The cast's `ApplyEffects`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, params: Retaliation) {
    fight.activate_aura(params.aura);
    fight.set_stacks(params.aura, params.charges);
}

/// The aura's OnSpellHitTaken for the target's melee swing.
pub(crate) fn on_hit_taken<A: Agent>(
    fight: &mut Fight<A>,
    params: Retaliation,
    result: &SpellResult,
) {
    if result.landed() && result.damage > 0.0 {
        fight.cast(params.hit, Side::Target);
        fight.remove_stack(params.aura);
    }
}

/// The strike's `ApplyEffects`.
pub(crate) fn strike<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    params: Retaliation,
) {
    let attack_power = fight.melee_attack_power();
    let base = params.hit_base_damage + fight.mh_weapon_damage(attack_power);
    let outcome = PhysicalOutcome::MeleeSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
}
