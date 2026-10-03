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

/// Go `partialResistRollThresholds`: rolls above the first threshold resist nothing.
pub(crate) fn partial_resist_thresholds(coefficient: f64) -> (f64, f64, f64) {
    let value = coefficient * 3.0;
    if value <= 1.0 {
        (0.76 * value, 0.21 * value, 0.03 * value)
    } else if value <= 2.0 {
        let value = value - 1.0;
        (
            0.76 + 0.24 * value,
            0.21 + 0.57 * value,
            0.03 + 0.19 * value,
        )
    } else {
        let value = value - 2.0;
        (1.0, 0.78 + 0.18 * value, 0.22 + 0.58 * value)
    }
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
    (base * crit_multiplier_pct * unit_crit_damage_multiplier * table_crit_multiplier - 1.0)
        * (crit_multiplier_additive + 1.0)
        + 1.0
}
