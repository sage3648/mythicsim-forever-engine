//! Aimed Shot (19434 to 20904), Multi-Shot (2643), Sniper Shot (1310687 to 1310786) and Arcane
//! Shot (3044 to 14287), from Go sim/hunter/aimed_shot.go, multi_shot.go, sniper_shot.go and
//! arcane_shot.go. The first three roll a normalized ranged weapon shot, Aimed and Sniper Shot
//! adding their rank's flat bonus; Arcane Shot deals its own flat damage and shares. Each rolls
//! on the ranged hit table, and the result lands after travel. Go hunter.go RegisterRangedSpell casts them over
//! the default cast time divided by the ranged haste multiplier, unrounded.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Outcome, Side, SpellId};

/// Multi-Shot's `ApplyEffects`: a shot on each of min(3, targets) targets from the cast
/// target on, each rolling its own weapon damage on the cast target's ranged attack power,
/// every result calculated before one travel deals them in turn.
pub(crate) fn multi_shot<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let hits = fight.targets.len().min(3);
    let mut results = Vec::with_capacity(hits);
    let mut current = target;
    for _ in 0..hits {
        let attack_power = fight.ranged_attack_power();
        let base = fight.ranged_normalized_weapon_damage(attack_power);
        results.push(fight.calc_physical_damage(
            spell,
            current,
            base,
            PhysicalOutcome::RangedHitAndCrit { count: true },
        ));
        current = fight.next_target(current);
    }
    fight.deal_damage_after_travel_batch(spell, &results);
}

/// Aimed and Sniper Shot's `ApplyEffects`.
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

/// Arcane Shot's `ApplyEffects`: its flat damage and ranged attack power share, plus its spell
/// power share, on the ranged hit and crit table, dealt after travel.
pub(crate) fn arcane_shot<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    base: f64,
    share: f64,
) {
    // Go's arm64 build fuses the share's multiply into the add.
    let base = share.mul_add(fight.ranged_attack_power(), base);
    let table = PhysicalOutcome::RangedHitAndCrit { count: true };
    let result = fight.calc_damage_with(spell, target, base, Outcome::Table(table));
    fight.deal_damage_after_travel(spell, result);
}

/// Go RegisterRangedSpell's `CastConfig.CastTime`, which its `ModifyCast` applies.
pub(crate) fn cast_time<A: Agent>(fight: &Fight<A>, spell: SpellId) -> Option<i64> {
    let cast_time = fight.spells[spell].default_cast.cast_time;
    (cast_time > 0).then(|| (cast_time as f64 / fight.ranged_haste_multiplier()) as i64)
}
