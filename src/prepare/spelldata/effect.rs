//! Go `spelldata/effect.go`: an effect's amount and the other columns it is read through.
//!
//! The target and aura-landing lookups at the end belong to Go's `resolve_spell.go`,
//! `item_aura.go` and `parse_effects.go`. They are here because `debuff.go` reads them and they
//! are plain row lookups; whoever ports those files should use these.

use super::super::dbcenums;
use super::store::find;
use super::{duration_from_millis, duration_from_millis_f64, Effect, Spell};
use crate::prepare::sim::Duration;

impl Effect {
    /// Whether the effect's class mask names the spell: the spell is in the effect's family and
    /// carries one of the mask's bits. An effect with an empty mask names none, which the client
    /// uses to switch an effect off.
    pub(crate) fn covers(&self, spell: &Spell) -> bool {
        self.class_flags.matches(&spell.class_flags)
    }

    /// The client's own number, before any unit the aura gives it.
    pub(crate) fn base_value(&self) -> f64 {
        self.base_points
    }

    /// The client states a percentage as an integer: Improved Righteous Fury reads 16, not
    /// 0.16.
    pub(crate) fn percent(&self) -> f64 {
        self.base_points / 100.0
    }

    /// The client states rage on a 0-1000 bar: a -30 cost modifier is 3 rage, Charge's energize
    /// of 150 is 15.
    pub(crate) fn tenths(&self) -> f64 {
        self.base_points / 10.0
    }

    /// The value read as a time, which is the unit a duration or delay modifier states it in.
    pub(crate) fn time_value(&self) -> Duration {
        duration_from_millis_f64(self.base_points)
    }

    /// `EffectAuraPeriod`: how long one tick of the aura lasts.
    pub(crate) fn period(&self) -> Duration {
        duration_from_millis(self.period_ms)
    }

    pub(crate) fn coeff(&self) -> f64 {
        self.sp_coef
    }

    pub(crate) fn ap_coeff(&self) -> f64 {
        self.ap_coef
    }

    /// The spell this effect fires, or Nil where it fires nothing.
    pub(crate) fn trigger(&self) -> &'static Spell {
        find(self.trigger_id)
    }

    /// The amount at a caster level: the base points plus the per-level gain over the spell's own
    /// level, stopping at the level the spell stops scaling at. Both levels are the owning
    /// spell's, stamped onto the effect at generation, so an effect answers the same amount
    /// however the caller reached it.
    ///
    /// The base keeps the client's own `EffectBasePointsF`, which is fractional on about one
    /// effect in 55, and the per-level gain is added on top of the fraction before the whole is
    /// floored.
    ///
    /// float32 is load-bearing: `EffectRealPointsPerLevel` is a float32 widened into the DB
    /// (3.79999995231628), and multiplying in float64 moves the result off the tooltip on six
    /// rows. The client resolves the amount to a whole number, which is the floor below. Go's
    /// explicit float32 conversions stop it fusing the multiply and the add, so neither is
    /// fused here.
    pub(crate) fn average(&self, level: i32) -> f64 {
        // MaxLevel 0 is the client's "no cap", which a spell with no SpellLevels row carries.
        let mut lvl = level;
        if self.max_level > 0 && i32::from(self.max_level) < lvl {
            lvl = i32::from(self.max_level);
        }

        let mut delta = lvl.wrapping_sub(i32::from(self.spell_level));
        if delta < 0 {
            delta = 0;
        }

        let gain: f32 = delta as f32 * self.ppl as f32;
        let base: f32 = self.base_points as f32 + gain;
        f64::from(base).floor()
    }

    /// The low end of the roll. `EffectVariance` is the spread around the average, so an effect
    /// that does not roll answers the average at both ends.
    pub(crate) fn min(&self, level: i32) -> f64 {
        self.average(level) * (1.0 - self.variance / 2.0)
    }

    pub(crate) fn max(&self, level: i32) -> f64 {
        self.average(level) * (1.0 + self.variance / 2.0)
    }

    /// The amount for one cast: rolled where the client states a spread, the average where it
    /// does not. Go `Roll`; `roll` is `sim.Roll(min, max)`.
    pub(crate) fn roll(&self, level: i32, roll: impl FnOnce(f64, f64) -> f64) -> f64 {
        let average = self.average(level);
        if self.variance == 0.0 {
            return average;
        }
        roll(
            average * (1.0 - self.variance / 2.0),
            average * (1.0 + self.variance / 2.0),
        )
    }

    /// Go `HitsAnEnemy` (resolve_spell.go): whether either of the effect's implicit targets is
    /// an enemy.
    pub(crate) fn hits_an_enemy(&self) -> bool {
        targets_an_enemy(self.target[0]) || targets_an_enemy(self.target[1])
    }

    /// Go `HitsAnArea` (resolve_spell.go): whether either of the effect's implicit targets picks
    /// the enemies in an area. The area is often the second: Shard of the Fallen Star states
    /// `TARGET_DEST_TARGET_ENEMY`, then `TARGET_UNIT_DEST_AREA_ENEMY`.
    pub(crate) fn hits_an_area(&self) -> bool {
        AREA_ENEMY_TARGETS.contains(&self.target[0]) || AREA_ENEMY_TARGETS.contains(&self.target[1])
    }

    /// Go `AuraTarget` (item_aura.go): where the effect's aura lands, or 0 for an implicit target
    /// an item's aura does not reach. Any enemy target is the enemy the item answers or is used
    /// on.
    pub(crate) fn aura_target(&self) -> AuraTarget {
        if self.hits_an_enemy() {
            return AURA_ON_ENEMY;
        }
        match self.target[0] {
            dbcenums::TARGET_UNIT_CASTER => AURA_ON_WEARER,
            dbcenums::TARGET_UNIT_PET => AURA_ON_PET,
            _ => 0,
        }
    }
}

