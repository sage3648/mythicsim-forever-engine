//! Go `spelldata/speed.go`: the haste an attack or cast speed aura adds.

use super::super::dbcenums;
use super::{Effect, Spell};

/// `proto.PseudoStat` numbers (from the pinned schema) for the haste pseudo stats.
pub(crate) mod pseudo_stat {
    pub const MELEE_HASTE_PERCENT: usize = 20;
    pub const RANGED_HASTE_PERCENT: usize = 21;
    pub const SPELL_HASTE_PERCENT: usize = 22;
}

/// Go `stats.PseudoStatsLen`: `len(proto.PseudoStat_name)`.
pub(crate) const PSEUDO_STATS_LEN: usize = 28;

/// Go `SpeedAuraPseudoStats`: the haste pseudo stats each attack or cast speed aura adds its
/// percent to: a raise of the caster's speeds, or a slow of an enemy's where the percent is
/// negative. Unlike ranged hit and crit, ranged haste is not a total including melee: the sim
/// applies melee, ranged and cast speed each on its own. An aura outside the table has none.
pub(crate) fn speed_aura_pseudo_stats(aura: dbcenums::EffectAuraType) -> &'static [usize] {
    use pseudo_stat::{MELEE_HASTE_PERCENT, RANGED_HASTE_PERCENT, SPELL_HASTE_PERCENT};
    match aura {
        dbcenums::A_MOD_ATTACKSPEED => &[MELEE_HASTE_PERCENT],
        dbcenums::A_MOD_MELEE_HASTE_3 => &[MELEE_HASTE_PERCENT],
        dbcenums::A_MOD_RANGED_HASTE => &[RANGED_HASTE_PERCENT],
        dbcenums::A_MOD_MELEE_RANGED_HASTE_2 => &[MELEE_HASTE_PERCENT, RANGED_HASTE_PERCENT],
        dbcenums::A_MOD_CASTING_SPEED_NOT_STACK => &[SPELL_HASTE_PERCENT],
        _ => &[],
    }
}

impl Spell {
    /// The auras that raise a speed of the spell's caster: 23733 (Blinding Light) states 33% cast
    /// speed and 25% melee haste.
    pub(crate) fn speed_effects(&self) -> Vec<&Effect> {
        self.effects
            .iter()
            .filter(|e| {
                e.effect_type == dbcenums::E_APPLY_AURA
                    && e.target[0] == dbcenums::TARGET_UNIT_CASTER
                    && e.base_points > 0.0
                    && !speed_aura_pseudo_stats(e.aura).is_empty()
            })
            .collect()
    }

    /// The haste percents the spell's speed effects add, indexed by `proto.PseudoStat`.
    pub(crate) fn speed_pseudo_stats(&self) -> Vec<f64> {
        let mut pseudo_stats = vec![0.0; PSEUDO_STATS_LEN];
        for e in self.speed_effects() {
            for &pseudo_stat in speed_aura_pseudo_stats(e.aura) {
                pseudo_stats[pseudo_stat] += e.base_points;
            }
        }
        pseudo_stats
    }
}

impl Effect {
    /// Whether the effect's speed aura changes the time between melee attacks.
    pub(crate) fn changes_attack_speed(&self) -> bool {
        speed_aura_pseudo_stats(self.aura).contains(&pseudo_stat::MELEE_HASTE_PERCENT)
    }

    /// Whether the effect's speed aura changes cast times.
    pub(crate) fn changes_cast_speed(&self) -> bool {
        speed_aura_pseudo_stats(self.aura).contains(&pseudo_stat::SPELL_HASTE_PERCENT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::spelldata::store::must_find;

    fn speed_effect(aura: i32, base_points: f64, target: i32) -> Effect {
        Effect {
            effect_type: dbcenums::E_APPLY_AURA,
            aura,
            base_points,
            target: [target, 0],
            ..Effect::default()
        }
    }

    #[test]
    fn speed_effects_raise_the_casters_speeds() {
        let spell = Spell {
            effects: vec![
                speed_effect(
                    dbcenums::A_MOD_CASTING_SPEED_NOT_STACK,
                    33.0,
                    dbcenums::TARGET_UNIT_CASTER,
                ),
                speed_effect(
                    dbcenums::A_MOD_MELEE_RANGED_HASTE_2,
                    25.0,
                    dbcenums::TARGET_UNIT_CASTER,
                ),
                // A slow, an enemy's, and an aura that is no speed are all left out.
                speed_effect(
                    dbcenums::A_MOD_ATTACKSPEED,
                    -20.0,
                    dbcenums::TARGET_UNIT_CASTER,
                ),
                speed_effect(
                    dbcenums::A_MOD_ATTACKSPEED,
                    20.0,
                    dbcenums::TARGET_UNIT_TARGET_ENEMY,
                ),
                speed_effect(dbcenums::A_MOD_STUN, 20.0, dbcenums::TARGET_UNIT_CASTER),
            ],
            ..Spell::default()
        };
        assert_eq!(spell.speed_effects().len(), 2);
        let stats = spell.speed_pseudo_stats();
        assert_eq!(stats.len(), PSEUDO_STATS_LEN);
        assert_eq!(stats[pseudo_stat::SPELL_HASTE_PERCENT], 33.0);
        assert_eq!(stats[pseudo_stat::MELEE_HASTE_PERCENT], 25.0);
        assert_eq!(stats[pseudo_stat::RANGED_HASTE_PERCENT], 25.0);
    }

    #[test]
    fn effects_know_which_speed_they_change() {
        let attack = speed_effect(dbcenums::A_MOD_MELEE_HASTE_3, 5.0, 0);
        assert!(attack.changes_attack_speed() && !attack.changes_cast_speed());
        let cast = speed_effect(dbcenums::A_MOD_CASTING_SPEED_NOT_STACK, 5.0, 0);
        assert!(cast.changes_cast_speed() && !cast.changes_attack_speed());
        let ranged = speed_effect(dbcenums::A_MOD_RANGED_HASTE, 5.0, 0);
        assert!(!ranged.changes_attack_speed() && !ranged.changes_cast_speed());
    }

    /// The rows of the store state the pseudo stat order the table assumes.
    #[test]
    fn pseudo_stats_match_the_schema() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/proto-schema.json")).unwrap();
        let names = schema["enums"]["proto.PseudoStat"].as_array().unwrap();
        assert_eq!(names.len(), PSEUDO_STATS_LEN);
        for (name, number) in [
            (
                "PseudoStatMeleeHastePercent",
                pseudo_stat::MELEE_HASTE_PERCENT,
            ),
            (
                "PseudoStatRangedHastePercent",
                pseudo_stat::RANGED_HASTE_PERCENT,
            ),
            (
                "PseudoStatSpellHastePercent",
                pseudo_stat::SPELL_HASTE_PERCENT,
            ),
        ] {
            let entry = names.iter().find(|n| n["name"] == name).unwrap();
            assert_eq!(entry["number"], number, "{name}");
        }
    }

    /// Blinding Light 23733 states 33% cast speed and 25% melee haste on its caster.
    #[test]
    fn blinding_light_hastes_its_caster() {
        let stats = must_find(23733).speed_pseudo_stats();
        assert_eq!(stats[pseudo_stat::SPELL_HASTE_PERCENT], 33.0);
        assert_eq!(stats[pseudo_stat::MELEE_HASTE_PERCENT], 25.0);
    }
}
