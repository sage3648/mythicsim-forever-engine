//! Go `spelldata/debuff.go`: the slows and stat changes a spell puts on the enemy it lands on.
//!
//! `resistance_stats`, `damage_done_stats` and `is_one_school` are Go's `parse_effects_table.go`
//! helpers of the same names, ported here because `debuffsAStat` reads them. They answer with
//! the sim's [`Stat`]s.

use super::super::dbcenums;
use super::super::stats::Stat;
use super::effect::{effects_on, AURA_ON_ENEMY};
use super::{Effect, Spell};

/// Go `miscAllSchools`: every school bit, which the sim states as one multiplier.
pub(crate) const MISC_ALL_SCHOOLS: i32 = 127;
/// Go `miscMagicSchool`: every school but physical, which is what spell damage covers.
pub(crate) const MISC_MAGIC_SCHOOL: i32 = 126;
/// Go `miscArmor`: `A_MOD_RESISTANCE` and `A_MOD_BASE_RESISTANCE_PCT` state armor as school 1.
pub(crate) const MISC_ARMOR: i32 = 1;

const SCHOOL_PHYSICAL: i32 = 1;
const SCHOOL_HOLY: i32 = 2;
const SCHOOL_FIRE: i32 = 4;
const SCHOOL_NATURE: i32 = 8;
const SCHOOL_FROST: i32 = 16;
const SCHOOL_SHADOW: i32 = 32;
const SCHOOL_ARCANE: i32 = 64;

/// Go `isOneSchool`.
pub(crate) fn is_one_school(mask: i32) -> bool {
    mask > 0 && mask & (mask - 1) == 0 && mask <= SCHOOL_ARCANE
}

/// Go `SpellSchool.ResistanceStat` for a single school bit: `None` where Go answers stat 0,
/// which is holy (no resistance exists) and every school combination.
fn resistance_stat(school: i32) -> Option<Stat> {
    match school {
        SCHOOL_PHYSICAL => Some(Stat::ArmorPenetration),
        SCHOOL_ARCANE => Some(Stat::ArcaneResistance),
        SCHOOL_FIRE => Some(Stat::FireResistance),
        SCHOOL_FROST => Some(Stat::FrostResistance),
        SCHOOL_NATURE => Some(Stat::NatureResistance),
        SCHOOL_SHADOW => Some(Stat::ShadowResistance),
        _ => None,
    }
}

/// Go `SpellSchool.SchoolDamage`: the damage stat of a school, spell damage for anything else.
fn school_damage(school: i32) -> Stat {
    match school {
        SCHOOL_ARCANE => Stat::ArcaneDamage,
        SCHOOL_FIRE => Stat::FireDamage,
        SCHOOL_FROST => Stat::FrostDamage,
        SCHOOL_HOLY => Stat::HolyDamage,
        SCHOOL_NATURE => Stat::NatureDamage,
        SCHOOL_SHADOW => Stat::ShadowDamage,
        _ => Stat::SpellDamage,
    }
}

/// Go `damageDoneStats`: the stats a flat damage mask names. The sim keeps one stat for physical
/// damage and one for magic, plus a stat per magic school, so a mask of every school is the
/// first two together.
pub(crate) fn damage_done_stats(mask: i32) -> Vec<Stat> {
    if mask == MISC_ALL_SCHOOLS {
        vec![Stat::PhysicalDamage, Stat::SpellDamage]
    } else if mask == MISC_MAGIC_SCHOOL {
        vec![Stat::SpellDamage]
    } else if mask == SCHOOL_PHYSICAL {
        vec![Stat::PhysicalDamage]
    } else if is_one_school(mask) {
        vec![school_damage(mask)]
    } else {
        Vec::new()
    }
}

/// Go `resistanceStats`: the stats a resistance mask names. School 1 is armor, and holy has no
/// resistance stat to take.
pub(crate) fn resistance_stats(mask: i32) -> Vec<Stat> {
    let mut out = Vec::new();
    if mask & MISC_ARMOR != 0 {
        out.push(Stat::Armor);
    }

    let mut bit = SCHOOL_HOLY;
    while bit <= SCHOOL_ARCANE {
        if mask & bit != 0 {
            if let Some(stat) = resistance_stat(bit) {
                out.push(stat);
            }
        }
        bit <<= 1;
    }
    out
}

/// Whether the effect changes a stat of the enemy the spell lands on, in the stats `ParseEffects`
/// applies to a unit: armor and resistances, attack power, flat damage done. Annihilator's Armor
/// Shatter 16928 takes 165 armor per stack.
fn debuffs_a_stat(e: &Effect) -> bool {
    match e.aura {
        dbcenums::A_MOD_RESISTANCE => !resistance_stats(e.misc).is_empty(),
        dbcenums::A_MOD_DAMAGE_DONE => !damage_done_stats(e.misc).is_empty(),
        dbcenums::A_MOD_ATTACK_POWER | dbcenums::A_MOD_RANGED_ATTACK_POWER => true,
        _ => false,
    }
}

