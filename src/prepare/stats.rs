//! Go `sim/core/stats`: the stat array, pseudo stats and stat dependencies.
//!
//! Every operation keeps Go's order of float operations, so the stats a reset leaves are
//! bit for bit Go's.

use std::ops::{Index, IndexMut};

/// Go's `stats.Stat`, in Go's order. The first 41 match `proto.Stat`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub(crate) enum Stat {
    Strength,
    Agility,
    Stamina,
    Intellect,
    HealingPower,
    SpellDamage,
    ArcaneDamage,
    FireDamage,
    FrostDamage,
    HolyDamage,
    NatureDamage,
    ShadowDamage,
    SpellHitRating,
    SpellCritRating,
    SpellHasteRating,
    SpellPiercing,
    Spirit,
    AttackPower,
    RangedAttackPower,
    FeralAttackPower,
    MeleeHitRating,
    MeleeCritRating,
    MeleeHasteRating,
    ArmorPenetration,
    ExpertiseRating,
    DefenseRating,
    BlockRating,
    BlockValue,
    DodgeRating,
    ParryRating,
    Armor,
    BonusArmor,
    Health,
    Mana,
    MP5,
    ArcaneResistance,
    FireResistance,
    FrostResistance,
    NatureResistance,
    ShadowResistance,
    PhysicalDamage,
    PhysicalHitPercent,
    SpellHitPercent,
    PhysicalCritPercent,
    SpellCritPercent,
    BlockPercent,
    RangedHitPercent,
    RangedCritPercent,
    DodgePercent,
    ParryPercent,
    ReducedCritTakenPercent,
    ExpertisePercent,
}

pub(crate) const SIM_STATS_LEN: usize = 52;
/// `len(proto.Stat_name)`: the stats a proto stat array holds.
pub(crate) const PROTO_STATS_LEN: usize = 41;

impl Stat {
    pub(crate) const ALL: [Stat; SIM_STATS_LEN] = {
        let mut all = [Stat::Strength; SIM_STATS_LEN];
        let mut i = 0;
        while i < SIM_STATS_LEN {
            all[i] = Stat::from_index(i);
            i += 1;
        }
        all
    };

    pub(crate) const fn from_index(index: usize) -> Stat {
        assert!(index < SIM_STATS_LEN);
        // SAFETY: Stat is repr(u8) with SIM_STATS_LEN contiguous variants from zero.
        unsafe { std::mem::transmute::<u8, Stat>(index as u8) }
    }

    /// Go's `Stat.StatName`.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Stat::Strength => "Strength",
            Stat::Agility => "Agility",
            Stat::Stamina => "Stamina",
            Stat::Intellect => "Intellect",
            Stat::HealingPower => "HealingPower",
            Stat::SpellDamage => "SpellDamage",
            Stat::ArcaneDamage => "ArcaneDamage",
            Stat::FireDamage => "FireDamage",
            Stat::FrostDamage => "FrostDamage",
            Stat::HolyDamage => "HolyDamage",
            Stat::NatureDamage => "NatureDamage",
            Stat::ShadowDamage => "ShadowDamage",
            Stat::SpellHitRating => "SpellHitRating",
            Stat::SpellCritRating => "SpellCritRating",
            Stat::SpellHasteRating => "SpellHasteRating",
            Stat::SpellPiercing => "SpellPiercing",
            Stat::Spirit => "Spirit",
            Stat::AttackPower => "AttackPower",
            Stat::RangedAttackPower => "RangedAttackPower",
            Stat::FeralAttackPower => "FeralAttackPower",
            Stat::MeleeHitRating => "MeleeHitRating",
            Stat::MeleeCritRating => "MeleeCritRating",
            Stat::MeleeHasteRating => "MeleeHasteRating",
            Stat::ArmorPenetration => "ArmorPenetration",
            Stat::ExpertiseRating => "ExpertiseRating",
            Stat::DefenseRating => "DefenseRating",
            Stat::BlockRating => "BlockRating",
            Stat::BlockValue => "BlockValue",
            Stat::DodgeRating => "DodgeRating",
            Stat::ParryRating => "ParryRating",
            Stat::Armor => "Armor",
            Stat::BonusArmor => "BonusArmor",
            Stat::Health => "Health",
            Stat::Mana => "Mana",
            Stat::MP5 => "MP5",
            Stat::ArcaneResistance => "ArcaneResistance",
            Stat::FireResistance => "FireResistance",
            Stat::FrostResistance => "FrostResistance",
            Stat::NatureResistance => "NatureResistance",
            Stat::ShadowResistance => "ShadowResistance",
            Stat::PhysicalDamage => "PhysicalDamage",
            Stat::PhysicalHitPercent => "PhysicalHitPercent",
            Stat::SpellHitPercent => "SpellHitPercent",
            Stat::PhysicalCritPercent => "PhysicalCritPercent",
            Stat::SpellCritPercent => "SpellCritPercent",
            Stat::BlockPercent => "BlockPercent",
            Stat::RangedHitPercent => "RangedHitPercent",
            Stat::RangedCritPercent => "RangedCritPercent",
            Stat::DodgePercent => "DodgePercent",
            Stat::ParryPercent => "ParryPercent",
            Stat::ReducedCritTakenPercent => "ReducedCritTakenPercent",
            Stat::ExpertisePercent => "ExpertisePercent",
        }
    }

    pub(crate) fn by_name(name: &str) -> Option<Stat> {
        Stat::ALL.into_iter().find(|stat| stat.name() == name)
    }
}