/// Go `AuraTarget` (item_aura.go): the unit an item's aura effect lands on. 0 is none.
pub(crate) type AuraTarget = u8;
pub(crate) const AURA_ON_WEARER: AuraTarget = 1;
pub(crate) const AURA_ON_PET: AuraTarget = 2;
pub(crate) const AURA_ON_ENEMY: AuraTarget = 3;

/// `ImplicitTarget` values that name an enemy unit, or a point or area chosen on one.
const ENEMY_TARGETS: [i32; 15] = [
    dbcenums::TARGET_UNIT_NEARBY_ENEMY,
    dbcenums::TARGET_UNIT_TARGET_ENEMY,
    dbcenums::TARGET_UNIT_SRC_AREA_ENEMY,
    dbcenums::TARGET_UNIT_DEST_AREA_ENEMY,
    dbcenums::TARGET_UNIT_CONE_ENEMY_24,
    dbcenums::TARGET_DEST_DYNOBJ_ENEMY,
    dbcenums::TARGET_DEST_TARGET_ENEMY,
    dbcenums::TARGET_UNIT_CONE_180_DEG_ENEMY,
    dbcenums::TARGET_UNIT_CONE_CASTER_TO_DEST_ENEMY,
    dbcenums::TARGET_UNIT_SRC_AREA_FURTHEST_ENEMY,
    dbcenums::TARGET_UNIT_AND_DEST_LAST_ENEMY,
    dbcenums::TARGET_UNIT_CASTER_AREA_ENEMY_CLUMP,
    dbcenums::TARGET_DEST_CASTER_ENEMY_CLUMP_CENTROID,
    dbcenums::TARGET_UNIT_RECT_CASTER_ENEMY,
    dbcenums::TARGET_UNIT_LINE_CASTER_TO_DEST_ENEMY,
];

/// The enemy targets that pick every enemy in an area, cone, rectangle or line rather than one
/// unit.
const AREA_ENEMY_TARGETS: [i32; 9] = [
    dbcenums::TARGET_UNIT_SRC_AREA_ENEMY,
    dbcenums::TARGET_UNIT_DEST_AREA_ENEMY,
    dbcenums::TARGET_UNIT_CONE_ENEMY_24,
    dbcenums::TARGET_UNIT_CONE_180_DEG_ENEMY,
    dbcenums::TARGET_UNIT_CONE_CASTER_TO_DEST_ENEMY,
    dbcenums::TARGET_UNIT_SRC_AREA_FURTHEST_ENEMY,
    dbcenums::TARGET_UNIT_CASTER_AREA_ENEMY_CLUMP,
    dbcenums::TARGET_UNIT_RECT_CASTER_ENEMY,
    dbcenums::TARGET_UNIT_LINE_CASTER_TO_DEST_ENEMY,
];

/// Go `TargetsAnEnemy` (resolve_spell.go): whether the implicit target names an enemy unit, or a
/// point or area chosen on one.
pub(crate) fn targets_an_enemy(target: dbcenums::ImplicitTarget) -> bool {
    ENEMY_TARGETS.contains(&target)
}

/// Go `AppliesAura` (parse_effects.go): the effect types that put an aura on someone, the plain
/// application and the area auras, which carry the same aura and misc values.
pub(crate) fn applies_aura(effect_type: dbcenums::SpellEffectType) -> bool {
    effect_type == dbcenums::E_APPLY_AURA
        || effect_type == dbcenums::E_APPLY_AREA_AURA_PARTY
        || effect_type == dbcenums::E_APPLY_AREA_AURA_RAID
}