impl Spell {
    /// The positions, counted from 1 the way `effect_n` counts, of the effects that slow the
    /// enemy the spell lands on: a speed aura of a negative value on an enemy target that
    /// changes its attack or cast speed, as Frostguard's Chilled 16927 states. A row that stacks
    /// answers none, since an exclusive slow cannot follow stacks.
    pub(crate) fn slow_effects(&self) -> Vec<i32> {
        self.enemy_effects(|e| self.slows(e))
    }

    /// The positions of the slows and stat changes the spell puts on the enemy it lands on, in
    /// order.
    pub(crate) fn debuff_effects(&self) -> Vec<i32> {
        self.enemy_effects(|e| self.slows(e) || debuffs_a_stat(e))
    }

    fn enemy_effects(&self, matches: impl Fn(&Effect) -> bool) -> Vec<i32> {
        let mut positions = effects_on(self, AURA_ON_ENEMY);
        positions.retain(|&i| matches(self.effect_n(i)));
        positions
    }

    fn slows(&self, e: &Effect) -> bool {
        self.max_stack <= 0
            && (e.changes_attack_speed() || e.changes_cast_speed())
            && e.base_points < 0.0
    }

    /// Whether the row puts a debuff the sim models on the enemy it lands on.
    pub(crate) fn debuffs_the_target(&self) -> bool {
        !self.debuff_effects().is_empty()
    }

    /// Whether any aura the row applies lands on an enemy. Such a row is never a buff on the
    /// wearer.
    pub(crate) fn applies_an_aura_to_an_enemy(&self) -> bool {
        !effects_on(self, AURA_ON_ENEMY).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::spelldata::store::must_find;

    fn aura_on_enemy(aura: i32, misc: i32, base_points: f64) -> Effect {
        Effect {
            effect_type: dbcenums::E_APPLY_AURA,
            aura,
            misc,
            base_points,
            target: [dbcenums::TARGET_UNIT_TARGET_ENEMY, 0],
            ..Effect::default()
        }
    }

    #[test]
    fn school_masks_name_stats() {
        assert_eq!(
            damage_done_stats(127),
            [Stat::PhysicalDamage, Stat::SpellDamage]
        );
        assert_eq!(damage_done_stats(126), [Stat::SpellDamage]);
        assert_eq!(damage_done_stats(1), [Stat::PhysicalDamage]);
        assert_eq!(damage_done_stats(16), [Stat::FrostDamage]);
        assert_eq!(damage_done_stats(2), [Stat::HolyDamage]);
        assert!(damage_done_stats(20).is_empty());
        assert!(damage_done_stats(0).is_empty());

        assert_eq!(resistance_stats(1), [Stat::Armor]);
        assert_eq!(
            resistance_stats(1 | 4 | 64),
            [Stat::Armor, Stat::FireResistance, Stat::ArcaneResistance]
        );
        // Holy has no resistance stat to take.
        assert!(resistance_stats(2).is_empty());
        assert_eq!(
            resistance_stats(8 | 16 | 32),
            [
                Stat::NatureResistance,
                Stat::FrostResistance,
                Stat::ShadowResistance
            ]
        );
    }

    #[test]
    fn a_slow_is_a_negative_speed_aura_on_an_enemy_that_does_not_stack() {
        let slow = aura_on_enemy(dbcenums::A_MOD_ATTACKSPEED, 0, -20.0);
        let cast_slow = aura_on_enemy(dbcenums::A_MOD_CASTING_SPEED_NOT_STACK, 0, -10.0);
        let haste = aura_on_enemy(dbcenums::A_MOD_ATTACKSPEED, 0, 20.0);
        let armor = aura_on_enemy(dbcenums::A_MOD_RESISTANCE, 1, -165.0);
        let holy = aura_on_enemy(dbcenums::A_MOD_RESISTANCE, 2, -165.0);
        let spell = Spell {
            effects: vec![slow, cast_slow, haste, armor, holy],
            ..Spell::default()
        };
        assert_eq!(spell.slow_effects(), [1, 2]);
        assert_eq!(spell.debuff_effects(), [1, 2, 4]);
        assert!(spell.debuffs_the_target());
        assert!(spell.applies_an_aura_to_an_enemy());

        // A row that stacks answers no slow, only the stat change.
        let stacking = Spell {
            max_stack: 5,
            ..spell
        };
        assert!(stacking.slow_effects().is_empty());
        assert_eq!(stacking.debuff_effects(), [4]);
    }

    #[test]
    fn a_wearer_aura_debuffs_nothing() {
        let mut slow = aura_on_enemy(dbcenums::A_MOD_ATTACKSPEED, 0, -20.0);
        slow.target = [dbcenums::TARGET_UNIT_CASTER, 0];
        let spell = Spell {
            effects: vec![slow],
            ..Spell::default()
        };
        assert!(!spell.debuffs_the_target());
        assert!(!spell.applies_an_aura_to_an_enemy());
    }

    /// Armor Shatter 16928 takes armor off the enemy.
    #[test]
    fn armor_shatter_debuffs_the_target() {
        let spell = must_find(16928);
        assert!(spell.debuffs_the_target(), "{:?}", spell.effects);
    }
}
