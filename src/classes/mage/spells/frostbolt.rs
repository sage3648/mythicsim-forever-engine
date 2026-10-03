//! Frostbolt 25304: binary hit table, labeled rolls and cast-completion outcomes.

use crate::{
    contracts::Request,
    core::rng::{labeled_seed, SplitMix64},
    mechanics::damage::{binary_hit_chance, Outcome},
    report::{Counts, Work},
};

pub(crate) struct FrostboltRng {
    damage: SplitMix64,
    hit: SplitMix64,
    crit: SplitMix64,
}

impl FrostboltRng {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            damage: SplitMix64::new(labeled_seed(seed, "Damage Roll")),
            hit: SplitMix64::new(labeled_seed(seed, "Magical Hit Roll")),
            crit: SplitMix64::new(labeled_seed(seed, "Magical Crit Roll")),
        }
    }
}

/// Frostbolt is binary: resistance reduces its hit chance rather than damage.
pub fn hit_chance(request: &Request) -> f64 {
    binary_hit_chance(
        request.caster.level,
        request.target.level,
        request.target.frost_resistance,
        request.caster.spell_penetration,
        request.caster.hit_percent,
    )
}

pub(crate) fn resolve_cast(
    request: &Request,
    hit_chance: f64,
    rng: &mut FrostboltRng,
    counts: &mut Counts,
    work: &mut Work,
) -> (f64, Outcome) {
    let base = if request.spell.max_damage > request.spell.min_damage {
        work.damage_rolls += 1;
        request.spell.min_damage
            + (request.spell.max_damage - request.spell.min_damage) * rng.damage.next_f64()
    } else {
        request.spell.min_damage
    };
    let mut damage = (base + request.spell.coefficient * request.caster.spell_power)
        * request.spell.damage_multiplier;
    work.hit_rolls += 1;
    let outcome = if rng.hit.next_f64() >= hit_chance {
        damage = 0.0;
        Outcome::Miss
    } else {
        work.crit_rolls += 1;
        if rng.crit.next_f64() < request.caster.crit_percent / 100.0 {
            damage *= request.spell.crit_multiplier;
            Outcome::Crit
        } else {
            Outcome::Hit
        }
    };
    // Go records the outcome when CalcDamage runs at cast
    // completion. Damage is recorded only on missile arrival.
    match outcome {
        Outcome::Hit => counts.hits += 1,
        Outcome::Crit => counts.crits += 1,
        Outcome::Miss => counts.misses += 1,
    }
    (damage, outcome)
}