/// Go's `stats.Stats`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stats(pub [f64; SIM_STATS_LEN]);

impl Default for Stats {
    fn default() -> Stats {
        Stats([0.0; SIM_STATS_LEN])
    }
}

impl Index<Stat> for Stats {
    type Output = f64;
    fn index(&self, stat: Stat) -> &f64 {
        &self.0[stat as usize]
    }
}

impl IndexMut<Stat> for Stats {
    fn index_mut(&mut self, stat: Stat) -> &mut f64 {
        &mut self.0[stat as usize]
    }
}

/// The attributes the game stores floored: `flooredGameStats`.
const FLOORED_GAME_STATS: [Stat; 5] = [
    Stat::Strength,
    Stat::Agility,
    Stat::Stamina,
    Stat::Intellect,
    Stat::Spirit,
];

fn is_floored_game_stat(stat: Stat) -> bool {
    FLOORED_GAME_STATS.contains(&stat)
}

impl Stats {
    pub(crate) fn from_pairs(pairs: &[(Stat, f64)]) -> Stats {
        let mut out = Stats::default();
        for (stat, value) in pairs {
            out[*stat] = *value;
        }
        out
    }

    /// Go `FromProtoArray`: the shared indices of a proto stat array.
    pub(crate) fn from_proto_array(values: &[f64]) -> Stats {
        let mut out = Stats::default();
        for (index, value) in values.iter().take(SIM_STATS_LEN).enumerate() {
            out.0[index] = *value;
        }
        out
    }

    pub(crate) fn add(mut self, other: &Stats) -> Stats {
        self.add_inplace(other);
        self
    }

    pub(crate) fn add_inplace(&mut self, other: &Stats) {
        for (value, other) in self.0.iter_mut().zip(other.0) {
            *value += other;
        }
    }

    pub(crate) fn subtract(mut self, other: &Stats) -> Stats {
        for (value, other) in self.0.iter_mut().zip(other.0) {
            *value -= other;
        }
        self
    }

    pub(crate) fn invert(mut self) -> Stats {
        for value in &mut self.0 {
            *value = -*value;
        }
        self
    }

    pub(crate) fn floor(mut self) -> Stats {
        for value in &mut self.0 {
            *value = value.floor();
        }
        self
    }

    pub(crate) fn floor_game_stats(mut self) -> Stats {
        for stat in FLOORED_GAME_STATS {
            self[stat] = self[stat].floor();
        }
        self
    }

    pub(crate) fn multiply(mut self, multiplier: f64) -> Stats {
        for value in &mut self.0 {
            *value *= multiplier;
        }
        self
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0.iter().all(|value| *value == 0.0)
    }
}

