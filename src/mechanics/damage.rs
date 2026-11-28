//! Magic attack outcomes and shared binary hit-table math.

#[derive(Clone, Copy, Debug)]
pub(crate) enum Outcome {
    Hit,
    Crit,
    Miss,
}

pub(crate) fn binary_hit_chance(
    caster_level: u32,
    target_level: u32,
    school_resistance: f64,
    spell_penetration: f64,
    hit_percent: f64,
) -> f64 {
    let base_miss = match target_level {
        60 => 0.04,
        61 => 0.05,
        62 => 0.06,
        _ => 0.17,
    };
    let resistance = (school_resistance - spell_penetration).max(0.0);
    let coefficient = (resistance / (5.0 * f64::from(caster_level))).min(1.0);
    let base_hit = (1.0 - base_miss) * (1.0 - 0.75 * coefficient);
    (base_hit + hit_percent / 100.0).min(0.99)
}

/// Go `levelBasedResist`: 2% average mitigation per level an enemy is above the caster.
pub(crate) fn level_based_resist(defender_level: i32, attacker_level: i32) -> f64 {
    if defender_level > attacker_level {
        0.02 * f64::from(defender_level - attacker_level)
    } else {
        0.0
    }
}

/// Go `Unit.resistCoeff` for an enemy defender and a magic school.
pub(crate) fn resist_coefficient(
    raw_resistance: f64,
    spell_piercing: f64,
    attacker_level: i32,
    defender_level: i32,
    binary: bool,
) -> f64 {
    let resistance = (raw_resistance - spell_piercing).max(0.0);
    let level = level_based_resist(defender_level, attacker_level);
    if resistance <= 0.0 {
        if binary {
            return 0.0;
        }
        return (level / 0.75).min(1.0);
    }
    let cap = f64::from(attacker_level * 5);
    let mut coefficient = resistance / cap;
    if !binary && defender_level > attacker_level {
        coefficient += level * 1.0 / 0.75;
    }
    coefficient.min(1.0)
}

/// Go `binaryHitChance`'s 1 - 0.75 * coefficient, fused by the arm64 build.
pub(crate) fn binary_resist_hit(coefficient: f64) -> f64 {
    (-0.75f64).mul_add(coefficient, 1.0)
}

/// Go `partialResistRollThresholds`: rolls above the first threshold resist nothing. The
/// arm64 build fuses the coefficient times 3 into the step's subtraction and each line's
/// multiply into its add.
pub(crate) fn partial_resist_thresholds(coefficient: f64) -> (f64, f64, f64) {
    let value = coefficient * 3.0;
    if value <= 1.0 {
        (0.76 * value, 0.21 * value, 0.03 * value)
    } else if value <= 2.0 {
        let value = coefficient.mul_add(3.0, -1.0);
        (
            0.24f64.mul_add(value, 0.76),
            0.57f64.mul_add(value, 0.21),
            0.19f64.mul_add(value, 0.03),
        )
    } else {
        let value = coefficient.mul_add(3.0, -2.0);
        (
            1.0,
            0.18f64.mul_add(value, 0.78),
            0.58f64.mul_add(value, 0.22),
        )
    }
}

/// The Spell Piercing bonus of one hit under Forever's rules, in tenths of its damage: the
/// partial resist roll turned over, around the average share `average`. Up to an average of 10%
/// a hit gains nothing, a tenth or two tenths at 1 - 7.5 x average, 5 x average and
/// 2.5 x average; from 90% the same mirrored from ten tenths; in between each tenth within 20%
/// of the average at 0.5 - 2.5 x its distance. `roll` is a uniform draw.
///
/// Public Forever logs of 14 to 18 November 2026 (forever.warcraftlogs.com, client
/// 1.60.1.70938 and 70959, 18 casters in 45 dungeon reports, on mobs that never resisted the
/// school): 2,476 direct hits gained damage, 1,114 of the non-crits by a tenth, 590 by two
/// tenths and 27 by three. A shaman with 8 Spell Piercing on gear gained on 71 of 518 direct
/// hits (1.8% on average; 8 / 507 is 1.6%) and a warlock with 6 on 29 of 381 (1.0%; 6 / 505 is
/// 1.2%).
pub(crate) fn spell_piercing_tenths(average: f64, roll: f64) -> u32 {
    if average <= 0.0 {
        return 0;
    }
    if average <= 0.1 {
        return if roll < 1.0 - 7.5 * average {
            0
        } else if roll < 1.0 - 2.5 * average {
            1
        } else {
            2
        };
    }
    if average >= 0.9 {
        let rest = 1.0 - average;
        return if roll < 1.0 - 7.5 * rest {
            10
        } else if roll < 1.0 - 2.5 * rest {
            9
        } else {
            8
        };
    }
    let mut cumulative = 0.0;
    let mut last = 0;
    for tenths in 0..=10u32 {
        let chance = 0.5 - 2.5 * (f64::from(tenths) / 10.0 - average).abs();
        if chance <= 0.0 {
            continue;
        }
        cumulative += chance;
        last = tenths;
        if roll < cumulative {
            return tenths;
        }
    }
    last
}

