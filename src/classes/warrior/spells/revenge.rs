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

/// Go spelldata `Effect.Roll` of the client row, a physical school damage row: it lands on a
/// whole number under Forever's rules.
pub(crate) fn roll<A: Agent>(fight: &mut Fight<A>, damage: DamageRoll) -> f64 {
    fight.physical_effect_roll(damage.average, damage.variance)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        classes::warrior::agent::WarriorAgent, contracts::prepared_v2::PreparedV2,
        prepare::prepare_json_with, prepare::Rules,
    };
    use std::collections::BTreeMap;

    /// A prepared Protection Warrior's fight, on the production request under the reference's
    /// rules, which read its recorded talents.
    fn fight() -> Fight<WarriorAgent> {
        let request = include_bytes!(
            "../../../../fixtures/mage/prepared-v2/production-protection-warrior.request.json"
        );
        let prepared = prepare_json_with(request, "rolls", Rules::GoReference).unwrap();
        let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
        WarriorAgent::fight(&prepared).unwrap()
    }

    fn rolls(fight: &mut Fight<WarriorAgent>, average: f64, variance: f64) -> Vec<f64> {
        let row = DamageRoll {
            average,
            min: 0.0,
            max: 0.0,
            rolls: true,
            variance,
        };
        (0..40_000).map(|_| roll(fight, row)).collect()
    }

    /// Revenge rank 1 (6572) states 22 with a Variance of 0.154, and in game its hits land on 20
    /// to 24, the two ends rarer than the three between (warrior damage measurements at levels 20
    /// to 24, 7 November 2026): the base points roll 22 +- 1.69 and round to a whole number.
    /// Eviscerate rank 1 (2098) states 3 with a Variance of 1.333 and rolls the same way, 1 to 5.
    #[test]
    fn a_physical_row_rolls_whole_numbers_with_its_ends_rarer() {
        for (average, variance, low, high) in [
            (22.0, 0.1538461595773697, 20, 24),
            (3.0, 1.3333333730697632, 1, 5),
        ] {
            let mut fight = fight();
            fight.physical_rolls_are_whole = true;
            let mut counts = BTreeMap::new();
            let mut sum = 0.0;
            let rolls = rolls(&mut fight, average, variance);
            for roll in &rolls {
                assert_eq!(roll.fract(), 0.0, "{roll} is not whole");
                *counts.entry(*roll as i32).or_insert(0usize) += 1;
                sum += roll;
            }
            let seen: Vec<i32> = counts.keys().copied().collect();
            assert_eq!(seen, (low..=high).collect::<Vec<_>>(), "{average}");
            for middle in (low + 1)..high {
                assert!(
                    counts[&low] < counts[&middle] && counts[&high] < counts[&middle],
                    "{average}: the ends came up {} and {} times, {middle} {}",
                    counts[&low],
                    counts[&high],
                    counts[&middle]
                );
            }
            // The rounding keeps the row's average.
            let mean = sum / rolls.len() as f64;
            assert!((mean - average).abs() < 0.05, "{average}: mean {mean}");
        }
    }

    /// A hunter's Cat has Claw (3009) as the literal range 43 to 59, the row's 51 with a Variance
    /// of 0.314: its hits land on whole numbers too, the ends half as likely.
    #[test]
    fn a_literal_physical_range_rolls_whole_numbers_with_its_ends_rarer() {
        let mut fight = fight();
        fight.physical_rolls_are_whole = true;
        let mut counts = BTreeMap::new();
        for _ in 0..60_000 {
            let roll = fight.physical_range_roll(43.0, 59.0);
            assert_eq!(roll.fract(), 0.0, "{roll} is not whole");
            *counts.entry(roll as i32).or_insert(0usize) += 1;
        }
        assert_eq!(
            counts.keys().copied().collect::<Vec<_>>(),
            (43..=59).collect::<Vec<_>>()
        );
        for middle in 44..59 {
            assert!(counts[&43] < counts[&middle] && counts[&59] < counts[&middle]);
        }
        fight.physical_rolls_are_whole = false;
        assert!((0..100).any(|_| fight.physical_range_roll(43.0, 59.0).fract() != 0.0));
    }

    /// The reference rolls the same span without rounding, and keeps doing so.
    #[test]
    fn the_reference_rolls_the_span_unrounded() {
        let mut fight = fight();
        assert!(!fight.physical_rolls_are_whole);
        let rolls = rolls(&mut fight, 22.0, 0.1538461595773697);
        assert!(rolls.iter().any(|roll| roll.fract() != 0.0));
        // 22 +- 1.692, where the whole roll lands from 20 to 24.
        assert!(rolls
            .iter()
            .all(|roll| (22.0 - 1.6924..=22.0 + 1.6924).contains(roll)));
    }
}
