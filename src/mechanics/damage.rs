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