/// Go's `SchoolIndex` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SchoolIndex {
    None = 0,
    Physical,
    Arcane,
    Fire,
    Frost,
    Holy,
    Nature,
    Shadow,
}

pub(crate) const SCHOOL_LEN: usize = 8;

pub(crate) fn school_ones() -> [f64; SCHOOL_LEN] {
    [1.0; SCHOOL_LEN]
}

/// Go's `stats.PseudoStats`. Durations are nanoseconds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PseudoStats {
    pub spell_cost_percent_modifier: i32,
    pub cast_speed_multiplier: f64,
    pub melee_speed_multiplier: f64,
    pub ranged_speed_multiplier: f64,
    pub ranged_haste_multiplier: f64,
    pub attack_speed_multiplier: f64,
    pub five_second_rule_refresh_time: i64,
    pub spirit_regen_rate_casting: f64,
    pub force_full_spirit_regen: bool,
    pub spirit_regen_multiplier: f64,
    pub in_front_of_target: bool,
    pub bonus_mh_dps: f64,
    pub bonus_oh_dps: f64,
    pub bonus_ranged_dps: f64,
    pub disable_dw_miss_penalty: bool,
    pub increased_miss_chance: f64,
    pub dodge_reduction: f64,
    pub threat_multiplier: f64,
    pub damage_dealt_multiplier: f64,
    pub school_damage_dealt_multiplier: [f64; SCHOOL_LEN],
    pub dot_damage_multiplier_additive: f64,
    pub healing_dealt_multiplier: f64,
    pub periodic_healing_dealt_multiplier: f64,
    pub crit_damage_multiplier: f64,
    pub bonus_ranged_attack_power: f64,
    pub bonus_attack_power: f64,
    pub block_value_multiplier: f64,
    pub damage_spread: f64,
    pub incapacitated: bool,
    pub stunned: bool,
    pub fear_immune: bool,
    pub stun_immune: bool,
    pub fear_duration_multiplier: f64,
    pub stun_duration_multiplier: f64,
    pub can_block: bool,
    pub can_parry: bool,
    pub can_crush: bool,
    pub parry_haste: bool,
    pub base_dodge_chance: f64,
    pub base_parry_chance: f64,
    pub base_block_chance: f64,
    pub base_reduced_crit_taken_percent: f64,
    pub reduced_crit_taken_percent: f64,
    pub bonus_healing_taken: f64,
    pub bonus_spell_crit_percent_taken: f64,
    pub bonus_physical_damage_taken: f64,
    pub bonus_spell_damage_taken: f64,
    pub damage_taken_multiplier: f64,
    pub school_damage_taken_multiplier: [f64; SCHOOL_LEN],
    pub school_bonus_spell_damage: [f64; SCHOOL_LEN],
    pub school_bonus_hit_chance: [f64; SCHOOL_LEN],
    pub disease_damage_taken_multiplier: f64,
    pub periodic_physical_damage_taken_multiplier: f64,
    pub armor_multiplier: f64,
    pub reduced_physical_hit_taken_chance: f64,
    pub reduced_arcane_hit_taken_chance: f64,
    pub reduced_fire_hit_taken_chance: f64,
    pub reduced_frost_hit_taken_chance: f64,
    pub reduced_nature_hit_taken_chance: f64,
    pub reduced_shadow_hit_taken_chance: f64,
    pub healing_taken_multiplier: f64,
    pub external_healing_taken_multiplier: f64,
    pub movement_speed_multiplier: f64,
    pub self_healing_multiplier: f64,
    pub pushback_chance: f64,
}