/// Go `SpellChanceToMiss`.
pub(crate) fn spell_chance_to_miss(
    base_spell_miss_chance: f64,
    binary_hit_chance: Option<f64>,
    spell_hit_chance: f64,
) -> f64 {
    let mut base_hit = 1.0 - base_spell_miss_chance;
    if let Some(binary) = binary_hit_chance {
        base_hit *= binary;
    }
    let hit = base_hit + spell_hit_chance;
    (1.0 - hit).max(0.01)
}

/// Go `Spell.CritDamageMultiplier`: multiplicative modifiers scale the base, additive
/// ones scale only the bonus above 100%.
pub(crate) fn crit_damage_multiplier(
    magic: bool,
    crit_multiplier_pct: f64,
    unit_crit_damage_multiplier: f64,
    table_crit_multiplier: f64,
    crit_multiplier_additive: f64,
) -> f64 {
    let base = if magic { 1.5 } else { 2.0 };
    // The arm64 build fuses the table multiplier into the subtraction and the additive
    // scale into the final add.
    let bonus = (base * crit_multiplier_pct * unit_crit_damage_multiplier)
        .mul_add(table_crit_multiplier, -1.0);
    bonus.mul_add(crit_multiplier_additive + 1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Inputs where Go's arm64 fused forms and a rounded product disagree in the last bit; the
    // expected values are the single-rounding results the reference computes.

    #[test]
    fn crit_damage_multiplier_fuses_the_table_and_additive_scales() {
        let fused = crit_damage_multiplier(false, 1.0, 1.29, 1.13, 0.7);
        assert_eq!(fused, 4.25618);
        assert_ne!(fused, (2.0 * 1.29 * 1.13 - 1.0) * 1.7 + 1.0);
    }

    #[test]
    fn partial_resist_thresholds_fuse_each_step() {
        let (none, _, _) = partial_resist_thresholds(0.3384242699249872);
        assert_eq!(none, 0.7636654743459907);
    }

    #[test]
    fn spell_piercing_tenths_average_the_share() {
        // 15 Spell Piercing at level 60: 15 / 514.
        let average = 15.0 / 514.0;
        assert_eq!(spell_piercing_tenths(average, 0.0), 0);
        assert_eq!(
            spell_piercing_tenths(average, 1.0 - 7.5 * average - 1e-9),
            0
        );
        assert_eq!(
            spell_piercing_tenths(average, 1.0 - 7.5 * average + 1e-9),
            1
        );
        assert_eq!(
            spell_piercing_tenths(average, 1.0 - 2.5 * average + 1e-9),
            2
        );
        assert_eq!(spell_piercing_tenths(0.0, 0.999), 0);
        for average in [0.01, 0.05, 0.1, 0.25, 0.5, 0.75, 0.95] {
            let steps = 200_000;
            let mean = (0..steps)
                .map(|i| {
                    let roll = (f64::from(i) + 0.5) / f64::from(steps);
                    0.1 * f64::from(spell_piercing_tenths(average, roll))
                })
                .sum::<f64>()
                / f64::from(steps);
            assert!((mean - average).abs() < 1e-3, "{average}: {mean}");
        }
    }

    #[test]
    fn binary_resist_hit_fuses_the_coefficient() {
        assert_eq!(binary_resist_hit(0.7609477375418205), 0.4292891968436346);
        assert_ne!(1.0 - 0.75 * 0.7609477375418205, 0.4292891968436346);
    }
}
