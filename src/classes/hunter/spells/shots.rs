//! Aimed Shot (19434 to 20904), Multi-Shot (2643) and Sniper Shot (1310687 to 1310786), from
//! Go sim/hunter/aimed_shot.go, multi_shot.go and sniper_shot.go. Each rolls a normalized
//! ranged weapon shot, Aimed and Sniper Shot add their rank's flat bonus, on the ranged hit
//! table, and the result lands after travel. Go hunter.go RegisterRangedSpell casts them over
//! the default cast time divided by the ranged haste multiplier, unrounded.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

/// The cast's `ApplyEffects`. Multi-Shot hits min(3, targets) targets: one in scope.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, flat_bonus: f64) {
    let attack_power = fight.ranged_attack_power();
    let base = fight.ranged_normalized_weapon_damage(attack_power) + flat_bonus;
    let result = fight.calc_physical_damage(
        spell,
        target,
        base,
        PhysicalOutcome::RangedHitAndCrit { count: true },
    );
    fight.deal_damage_after_travel(spell, result);
}

/// Go RegisterRangedSpell's `CastConfig.CastTime`, which its `ModifyCast` applies.
pub(crate) fn cast_time<A: Agent>(fight: &Fight<A>, spell: SpellId) -> Option<i64> {
    let cast_time = fight.spells[spell].default_cast.cast_time;
    (cast_time > 0).then(|| (cast_time as f64 / fight.ranged_haste_multiplier()) as i64)
}