impl PseudoStats {
    /// Go `NewPseudoStats`.
    pub(crate) fn new() -> PseudoStats {
        PseudoStats {
            spell_cost_percent_modifier: 100,
            cast_speed_multiplier: 1.0,
            melee_speed_multiplier: 1.0,
            ranged_speed_multiplier: 1.0,
            ranged_haste_multiplier: 1.0,
            attack_speed_multiplier: 1.0,
            five_second_rule_refresh_time: 0,
            spirit_regen_rate_casting: 0.0,
            force_full_spirit_regen: false,
            spirit_regen_multiplier: 1.0,
            in_front_of_target: false,
            bonus_mh_dps: 0.0,
            bonus_oh_dps: 0.0,
            bonus_ranged_dps: 0.0,
            disable_dw_miss_penalty: false,
            increased_miss_chance: 0.0,
            dodge_reduction: 0.0,
            threat_multiplier: 1.0,
            damage_dealt_multiplier: 1.0,
            school_damage_dealt_multiplier: school_ones(),
            dot_damage_multiplier_additive: 1.0,
            healing_dealt_multiplier: 1.0,
            periodic_healing_dealt_multiplier: 1.0,
            crit_damage_multiplier: 1.0,
            bonus_ranged_attack_power: 0.0,
            bonus_attack_power: 0.0,
            block_value_multiplier: 1.0,
            damage_spread: 0.3333,
            incapacitated: false,
            stunned: false,
            fear_immune: false,
            stun_immune: false,
            fear_duration_multiplier: 1.0,
            stun_duration_multiplier: 1.0,
            can_block: false,
            can_parry: false,
            can_crush: false,
            parry_haste: false,
            base_dodge_chance: 0.0,
            base_parry_chance: 0.0,
            base_block_chance: 0.0,
            base_reduced_crit_taken_percent: 0.0,
            reduced_crit_taken_percent: 0.0,
            bonus_healing_taken: 0.0,
            bonus_spell_crit_percent_taken: 0.0,
            bonus_physical_damage_taken: 0.0,
            bonus_spell_damage_taken: 0.0,
            damage_taken_multiplier: 1.0,
            school_damage_taken_multiplier: school_ones(),
            school_bonus_spell_damage: [0.0; SCHOOL_LEN],
            school_bonus_hit_chance: [0.0; SCHOOL_LEN],
            disease_damage_taken_multiplier: 1.0,
            periodic_physical_damage_taken_multiplier: 1.0,
            armor_multiplier: 1.0,
            reduced_physical_hit_taken_chance: 0.0,
            reduced_arcane_hit_taken_chance: 0.0,
            reduced_fire_hit_taken_chance: 0.0,
            reduced_frost_hit_taken_chance: 0.0,
            reduced_nature_hit_taken_chance: 0.0,
            reduced_shadow_hit_taken_chance: 0.0,
            healing_taken_multiplier: 1.0,
            external_healing_taken_multiplier: 1.0,
            movement_speed_multiplier: 1.0,
            self_healing_multiplier: 0.0,
            pushback_chance: 1.0,
        }
    }
}

/// The order in which evaluating dependencies is safe: `safeDepsOrder`.
const SAFE_DEPS_ORDER: [Stat; 38] = [
    Stat::Strength,
    Stat::Agility,
    Stat::Stamina,
    Stat::Intellect,
    Stat::Spirit,
    Stat::BonusArmor,
    Stat::Armor,
    Stat::FeralAttackPower,
    Stat::AttackPower,
    Stat::RangedAttackPower,
    Stat::SpellDamage,
    Stat::HealingPower,
    Stat::Health,
    Stat::Mana,
    Stat::MP5,
    Stat::MeleeHasteRating,
    Stat::MeleeCritRating,
    Stat::MeleeHitRating,
    Stat::SpellHitRating,
    Stat::SpellCritRating,
    Stat::SpellHasteRating,
    Stat::DefenseRating,
    Stat::BlockRating,
    Stat::BlockPercent,
    Stat::DodgeRating,
    Stat::ParryRating,
    Stat::ExpertiseRating,
    Stat::BlockValue,
    Stat::ArmorPenetration,
    Stat::SpellPiercing,
    Stat::SpellCritPercent,
    Stat::PhysicalCritPercent,
    Stat::SpellHitPercent,
    Stat::PhysicalHitPercent,
    Stat::DodgePercent,
    Stat::ParryPercent,
    Stat::ReducedCritTakenPercent,
    Stat::ExpertisePercent,
];

