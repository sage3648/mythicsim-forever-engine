//! Go sim/core/weapon_skill.go (the fork's patch 135, ported from wowsims/classic): weapon skill
//! above the 5 a level every character has, kept per skill line from the equipment's pseudo stats
//! and skill auras, resolved per hand by the weapon in it, and the attack table values it moves.
//!
//! An attack takes its hand's skill by proc mask (melee main hand, off hand, ranged). Against an
//! enemy at or above the attacker's level, the gap between the target's defense and that skill
//! sets the miss and dodge chances, the hit suppression past a 10 point gap and how much damage a
//! glancing blow keeps. With no skill above base every lookup returns the table's own field, so a
//! build without weapon skill exports exactly as before.

use super::attack::AttackTable;
use super::items::{self, Item};
use super::sim::{AuraId, Sim, UnitId, UnitType};
use std::rc::Rc;

/// The hands an attack can be made with, in `Unit::weapon_skill` order.
pub(crate) const MAIN_HAND: usize = 0;
pub(crate) const OFF_HAND: usize = 1;
pub(crate) const RANGED: usize = 2;
pub(crate) const HANDS: usize = 3;

/// Every skill line, in proto order from `PseudoStatAxesSkill`.
pub(crate) const NUM_WEAPON_SKILLS: usize = 14;

/// A skill line, as its offset from `PseudoStatAxesSkill`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WeaponSkill {
    Axes,
    Swords,
    Maces,
    Daggers,
    /// Fist weapons.
    Unarmed,
    TwoHandedAxes,
    TwoHandedSwords,
    TwoHandedMaces,
    Polearms,
    Staves,
    Bows,
    Guns,
    Crossbows,
    Thrown,
}

impl WeaponSkill {
    /// The `proto.PseudoStat` index the line's item pseudo stat is kept at.
    fn pseudo_stat(self) -> usize {
        let first =
            crate::contracts::request::enum_number("proto.PseudoStat", "PseudoStatAxesSkill")
                .expect("proto.PseudoStat has PseudoStatAxesSkill");
        first as usize + self as usize
    }
}

/// Go `WeaponSkillOf`: the skill line a weapon is used with; none for no weapon, a shield, an
/// off-hand item, a wand or a relic.
pub(crate) fn weapon_skill_of(item: &Item) -> Option<WeaponSkill> {
    use WeaponSkill::*;
    if item.is_empty() {
        return None;
    }
    match item.ranged_weapon_type.as_str() {
        "RangedWeaponTypeUnknown" => {}
        "RangedWeaponTypeBow" => return Some(Bows),
        "RangedWeaponTypeGun" => return Some(Guns),
        "RangedWeaponTypeCrossbow" => return Some(Crossbows),
        "RangedWeaponTypeThrown" => return Some(Thrown),
        _ => return None,
    }
    if item.hand_type == "HandTypeTwoHand" {
        return match item.weapon_type.as_str() {
            "WeaponTypeAxe" => Some(TwoHandedAxes),
            "WeaponTypeSword" => Some(TwoHandedSwords),
            "WeaponTypeMace" => Some(TwoHandedMaces),
            "WeaponTypePolearm" => Some(Polearms),
            "WeaponTypeStaff" => Some(Staves),
            _ => None,
        };
    }
    match item.weapon_type.as_str() {
        "WeaponTypeAxe" => Some(Axes),
        "WeaponTypeSword" => Some(Swords),
        "WeaponTypeMace" => Some(Maces),
        "WeaponTypeDagger" => Some(Daggers),
        "WeaponTypeFist" => Some(Unarmed),
        _ => None,
    }
}

/// Go `stats.PseudoStatValue` for a skill line.
fn item_skill(item: &Item, skill: WeaponSkill) -> f64 {
    item.pseudo_stats
        .get(skill.pseudo_stat())
        .copied()
        .unwrap_or(0.0)
}

impl Sim {
    /// Go `Character.WeaponSkill`: the skill above base in one line, the auras' bonus plus each
    /// equipped item's, in slot order.
    pub(crate) fn weapon_skill(&self, unit: UnitId, skill: WeaponSkill) -> f64 {
        let mut total = self.unit(unit).weapon_skill_bonus[skill as usize];
        for item in &self.character(unit).equipment {
            total += item_skill(item, skill);
        }
        total
    }

