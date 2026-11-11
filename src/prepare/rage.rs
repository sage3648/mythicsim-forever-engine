//! Go sim/core/rage.go: the rage bar a class enables, as far as construction and a reset
//! leave it.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;

use super::sim::{AuraConfig, EventCallbacks, PowerBar, Sim, UnitId, NEVER_EXPIRES};
use super::spell::SpellConfig;

/// Go `ThreatPerRageGained`.
pub(crate) const THREAT_PER_RAGE_GAINED: f64 = 5.0;
/// Go `CritRageMultiplier`: the beta development notes of 1 and 8 October 2026 raise a critical
/// auto attack's extra rage from 75% to 100%, for warriors, bears and the cat's bear form alike.
pub(crate) const CRIT_RAGE_MULTIPLIER: f64 = 2.0;
/// Go `BaseRageHitFactor`.
pub(crate) const BASE_RAGE_HIT_FACTOR: f64 = 3.46;
/// Go `TwoHandRageHitFactor`.
pub(crate) const TWO_HAND_RAGE_HIT_FACTOR: f64 = 4.5;
/// Go `TwoHandRageMultiplier`.
pub(crate) const TWO_HAND_RAGE_MULTIPLIER: f64 = TWO_HAND_RAGE_HIT_FACTOR / BASE_RAGE_HIT_FACTOR;

/// Go `RageBarOptions`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RageBarOptions {
    pub max_rage: f64,
    pub starting_rage: f64,
    pub base_rage_multiplier: f64,
}