fn validate_dep(src: Stat, dst: Stat) {
    let position = |stat| SAFE_DEPS_ORDER.iter().position(|s| *s == stat);
    match (position(src), position(dst)) {
        (Some(a), Some(b)) if a <= b => {}
        _ => panic!("Invalid stat dependency: {} --> {}", src.name(), dst.name()),
    }
}

/// A handle to a dynamic dependency: `*stats.StatDependency`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DepId(usize);

#[derive(Clone, Debug)]
struct StatDependency {
    dynamic: bool,
    enabled: bool,
    src: Stat,
    dst: Stat,
    amount: f64,
    step: f64,
    /// The handle, kept through sorting so a dynamic dependency stays addressable.
    id: Option<DepId>,
}

/// Go's `StatDependencyManager`.
#[derive(Clone, Debug, Default)]
pub(crate) struct StatDependencyManager {
    deps: Vec<StatDependency>,
    finalized: bool,
    next_id: usize,
}

impl StatDependencyManager {
    fn push(&mut self, dynamic: bool, src: Stat, dst: Stat, amount: f64) -> DepId {
        assert!(!self.finalized, "StatDependencyManager already finalized!");
        let id = DepId(self.next_id);
        self.next_id += 1;
        self.deps.push(StatDependency {
            dynamic,
            enabled: !dynamic,
            src,
            dst,
            amount,
            step: 0.0,
            id: Some(id),
        });
        id
    }

    pub(crate) fn add_stat_dependency(&mut self, src: Stat, dst: Stat, amount: f64) {
        validate_dep(src, dst);
        assert!(
            src != dst,
            "For same-stat dependencies, use MultiplyStat instead!"
        );
        self.push(false, src, dst, amount);
    }

    pub(crate) fn add_floored_stat_dependency(
        &mut self,
        src: Stat,
        dst: Stat,
        step: f64,
        amount: f64,
    ) {
        self.add_stat_dependency(src, dst, amount);
        self.deps.last_mut().expect("just added").step = step;
    }

    pub(crate) fn multiply_stat(&mut self, stat: Stat, amount: f64) {
        validate_dep(stat, stat);
        self.push(false, stat, stat, amount);
    }

    pub(crate) fn new_dynamic_stat_dependency(
        &mut self,
        src: Stat,
        dst: Stat,
        amount: f64,
    ) -> DepId {
        validate_dep(src, dst);
        assert!(
            src != dst,
            "For same-stat dependencies, use NewDynamicMultiplyStat instead!"
        );
        self.push(true, src, dst, amount)
    }

    pub(crate) fn new_dynamic_multiply_stat(&mut self, stat: Stat, amount: f64) -> DepId {
        validate_dep(stat, stat);
        self.push(true, stat, stat, amount)
    }

    fn sort_deps(&mut self) {
        let mut deps = Vec::with_capacity(self.deps.len());
        for (i, src) in SAFE_DEPS_ORDER.iter().enumerate() {
            for dst in &SAFE_DEPS_ORDER[i..] {
                let start = if src == dst { 1.0 } else { 0.0 };
                let mut amount = start;
                for dep in &self.deps {
                    if dep.src != *src || dep.dst != *dst {
                        continue;
                    }
                    if dep.dynamic || dep.step != 0.0 {
                        deps.push(dep.clone());
                    } else if src == dst {
                        amount *= dep.amount;
                    } else {
                        amount += dep.amount;
                    }
                }
                if amount != start {
                    deps.push(StatDependency {
                        dynamic: false,
                        enabled: true,
                        src: *src,
                        dst: *dst,
                        amount,
                        step: 0.0,
                        id: None,
                    });
                }
            }
        }
        self.deps = deps;
    }

    pub(crate) fn finalize_stat_deps(&mut self) {
        assert!(!self.finalized, "StatDependencyManager already finalized!");
        self.sort_deps();
        self.finalized = true;
    }

    pub(crate) fn reset_stat_deps(&mut self) {
        for dep in &mut self.deps {
            if dep.dynamic {
                dep.enabled = false;
            }
        }
    }

    pub(crate) fn is_finalized(&self) -> bool {
        self.finalized
    }