    /// Go `Character.updateWeaponSkills`: the skill each hand attacks with. Druids fight with
    /// paws in their forms and keep none.
    pub(crate) fn update_weapon_skills(&mut self, unit: UnitId) {
        if self.character(unit).class == "ClassDruid" {
            return;
        }
        for (hand, slot) in [
            (MAIN_HAND, items::slot::MAIN_HAND),
            (OFF_HAND, items::slot::OFF_HAND),
            (RANGED, items::slot::RANGED),
        ] {
            let skill = weapon_skill_of(&self.character(unit).equipment[slot])
                .map_or(0.0, |skill| self.weapon_skill(unit, skill));
            self.unit_mut(unit).weapon_skill[hand] = skill;
        }
    }

    /// Go `Unit.AddWeaponSkill`: raises (or, negative, lowers) one line, and recomputes the hands
    /// of a character once its effects apply (`onWeaponSkillChange`).
    pub(crate) fn add_weapon_skill(&mut self, unit: UnitId, skill: WeaponSkill, amount: f64) {
        self.unit_mut(unit).weapon_skill_bonus[skill as usize] += amount;
        if self.unit(unit).on_weapon_skill_change {
            self.update_weapon_skills(unit);
        }
    }

    /// Go `parentAura.AttachWeaponSkill`: the bonus lasts as long as the aura is active.
    pub(crate) fn attach_weapon_skill(
        &mut self,
        aura: AuraId,
        skill: WeaponSkill,
        amount: f64,
    ) -> AuraId {
        self.apply_on_gain(
            aura,
            Rc::new(move |sim: &mut Sim, aura: AuraId| {
                let unit = sim.aura(aura).unit;
                sim.add_weapon_skill(unit, skill, amount);
            }),
        );
        self.apply_on_expire(
            aura,
            Rc::new(move |sim: &mut Sim, aura: AuraId| {
                let unit = sim.aura(aura).unit;
                sim.add_weapon_skill(unit, skill, -amount);
            }),
        );
        if self.aura(aura).active {
            let unit = self.aura(aura).unit;
            self.add_weapon_skill(unit, skill, amount);
        }
        aura
    }
}

/// The two units of an attack table, as Go's `AttackTable` keeps them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TableUnits {
    pub attacker_level: i32,
    pub defender_type: UnitType,
    pub defender_level: i32,
}

/// Go `AttackTable.weaponSkillGap`: target defense minus the attack's skill, when the skill moves
/// the table at all: skill above base, against an enemy at or above the attacker's level.
fn weapon_skill_gap(units: TableUnits, bonus: f64) -> Option<f64> {
    if bonus == 0.0
        || units.defender_type != UnitType::Enemy
        || units.defender_level < units.attacker_level
    {
        return None;
    }
    // Go fuses the product into the subtraction (FNMSUBD); it is exact either way.
    Some(f64::from(units.defender_level - units.attacker_level).mul_add(5.0, -bonus))
}

/// Whether an attack with `bonus` skill above base reads its own table: Go's `weaponSkillGap`
/// reports a gap.
pub(crate) fn moves_table(units: TableUnits, bonus: f64) -> bool {
    weapon_skill_gap(units, bonus).is_some()
}

/// Go `weaponSkillAvoidance`: below the defense the gap costs 0.1% a point, `per_point_far` once
/// it passes 10; above it, skill takes 0.04% a point off. Go fuses each product into its add.
fn weapon_skill_avoidance(gap: f64, per_point_far: f64) -> f64 {
    if gap > 10.0 {
        gap.mul_add(per_point_far, 0.05)
    } else if gap >= 0.0 {
        gap.mul_add(0.001, 0.05)
    } else {
        gap.mul_add(0.0004, 0.05)
    }
}

/// Go `AttackTable.MissChance`: the chance to miss before hit.
pub(crate) fn miss_chance(table: &AttackTable, units: TableUnits, bonus: f64) -> f64 {
    match weapon_skill_gap(units, bonus) {
        None => table.base_miss_chance,
        Some(gap) => weapon_skill_avoidance(gap, 0.002),
    }
}