/// Go `EffectsOn` (item_aura.go): the positions, counted from 1 the way `Effects` counts, of the
/// row's aura effects that land on the target.
pub(crate) fn effects_on(spell: &Spell, target: AuraTarget) -> Vec<i32> {
    spell
        .effects
        .iter()
        .enumerate()
        .filter(|(_, e)| applies_aura(e.effect_type) && e.aura_target() == target)
        .map(|(i, _)| (i + 1) as i32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::spelldata::store::must_find;

    fn damage_effect() -> Effect {
        Effect {
            effect_type: dbcenums::E_SCHOOL_DAMAGE,
            base_points: 19.0,
            ppl: 0.5,
            spell_level: 4,
            max_level: 8,
            variance: 0.105263,
            sp_coef: 0.407,
            ..Effect::default()
        }
    }

    /// Go `TestEffectAverage`: 19 base plus 0.5 a level over the four levels between the spell's
    /// own 4 and its cap of 8, which is where the scaling stops however high the caster is.
    #[test]
    fn average_stops_at_the_levels_cap() {
        let damage = damage_effect();
        assert_eq!(damage.average(60), 21.0);
        assert_eq!(damage.average(6), 20.0);
        assert_eq!(damage.average(2), 19.0);

        let (want_min, want_max) = (21.0 - 21.0 * 0.105263 / 2.0, 21.0 + 21.0 * 0.105263 / 2.0);
        assert!((damage.min(60) - want_min).abs() < 1e-9);
        assert!((damage.max(60) - want_max).abs() < 1e-9);

        // The same numbers off the store's own row.
        let stored = must_find(116).effect_n(2);
        assert_eq!(stored.average(60), 21.0);
        assert_eq!(stored.average(6), 20.0);
        assert_eq!(stored.average(2), 19.0);
    }

    /// Go `TestEffectAverageKeepsTheFraction`: a fractional base keeps its fraction until the
    /// whole amount is floored.
    #[test]
    fn average_keeps_the_fraction() {
        let negative = Effect {
            base_points: -58.4697,
            spell_level: 60,
            ..Effect::default()
        };
        assert_eq!(negative.average(60), -59.0);
        let fractional = Effect {
            base_points: 19.7,
            ppl: 0.5,
            spell_level: 4,
            max_level: 7,
            ..Effect::default()
        };
        assert_eq!(fractional.average(60), 21.0);
        assert_eq!(fractional.average(4), 19.0);
    }

    #[test]
    fn units() {
        let effect = Effect {
            base_points: 150.0,
            period_ms: 3000,
            ..Effect::default()
        };
        assert_eq!(effect.base_value(), 150.0);
        assert_eq!(effect.percent(), 1.5);
        assert_eq!(effect.tenths(), 15.0);
        assert_eq!(effect.time_value(), 150_000_000);
        assert_eq!(effect.period(), 3_000_000_000);
        // A float is truncated toward zero to whole milliseconds.
        let fractional = Effect {
            base_points: -2.9,
            ..Effect::default()
        };
        assert_eq!(fractional.time_value(), -2_000_000);
    }

    #[test]
    fn roll_rolls_only_a_spread() {
        let damage = damage_effect();
        let rolled = damage.roll(60, |lo, hi| (lo + hi) / 2.0);
        assert!((rolled - 21.0).abs() < 1e-9);
        let flat = Effect {
            base_points: 5.0,
            ..Effect::default()
        };
        assert_eq!(
            flat.roll(60, |_, _| panic!("a flat effect does not roll")),
            5.0
        );
    }

    #[test]
    fn covers_by_family_and_mask() {
        use crate::prepare::spelldata::ClassFlags;
        let effect = Effect {
            class_flags: ClassFlags {
                family: 3,
                mask: [0x20, 0, 0, 0],
            },
            ..Effect::default()
        };
        let in_family = Spell {
            class_flags: ClassFlags {
                family: 3,
                mask: [0x30, 0, 0, 0],
            },
            ..Spell::default()
        };
        let other_family = Spell {
            class_flags: ClassFlags {
                family: 4,
                mask: [0x30, 0, 0, 0],
            },
            ..Spell::default()
        };
        assert!(effect.covers(&in_family));
        assert!(!effect.covers(&other_family));
        assert!(!Effect::default().covers(&in_family));
    }

    #[test]
    fn aura_targets() {
        let on_enemy = Effect {
            effect_type: dbcenums::E_APPLY_AURA,
            target: [dbcenums::TARGET_UNIT_TARGET_ENEMY, 0],
            ..Effect::default()
        };
        let on_caster = Effect {
            effect_type: dbcenums::E_APPLY_AURA,
            target: [dbcenums::TARGET_UNIT_CASTER, 0],
            ..Effect::default()
        };
        let on_pet = Effect {
            target: [dbcenums::TARGET_UNIT_PET, 0],
            ..Effect::default()
        };
        assert_eq!(on_enemy.aura_target(), AURA_ON_ENEMY);
        assert_eq!(on_caster.aura_target(), AURA_ON_WEARER);
        assert_eq!(on_pet.aura_target(), AURA_ON_PET);
        assert_eq!(Effect::default().aura_target(), 0);
        // The second target counts for an enemy, not for the wearer.
        let second = Effect {
            target: [
                dbcenums::TARGET_UNIT_CASTER,
                dbcenums::TARGET_UNIT_SRC_AREA_ENEMY,
            ],
            ..Effect::default()
        };
        assert_eq!(second.aura_target(), AURA_ON_ENEMY);
        assert!(second.hits_an_area());
        let spell = Spell {
            effects: vec![on_caster.clone(), on_enemy.clone(), on_pet],
            ..Spell::default()
        };
        assert_eq!(effects_on(&spell, AURA_ON_WEARER), [1]);
        assert_eq!(effects_on(&spell, AURA_ON_ENEMY), [2]);
        assert!(effects_on(&spell, AURA_ON_PET).is_empty());
    }
}