    pub(crate) fn apply_stat_dependencies(&self, mut s: Stats) -> Stats {
        for dep in &self.deps {
            if !dep.enabled {
                continue;
            }
            // Go's arm64 build fuses each sum into one multiply-add (deps.go 277, 282, 284).
            if dep.src == dep.dst {
                s[dep.dst] *= dep.amount;
            } else if dep.step != 0.0 {
                let steps = (s[dep.src] / dep.step).floor() * dep.step;
                s[dep.dst] = steps.mul_add(dep.amount, s[dep.dst]);
            } else if is_floored_game_stat(dep.src) {
                s[dep.dst] = s[dep.src].floor().mul_add(dep.amount, s[dep.dst]);
            } else {
                s[dep.dst] = s[dep.src].mul_add(dep.amount, s[dep.dst]);
            }
        }
        s
    }

    pub(crate) fn sort_and_apply_stat_dependencies(&mut self, s: Stats) -> Stats {
        self.sort_deps();
        self.apply_stat_dependencies(s)
    }

    /// The enabled dependencies as `(src, dst, amount, step)`, in their applied order.
    pub(crate) fn enabled_dependencies(&self) -> Vec<(Stat, Stat, f64, f64)> {
        self.deps
            .iter()
            .filter(|dep| dep.enabled)
            .map(|dep| (dep.src, dep.dst, dep.amount, dep.step))
            .collect()
    }

    pub(crate) fn stat_dependency_coeff(&self, src: Stat, dst: Stat) -> f64 {
        self.deps
            .iter()
            .find(|dep| dep.enabled && dep.src == src && dep.dst == dst)
            .map_or(0.0, |dep| dep.amount)
    }

    fn dep_mut(&mut self, id: DepId) -> &mut StatDependency {
        self.deps
            .iter_mut()
            .find(|dep| dep.id == Some(id))
            .expect("a registered dynamic dependency")
    }

    /// Returns whether the state changed.
    pub(crate) fn enable_dynamic_stat_dep(&mut self, id: DepId) -> bool {
        let dep = self.dep_mut(id);
        let changed = !dep.enabled;
        dep.enabled = true;
        changed
    }

    /// Returns whether the state changed.
    pub(crate) fn disable_dynamic_stat_dep(&mut self, id: DepId) -> bool {
        let dep = self.dep_mut(id);
        let changed = dep.enabled;
        dep.enabled = false;
        changed
    }

    pub(crate) fn update_value(&mut self, id: DepId, amount: f64) {
        self.dep_mut(id).amount = amount;
    }

    pub(crate) fn amount(&mut self, id: DepId) -> f64 {
        self.dep_mut(id).amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_go() {
        assert_eq!(Stat::ALL.len(), SIM_STATS_LEN);
        assert_eq!(Stat::PhysicalDamage as usize, PROTO_STATS_LEN - 1);
        assert_eq!(Stat::by_name("MP5"), Some(Stat::MP5));
        assert_eq!(Stat::from_index(51), Stat::ExpertisePercent);
    }

    #[test]
    fn static_dependencies_combine_and_dynamic_ones_stay_apart() {
        let mut sdm = StatDependencyManager::default();
        sdm.add_stat_dependency(Stat::Intellect, Stat::Mana, 15.0);
        sdm.multiply_stat(Stat::Intellect, 1.1);
        sdm.multiply_stat(Stat::Intellect, 1.03);
        let dynamic = sdm.new_dynamic_multiply_stat(Stat::Intellect, 2.0);
        sdm.finalize_stat_deps();
        let mut s = Stats::default();
        s[Stat::Intellect] = 100.5;
        let out = sdm.apply_stat_dependencies(s);
        assert_eq!(out[Stat::Intellect], 100.5 * (1.1 * 1.03));
        assert_eq!(
            out[Stat::Mana],
            (100.5f64 * (1.1 * 1.03)).floor().mul_add(15.0, 0.0)
        );
        assert!(sdm.enable_dynamic_stat_dep(dynamic));
        assert_eq!(
            sdm.apply_stat_dependencies(s)[Stat::Intellect],
            100.5 * 2.0 * (1.1 * 1.03)
        );
    }
}