impl Sim {
    /// Go `Unit.EnableRageBar`: the `RageBar` aura, the spell that holds the rage gain's
    /// metrics and the bar.
    pub(crate) fn enable_rage_bar(&mut self, unit: UnitId, options: RageBarOptions) {
        self.unit_mut(unit).current_power_bar = PowerBar::Rage;
        self.register_aura(
            unit,
            AuraConfig {
                label: "RageBar".to_string(),
                duration: NEVER_EXPIRES,
                on_reset: Some(Rc::new(|sim: &mut Sim, aura| sim.activate(aura))),
                events: EventCallbacks {
                    on_spell_hit_dealt: true,
                    on_spell_hit_taken: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        self.register_spell(
            unit,
            SpellConfig {
                action_id: ActionId {
                    other_id: "OtherActionRageGain".to_string(),
                    ..ActionId::default()
                },
                ..Default::default()
            },
        );
        let max_rage = options.max_rage.max(100.0);
        let bar = &mut self.unit_mut(unit).rage_bar;
        bar.enabled = true;
        bar.max_rage = max_rage;
        bar.starting_rage = f64::max(0.0, f64::min(options.starting_rage, max_rage));
        bar.current_rage = 0.0;
        bar.off_hand_rage_multiplier = 1.0;
        bar.base_rage_multiplier = options.base_rage_multiplier;
    }

    /// Go `Unit.HasRageBar`.
    pub(crate) fn has_rage_bar(&self, unit: UnitId) -> bool {
        self.unit(unit).rage_bar.enabled
    }

    /// Go `rageBar.reset`.
    pub(crate) fn reset_rage_bar(&mut self, unit: UnitId) {
        let bar = &mut self.unit_mut(unit).rage_bar;
        if !bar.enabled {
            return;
        }
        bar.current_rage = bar.starting_rage;
    }
}

/// Go `TwoHandNormalizedSwingSpeed`.
const TWO_HAND_NORMALIZED_SWING_SPEED: f64 = 3.3;

/// tools/oracle-v2/warrior.go `rageBarEffect`: the rage each landed white hit gives, with Go's
/// operation order: the hit factor, halved for the off hand, times the swing speed, the bar's base
/// multiplier and the hand's multiplier, then raised for a two-handed weapon. A critical hit adds
/// the swing's rage before that two-hander factor times `CritRageMultiplier` less one
/// (`CritRageBonusSkipsTwoHand`), which the crit rage values carry. Under Forever's rules the
/// effect carries `crit_multiplies_swing`: a crit multiplies the swing's whole rage instead.
pub(crate) fn rage_bar_effect(
    sim: &Sim,
    unit: UnitId,
    base_rage_multiplier: f64,
) -> serde_json::Value {
    let bar = &sim.unit(unit).rage_bar;
    let auto_attacks = &sim.unit(unit).auto_attacks;
    let hit_rage = |weapon: &super::attack::Weapon, off_hand: bool| {
        let mut hit_factor = BASE_RAGE_HIT_FACTOR;
        let mut hand_multiplier = 1.0;
        if off_hand {
            hit_factor /= 2.0;
            hand_multiplier = bar.off_hand_rage_multiplier;
        }
        let base_rage = hit_factor * weapon.swing_speed * base_rage_multiplier * hand_multiplier;
        let mut generated = base_rage;
        if weapon.normalized_swing_speed == TWO_HAND_NORMALIZED_SWING_SPEED {
            generated *= TWO_HAND_RAGE_MULTIPLIER;
        }
        (generated, base_rage * (CRIT_RAGE_MULTIPLIER - 1.0))
    };
    let (main_hand, main_hand_crit) = hit_rage(&auto_attacks.mh, false);
    let (off_hand, off_hand_crit) = hit_rage(&auto_attacks.oh, true);
    let mut effect = serde_json::json!({
        "kind": "rage_bar", "aura": "RageBar", "max_rage": bar.max_rage,
        "starting_rage": bar.starting_rage,
        "main_hand_rage": main_hand, "off_hand_rage": off_hand,
        "main_hand_crit_rage": main_hand_crit, "off_hand_crit_rage": off_hand_crit,
        "threat_per_rage": THREAT_PER_RAGE_GAINED,
        // Patch 182: the threat of a gain takes the unit's threat multiplier at the gain and is split
        // between the targets in combat.
        "threat_multiplied": true,
    });
    // Forever's combat logs of 9 October 2026: a two-hander's critical white hit gives exactly
    // twice the swing's rage, two-hander factor included (src/core/fight/rage.rs).
    // Forever combat logs: rage gained before the pull, as a prepull Bloodrage's, makes no
    // threat.
    if sim.rules == super::sim::Rules::Forever {
        effect["crit_multiplies_swing"] = serde_json::json!(true);
        effect["no_threat_before_pull"] = serde_json::json!(true);
        if let Some(attacker) = sim.unit(unit).current_target {
            effect["damage_taken_rage_per_damage"] =
                serde_json::json!(damage_taken_rage_per_damage(sim.unit(attacker).level));
        }
    }
    effect
}

/// The Go reference's rage from a hit taken is 10 times its damage before armor over the
/// defender's maximum health. Under Forever's rules it is `2.5 * (1 - A) * damage / c`, where
/// the damage is the hit before armor, resistances, block and absorbs, `c` is the rage
/// conversion value at the attacker's level (230.6 at 60) and `A` is an armor reduction the game
/// assumes in place of the defender's own. The defender's level, health and armor play no part.
/// The game build's notes describe the change: rage balanced around 20% to 40% armor by level
/// instead of 50%.
///
/// `A` by the attacker's level is measured on combat logs of warriors and bears taking melee
/// hits (client 1.60.1.70862 and earlier): 21.8% from attackers of levels 17 to 38, 23.9% at 42
/// and 31% at 44 on hundreds of hits each, 36% at 46 and 38% at 50 on a few dozen. The 40% at
/// 63 is the top of the notes' range and is not measured; no log covers attackers of 47 to 49
/// or 51 to 63. Levels between two rows take the line between them.
const DAMAGE_TAKEN_RAGE_FACTOR_FOREVER: f64 = 2.5;
const DAMAGE_TAKEN_RAGE_ASSUMED_ARMOR: [(i32, f64); 6] = [
    (38, 0.218),
    (42, 0.239),
    (44, 0.31),
    (46, 0.36),
    (50, 0.38),
    (63, 0.40),
];

fn damage_taken_rage_assumed_armor(attacker_level: i32) -> f64 {
    let rows = &DAMAGE_TAKEN_RAGE_ASSUMED_ARMOR;
    if attacker_level <= rows[0].0 {
        return rows[0].1;
    }
    for pair in rows.windows(2) {
        let (low, high) = (pair[0], pair[1]);
        if attacker_level <= high.0 {
            return low.1
                + (high.1 - low.1) * f64::from(attacker_level - low.0) / f64::from(high.0 - low.0);
        }
    }
    rows[rows.len() - 1].1
}

/// The rage conversion value at a level.
fn rage_conversion(level: i32) -> f64 {
    let level = f64::from(level);
    0.0091107836 * level * level + 3.225598133 * level + 4.2652911
}

/// The rage a point of damage taken gives under Forever's rules, from an attacker of this level.
pub(crate) fn damage_taken_rage_per_damage(attacker_level: i32) -> f64 {
    DAMAGE_TAKEN_RAGE_FACTOR_FOREVER * (1.0 - damage_taken_rage_assumed_armor(attacker_level))
        / rage_conversion(attacker_level)
}

#[cfg(test)]
mod damage_taken_rage_tests {
    use super::*;

    #[test]
    fn the_assumed_armor_follows_the_measured_rows() {
        assert_eq!(damage_taken_rage_assumed_armor(17), 0.218);
        assert_eq!(damage_taken_rage_assumed_armor(38), 0.218);
        assert!((damage_taken_rage_assumed_armor(40) - 0.2285).abs() < 1e-12);
        assert_eq!(damage_taken_rage_assumed_armor(44), 0.31);
        assert_eq!(damage_taken_rage_assumed_armor(63), 0.40);
        assert_eq!(damage_taken_rage_assumed_armor(70), 0.40);
    }

    #[test]
    fn a_level_60_attacker_converts_at_230_6() {
        assert!((rage_conversion(60) - 230.6).abs() < 0.01);
        // A level 63 boss: 2.5 * 0.6 / 243.64, a point of rage for about 162 damage.
        assert!((1.0 / damage_taken_rage_per_damage(63) - 162.43).abs() < 0.01);
    }
}