/// Go `AttackTable.HitSuppressionFor`: 0.2% of hit a point past a 10 point gap, and none when the
/// table takes none (`Encounter.no_hit_suppression`).
pub(crate) fn hit_suppression(table: &AttackTable, units: TableUnits, bonus: f64) -> f64 {
    match weapon_skill_gap(units, bonus) {
        Some(gap) if table.hit_suppression != 0.0 => (gap - 10.0).max(0.0) * 0.002,
        _ => table.hit_suppression,
    }
}

/// Go `AttackTable.DodgeChance`: the target's chance to dodge, before expertise.
pub(crate) fn dodge_chance(table: &AttackTable, units: TableUnits, bonus: f64) -> f64 {
    match weapon_skill_gap(units, bonus) {
        None => table.base_dodge_chance,
        Some(gap) => weapon_skill_avoidance(gap, 0.001),
    }
}

/// Go `AttackTable.Glance`: the average share of damage a glancing blow keeps, and the spread a
/// roll can take it either side. Go fuses each bound's product into its subtraction (FMSUBD).
pub(crate) fn glance(table: &AttackTable, units: TableUnits, bonus: f64) -> (f64, f64) {
    match weapon_skill_gap(units, bonus) {
        None => (table.glance_multiplier, table.glance_spread),
        Some(gap) => {
            let low = (-0.05f64).mul_add(gap, 1.3).clamp(0.01, 0.91);
            let high = (-0.03f64).mul_add(gap, 1.2).clamp(0.2, 0.99);
            ((low + high) / 2.0, (high - low) / 2.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::attack::new_attack_table;
    use crate::prepare::levels::character_level;

    fn boss(level: i32) -> (AttackTable, TableUnits) {
        let units = TableUnits {
            attacker_level: character_level(),
            defender_type: UnitType::Enemy,
            defender_level: level,
        };
        (
            new_attack_table(character_level(), UnitType::Enemy, level),
            units,
        )
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// Go `TestWeaponSkillAtBaseIsTheTable`: with no skill above base every lookup is the
    /// table's own field, bit for bit.
    #[test]
    fn at_base_skill_every_lookup_is_the_table() {
        for level in character_level() - 2..=character_level() + 3 {
            let (table, units) = boss(level);
            let (multiplier, spread) = glance(&table, units, 0.0);
            assert_eq!(
                miss_chance(&table, units, 0.0).to_bits(),
                table.base_miss_chance.to_bits()
            );
            assert_eq!(
                dodge_chance(&table, units, 0.0).to_bits(),
                table.base_dodge_chance.to_bits()
            );
            assert_eq!(
                hit_suppression(&table, units, 0.0).to_bits(),
                table.hit_suppression.to_bits()
            );
            assert_eq!(multiplier.to_bits(), table.glance_multiplier.to_bits());
            assert_eq!(spread.to_bits(), table.glance_spread.to_bits());
        }
    }

    /// Go `TestWeaponSkillFormulasMeetTheBossTable`: at base skill, a 15 point gap, the
    /// formulas give the boss table.
    #[test]
    fn the_formulas_meet_the_boss_table() {
        let (table, _) = boss(character_level() + 3);
        let low = (1.3f64 - 0.05 * 15.0).clamp(0.01, 0.91);
        let high = (1.2f64 - 0.03 * 15.0).clamp(0.2, 0.99);
        for (name, formula, stored) in [
            (
                "miss",
                weapon_skill_avoidance(15.0, 0.002),
                table.base_miss_chance,
            ),
            (
                "dodge",
                weapon_skill_avoidance(15.0, 0.001),
                table.base_dodge_chance,
            ),
            ("hit suppression", 5.0 * 0.002, table.hit_suppression),
            ("glance", (low + high) / 2.0, table.glance_multiplier),
            ("glance spread", (high - low) / 2.0, table.glance_spread),
        ] {
            assert!(
                (formula - stored).abs() < 1e-12,
                "{name}: formula {formula}, table {stored}"
            );
        }
    }

    /// Go `TestWeaponSkillAgainstABoss`: a level 63 boss (315 defense), per hand.
    #[test]
    fn against_a_boss() {
        let (table, units) = boss(character_level() + 3);
        // Main hand 311 (Aged Core Leather Gloves and Distracting Dagger with a dagger): a 4
        // point gap, 5.4% miss and dodge, no hit suppression, glances keep 91-99%.
        // Off hand 305: a 10 point gap, 6%, still no suppression, glances keep 80-90%.
        // Base skill: the table.
        for (bonus, miss, dodge, suppressed, multiplier, spread) in [
            (11.0, 0.054, 0.054, 0.0, 0.95, 0.04),
            (5.0, 0.06, 0.06, 0.0, 0.85, 0.05),
            (0.0, 0.08, 0.065, 0.01, 0.65, 0.10),
        ] {
            let (got_multiplier, got_spread) = glance(&table, units, bonus);
            assert!(near(miss_chance(&table, units, bonus), miss), "{bonus}");
            assert!(near(dodge_chance(&table, units, bonus), dodge), "{bonus}");
            assert!(
                near(hit_suppression(&table, units, bonus), suppressed),
                "{bonus}"
            );
            assert!(near(got_multiplier, multiplier), "{bonus}");
            assert!(near(got_spread, spread), "{bonus}");
            // Glancing chance and crit suppression stay on the base skill.
            assert_eq!(table.base_glance_chance, 0.40);
            assert_eq!(table.melee_crit_suppression, 0.048);
        }
        // A 12 point gap (303) is past 10: 0.2% a point and 0.4% of hit ignored.
        assert!(near(miss_chance(&table, units, 3.0), 0.074));
        assert!(near(hit_suppression(&table, units, 3.0), 0.004));
        assert!(near(dodge_chance(&table, units, 3.0), 0.062));
        // Skill past the target's defense (320 against 315) takes 0.04% a point off.
        assert!(near(miss_chance(&table, units, 20.0), 0.048));
        assert!(near(dodge_chance(&table, units, 20.0), 0.048));
        // Encounter.no_hit_suppression stays off whatever the skill.
        let mut suppressed_off = table.clone();
        suppressed_off.hit_suppression = 0.0;
        assert_eq!(hit_suppression(&suppressed_off, units, 3.0), 0.0);
    }

    /// Go `TestWeaponSkillOnlyAgainstEnemiesAtOrAboveTheAttacker`.
    #[test]
    fn only_against_enemies_at_or_above_the_attacker() {
        let (table, units) = boss(character_level() - 2);
        assert_eq!(miss_chance(&table, units, 11.0), table.base_miss_chance);
        // An enemy's table against a player.
        let enemy = TableUnits {
            attacker_level: character_level() + 3,
            defender_type: UnitType::Player,
            defender_level: character_level(),
        };
        let enemy_table =
            new_attack_table(character_level() + 3, UnitType::Player, character_level());
        assert_eq!(
            dodge_chance(&enemy_table, enemy, 11.0),
            enemy_table.base_dodge_chance
        );
    }

    /// Go `TestWeaponSkillOf`.
    #[test]
    fn the_skill_line_of_a_weapon() {
        use WeaponSkill::*;
        let item = |weapon: &str, hand: &str, ranged: &str| Item {
            id: 1,
            weapon_type: weapon.to_string(),
            hand_type: hand.to_string(),
            ranged_weapon_type: ranged.to_string(),
            ..Item::default()
        };
        let none = "RangedWeaponTypeUnknown";
        for (item, want) in [
            (
                item("WeaponTypeDagger", "HandTypeOneHand", none),
                Some(Daggers),
            ),
            (
                item("WeaponTypeSword", "HandTypeMainHand", none),
                Some(Swords),
            ),
            (
                item("WeaponTypeSword", "HandTypeTwoHand", none),
                Some(TwoHandedSwords),
            ),
            (item("WeaponTypeAxe", "HandTypeOffHand", none), Some(Axes)),
            (
                item("WeaponTypeFist", "HandTypeOneHand", none),
                Some(Unarmed),
            ),
            (
                item("WeaponTypePolearm", "HandTypeTwoHand", none),
                Some(Polearms),
            ),
            (
                item("WeaponTypeStaff", "HandTypeTwoHand", none),
                Some(Staves),
            ),
            (
                item(
                    "WeaponTypeUnknown",
                    "HandTypeUnknown",
                    "RangedWeaponTypeGun",
                ),
                Some(Guns),
            ),
            (
                item(
                    "WeaponTypeUnknown",
                    "HandTypeUnknown",
                    "RangedWeaponTypeWand",
                ),
                None,
            ),
            (item("WeaponTypeShield", "HandTypeOffHand", none), None),
            (item("WeaponTypeOffHand", "HandTypeOffHand", none), None),
            (Item::default(), None),
        ] {
            assert_eq!(weapon_skill_of(&item), want, "{item:?}");
        }
    }

    /// The hands' skill of a warrior wearing `items` by slot, with Alcor's Sunrazor, a dagger,
    /// in the main hand.
    fn hands_wearing(items: &[(usize, i32)]) -> [f64; HANDS] {
        let mut listing = vec![(0, 0); items::slot::MAIN_HAND + 1];
        listing[items::slot::MAIN_HAND] = (14555, 0);
        for (slot, id) in items {
            listing[*slot] = (*id, 0);
        }
        let env = crate::prepare::item_test_support::environment(&listing);
        env.sim.unit(env.player).weapon_skill
    }

    /// Go patch 135: the Defias Leather 5 piece raises daggers by 1 while its aura is up; four
    /// pieces leave the skill at base. Preparation refuses the set's 3 piece humanoid bonus, so
    /// no prepared build shows it.
    #[test]
    fn the_defias_leather_five_piece_raises_daggers() {
        use items::slot::*;
        let defias = [
            (CHEST, 10399),
            (LEGS, 10400),
            (HANDS, 10401),
            (FEET, 10402),
            (WAIST, 10403),
        ];
        assert_eq!(hands_wearing(&defias), [1.0, 0.0, 0.0]);
        assert_eq!(hands_wearing(&defias[..4]), [0.0, 0.0, 0.0]);
        assert_eq!(
            hands_wearing(&[(HANDS, 18823)]),
            [5.0, 0.0, 0.0],
            "Aged Core Leather Gloves"
        );
    }

    /// Go `TestItemWeaponSkills`: the skill the imported database gives each item.
    #[test]
    fn the_database_gives_items_their_skill() {
        use WeaponSkill::*;
        for (id, want) in [
            (21126, vec![(Daggers, 3.0)]),
            (7951, vec![(Daggers, 4.0)]),
            (12775, vec![(TwoHandedAxes, 2.0)]),
            (4548, vec![(TwoHandedMaces, 1.0)]),
            (276102, vec![(TwoHandedSwords, 1.0)]),
            (274929, vec![(TwoHandedAxes, 1.0)]),
            (275669, vec![(Unarmed, 1.0)]),
            (13056, vec![(Polearms, 1.0)]),
            (20005, vec![(Unarmed, 4.0)]),
            (18823, vec![(Daggers, 5.0)]),
            (18392, vec![(Daggers, 6.0)]),
            (18505, vec![(Daggers, 5.0)]),
            (18822, vec![(TwoHandedSwords, 8.0)]),
            (14551, vec![(Axes, 7.0), (Daggers, 7.0), (Swords, 7.0)]),
            (13255, vec![]),
            (22207, vec![]),
            (2907, vec![]),
        ] {
            let item = items::database_item(id).expect("a database item");
            for skill in [
                Axes,
                Swords,
                Maces,
                Daggers,
                Unarmed,
                TwoHandedAxes,
                TwoHandedSwords,
                TwoHandedMaces,
                Polearms,
                Staves,
                Bows,
                Guns,
                Crossbows,
                Thrown,
            ] {
                let expected = want
                    .iter()
                    .find(|(line, _)| *line == skill)
                    .map_or(0.0, |(_, value)| *value);
                assert_eq!(
                    item_skill(&item, skill),
                    expected,
                    "{id} {} {skill:?}",
                    item.name
                );
            }
        }
    }
}
