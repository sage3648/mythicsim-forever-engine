//! Spell haste and cast speed.

/// Go's constant expression `SpellHasteRatingPerHastePercent*100`.
const SPELL_HASTE_RATING_PER_PERCENT_TIMES_100: f64 = 1000.0;

/// Go `updateCastSpeed`: the reciprocal of `TotalSpellHasteMultiplier`.
pub(crate) fn cast_speed(cast_speed_multiplier: f64, spell_haste_rating: f64) -> f64 {
    1.0 / (cast_speed_multiplier
        * (1.0 + spell_haste_rating / SPELL_HASTE_RATING_PER_PERCENT_TIMES_100))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_go_cast_speed() {
        assert_eq!(cast_speed(1.0, 0.0), 1.0);
        assert_eq!(cast_speed(1.0, 100.0), 0.9090909090909091);
        // Berserking on and off: Go multiplies by the reciprocal, which need not round trip.
        let multiplier = 1.01 * 1.1 * (1.0 / 1.1);
        assert_eq!(cast_speed(multiplier, 0.0), 1.0 / multiplier);
    }
}
