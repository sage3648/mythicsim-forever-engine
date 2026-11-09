//! Go sim/warlock/pets.go: the four demons, registered at construction with only the summoned
//! one enabled at reset, and the abilities the Imp and the Succubus carry. The demons' rotation
//! only runs in a fight.

use std::rc::Rc;

use crate::prepare::attack::{AutoAttackOptions, Weapon};
use crate::prepare::levels::{character_level, creature_dps_share, spellbook_lists, MAX_LEVEL};
use crate::prepare::pet::PetConfig;
use crate::prepare::sim::{Cooldown, Rules, Sim, SpellId, UnitId, MILLISECOND, SECOND};
use crate::prepare::spell::{
    school, Cast, CastConfig, CostOptions, DefenseType, ProcMask, SpellConfig, GCD_DEFAULT, GCD_MIN,
};
use crate::prepare::spelldata::Effect;
use crate::prepare::stats::{Stat, Stats};

use super::masks;
use super::spells::spell_action;
use super::Warlock;

/// Go `petSpellDamageFraction` and `petAttackPowerFraction`: what a Forever demon gets from its
/// warlock through the client's hidden Warlock Pet Scaling aura (416189).
const PET_SPELL_DAMAGE_FRACTION: f64 = 0.10;
const PET_ATTACK_POWER_FRACTION: f64 = 0.17;

/// A demon's melee hit for each point of its warlock's spell hit: the 8% melee hit cap over the
/// 17% spell hit cap. Forever's developers on their tracker (22 October 2026): "Warlocks will give
/// spell hit equal to their spell hit, and melee hit equal to 8/17ths of their spell hit."
pub(crate) const DEMON_MELEE_HIT_PER_SPELL_HIT: f64 = 8.0 / 17.0;

/// Go `proto.WarlockOptions_Summon` of the four demons the warlock registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Summon {
    Imp,
    Voidwalker,
    Succubus,
    Felhunter,
}

impl Summon {
    /// `proto.WarlockOptions_Summon_name`.
    fn name(self) -> &'static str {
        match self {
            Summon::Imp => "Imp",
            Summon::Voidwalker => "Voidwalker",
            Summon::Succubus => "Succubus",
            Summon::Felhunter => "Felhunter",
        }
    }

    /// Go `petBaseStats`: level 60 pet stats, from our Forever sim.
    fn base_stats(self) -> Stats {
        match self {
            Summon::Imp => Stats::from_pairs(&[
                (Stat::Strength, 122.0),
                (Stat::Agility, 35.0),
                (Stat::Stamina, 86.0),
                (Stat::Intellect, 264.0),
                (Stat::Spirit, 260.0),
                (Stat::Mana, 576.0),
            ]),
            Summon::Voidwalker | Summon::Felhunter => Stats::from_pairs(&[
                (Stat::Strength, 129.0),
                (Stat::Agility, 85.0),
                (Stat::Stamina, 234.0),
                (Stat::Intellect, 70.0),
                (Stat::Spirit, 150.0),
                (Stat::Mana, 1066.0),
            ]),
            // The Succubus' Lash of Pain crits about 6% whatever its master's crit: demons carry
            // their own base crit (community #790).
            Summon::Succubus => Stats::from_pairs(&[
                (Stat::Strength, 129.0),
                (Stat::Agility, 85.0),
                (Stat::Stamina, 234.0),
                (Stat::Intellect, 70.0),
                (Stat::Spirit, 150.0),
                (Stat::Mana, 1066.0),
                (Stat::SpellCritPercent, 6.2),
            ]),
        }
    }

    /// Go `petAutoAttacks`: the demon's melee damage range; the Imp has none.
    fn auto_attack(self) -> Option<(f64, f64)> {
        match self {
            Summon::Imp => None,
            Summon::Voidwalker => Some((31.0, 46.0)),
            Summon::Succubus => Some((95.0, 131.0)),
            Summon::Felhunter => Some((70.0, 97.0)),
        }
    }

    /// The demon's base stats at the character's level: the level 60 Strength and Agility, which
    /// make its own attack power and melee crit, times the level's [`demon_level_share`]. Its mana,
    /// Intellect, Spirit, Stamina and base crit stay the level 60 ones: levelling Imps on public
    /// logs cast Firebolt every 2.44 sec in runs of 13 to 19 at levels 12 to 18, where a pool
    /// scaled with the level would run dry after 5 (the demon regenerates no mana while casting).
    fn base_stats_at(self, share: Option<f64>) -> Stats {
        let mut stats = self.base_stats();
        if let Some(share) = share {
            stats[Stat::Strength] *= share;
            stats[Stat::Agility] *= share;
        }
        stats
    }

    /// The demon's melee damage range at the character's level, the level 60 range times the
    /// level's [`demon_level_share`].
    fn auto_attack_at(self, share: Option<f64>) -> Option<(f64, f64)> {
        let (min, max) = self.auto_attack()?;
        Some(share.map_or((min, max), |share| (min * share, max * share)))
    }
}

/// How a summoned demon at the character's level compares with a level 60 one under Forever's
/// rules, or None where it is the level 60 demon (at 60, and under the reference's rules).
///
/// A demon is a creature of its warlock's level, and Forever scales a creature with the
/// client's `ExpectedStat` rows: its swing, the weapon and the attack power from its own
/// Strength, follows the creature auto attack dps of its level (README, Rust-only mechanics),
/// and so does its melee crit from Agility. A level 25 Succubus swings at 0.345 of a level 60
/// one.
pub(crate) fn demon_level_share(rules: Rules) -> Option<f64> {
    let level = character_level();
    (rules == Rules::Forever && level != MAX_LEVEL).then(|| creature_dps_share(level))
}

/// A rank of a demon's damage spell, from the client's rows (wowf `SpellEffect`, `SpellLevels`
/// and `SpellPower` from wago.tools on 9 November 2026). The reference has only the top rank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DemonRank {
    pub id: i32,
    /// The level the rank is learned at and the level its amount stops growing at.
    pub spell_level: i16,
    pub max_level: i16,
    pub base_points: f64,
    pub points_per_level: f64,
    pub variance: f64,
    pub coefficient: f64,
    pub cost: i32,
}

impl DemonRank {
    /// The rank's damage effect, which scales to the caster's level as a spell row does.
    pub(crate) fn effect(&self) -> Effect {
        Effect {
            base_points: self.base_points,
            ppl: self.points_per_level,
            variance: self.variance,
            spell_level: self.spell_level,
            max_level: self.max_level,
            ..Effect::default()
        }
    }
}

const fn rank(
    id: i32,
    (spell_level, max_level): (i16, i16),
    (base_points, points_per_level, variance): (f64, f64, f64),
    coefficient: f64,
    cost: i32,
) -> DemonRank {
    DemonRank {
        id,
        spell_level,
        max_level,
        base_points,
        points_per_level,
        variance,
        coefficient,
        cost,
    }
}

/// The Imp's Firebolt, ranks 1 to 7. Rank 7 (11763) is the reference's `impFireboltEffect`.
/// Each rank is a roll a variance wide around its base plus the points per level from its own
/// level up to its cap, with its own spell power coefficient (0.164 at rank 1, 0.571 from rank
/// 4) and mana cost.
pub(crate) const FIREBOLT_RANKS: [DemonRank; 7] = [
    rank(3110, (1, 5), (4.0, 0.1, 0.285_714_298_49), 0.164, 10),
    rank(7799, (8, 13), (7.0, 0.2, 0.153_846_159_58), 0.314, 20),
    rank(7800, (18, 23), (12.0, 0.3, 0.166_666_671_63), 0.529, 35),
    rank(7801, (28, 33), (17.0, 0.3, 0.114_285_707_47), 0.571, 50),
    rank(7802, (38, 43), (25.0, 0.4, 0.117_647_059_26), 0.571, 70),
    rank(11762, (48, 53), (35.0, 0.5, 0.112_676_046_79), 0.571, 95),
    rank(
        11763,
        (58, 63),
        (44.0, 0.600_000_023_841_857_9, 0.113_636_367_02),
        0.571,
        115,
    ),
];

/// The Succubus's Lash of Pain, ranks 1 to 6: a fixed base at every level, 0.429 of spell
/// power, learned from level 20. Rank 6 (11780) is the reference's 50.
pub(crate) const LASH_OF_PAIN_RANKS: [DemonRank; 6] = [
    rank(7814, (20, 0), (16.0, 0.0, 0.0), 0.429, 65),
    rank(7815, (28, 0), (22.0, 0.0, 0.0), 0.429, 80),
    rank(7816, (36, 0), (30.0, 0.0, 0.0), 0.429, 105),
    rank(11778, (44, 0), (36.0, 0.0, 0.0), 0.429, 125),
    rank(11779, (52, 0), (43.0, 0.0, 0.0), 0.429, 145),
    rank(11780, (60, 0), (50.0, 0.0, 0.0), 0.429, 160),
];

/// The rank of a demon's spell a levelling warlock's demon casts: the highest rank its
/// spellbook lists, or the highest its level teaches, None below the first rank. A demon learns
/// the first rank of each spell when it is summoned and the warlock trains it the others as it
/// levels (`SkillLineAbility` AcquireMethod 2 on rank 1, 0 on the others, taught by the Learn
/// Pet Spell rows 7832 to 7835 and the grimoires' 20270 and on). The addon reads the warlock's
/// own spellbook, not the demon's, so today the level decides, which takes every rank as
/// trained: public logs show 4 of 6 levelling Imps on the top rank of their level and 2 a rank
/// behind (docs/levels.md). At level 60, and under the reference's rules, it is the top rank.
pub(crate) fn demon_rank(ranks: &'static [DemonRank], rules: Rules) -> Option<&'static DemonRank> {
    let level = character_level();
    if rules == Rules::GoReference || level == MAX_LEVEL {
        return ranks.last();
    }
    let by_level = ranks
        .iter()
        .rev()
        .filter(|rank| i32::from(rank.spell_level) <= level);
    by_level
        .clone()
        .find(|rank| spellbook_lists(rank.id))
        .or_else(|| by_level.clone().next())
}

/// Go `petStatInheritance`: linear in the owner's stats, which the dynamic inheritance needs.
/// Melee hit is Rust only: Go gives the demon its master's whole spell hit as melee hit, Forever
/// scales it to the melee cap ([`DEMON_MELEE_HIT_PER_SPELL_HIT`]).
fn pet_stat_inheritance(sim: &mut Sim, owner: &Stats) -> Stats {
    let mut inherited = Stats::default();
    inherited[Stat::SpellHitPercent] = owner[Stat::SpellHitPercent];
    inherited[Stat::PhysicalHitPercent] = match sim.rules {
        Rules::Forever => owner[Stat::SpellHitPercent] * DEMON_MELEE_HIT_PER_SPELL_HIT,
        Rules::GoReference => owner[Stat::SpellHitPercent],
    };
    inherited[Stat::SpellCritPercent] = owner[Stat::SpellCritPercent];
    inherited[Stat::PhysicalCritPercent] = owner[Stat::SpellCritPercent];
    inherited[Stat::SpellDamage] = owner[Stat::SpellDamage] * PET_SPELL_DAMAGE_FRACTION;
    inherited[Stat::AttackPower] = owner[Stat::SpellDamage] * PET_ATTACK_POWER_FRACTION;
    inherited
}

/// What Go's `WarlockPet` adds to its `core.Pet`.
#[derive(Clone, Debug)]
pub(crate) struct WarlockPet {
    pub unit: UnitId,
    /// Go `MinMana`: the minimum mana the AI keeps before it casts again.
    pub min_mana: f64,
    /// Go `AutoCastAbilities`.
    pub auto_cast_abilities: Vec<SpellId>,
}

/// Go `Warlock.BasePets`, `ActivePet` and the demons by name.
#[derive(Clone, Debug, Default)]
pub(crate) struct WarlockPets {
    pub base: Vec<UnitId>,
    pub pets: Vec<WarlockPet>,
    pub imp: Option<UnitId>,
    pub voidwalker: Option<UnitId>,
    pub succubus: Option<UnitId>,
    pub felhunter: Option<UnitId>,
    pub active: Option<UnitId>,
}

impl WarlockPets {
    /// The state of a demon.
    pub(super) fn state(&self, unit: UnitId) -> Option<&WarlockPet> {
        self.pets.iter().find(|pet| pet.unit == unit)
    }

    fn state_mut(&mut self, unit: UnitId) -> Option<&mut WarlockPet> {
        self.pets.iter_mut().find(|pet| pet.unit == unit)
    }
}

impl Warlock {
    /// Go `Warlock.registerPets`.
    pub(super) fn register_pets(&mut self, sim: &mut Sim, owner: UnitId) {
        let imp = self.register_pet(sim, owner, Summon::Imp);
        let voidwalker = self.register_pet(sim, owner, Summon::Voidwalker);
        let succubus = self.register_pet(sim, owner, Summon::Succubus);
        let felhunter = self.register_pet(sim, owner, Summon::Felhunter);
        self.pets.imp = Some(imp);
        self.pets.voidwalker = Some(voidwalker);
        self.pets.succubus = Some(succubus);
        self.pets.felhunter = Some(felhunter);
        self.pets.base = vec![imp, voidwalker, succubus, felhunter];
    }

    /// Go `Warlock.registerPet`.
    fn register_pet(&mut self, sim: &mut Sim, owner: UnitId, summon: Summon) -> UnitId {
        let enabled_on_start = self.options.enum_name("summon") == summon.name();
        let share = demon_level_share(sim.rules);

        let pet = sim.new_pet(PetConfig {
            name: summon.name().to_string(),
            owner,
            base_stats: summon.base_stats_at(share),
            stat_inheritance: Rc::new(pet_stat_inheritance),
            enabled_on_start,
            is_guardian: false,
            is_dynamic: true,
            has_dynamic_melee_speed_inheritance: false,
            has_dynamic_cast_speed_inheritance: false,
            has_resource_regen_inheritance: false,
            starts_at_owner_distance: false,
        });

        sim.enable_pet_mana_bar(pet);
        let tables = crate::data::tables::tables();
        let crit = |table: &std::collections::BTreeMap<String, f64>, class: &str| {
            table.get(class).copied().unwrap_or(0.0)
        };
        if summon == Summon::Imp {
            sim.unit_mut(pet).sdm.add_stat_dependency(
                Stat::Intellect,
                Stat::SpellCritPercent,
                crit(&tables.crit_per_int_max_level, "ClassMage"),
            );
        } else {
            let sdm = &mut sim.unit_mut(pet).sdm;
            sdm.add_stat_dependency(Stat::Strength, Stat::AttackPower, 2.0);
            sdm.add_stat_dependency(
                Stat::Agility,
                Stat::PhysicalCritPercent,
                crit(&tables.crit_per_agi_max_level, "ClassWarrior"),
            );
            sdm.add_stat_dependency(
                Stat::Intellect,
                Stat::SpellCritPercent,
                crit(&tables.crit_per_int_max_level, "ClassWarrior"),
            );

            if let Some((min, max)) = summon.auto_attack_at(share) {
                sim.enable_auto_attacks(
                    pet,
                    AutoAttackOptions {
                        main_hand: Weapon {
                            base_damage_min: min,
                            base_damage_max: max,
                            swing_speed: 2.0,
                            ..Weapon::default()
                        },
                        auto_swing_melee: true,
                        ..AutoAttackOptions::default()
                    },
                );
            }
        }

        if enabled_on_start {
            self.pets.active = Some(pet);
            sim.register_reset_effect(owner, Rc::new(|_: &mut Sim| {}));
        }

        sim.add_pet(owner, pet);
        self.pets.pets.push(WarlockPet {
            unit: pet,
            min_mana: 0.0,
            auto_cast_abilities: Vec::new(),
        });
        pet
    }

    /// Go `Warlock.registerPetAbilities`. The Voidwalker's Torment is a threat ability and is not
    /// modelled.
    pub(super) fn register_pet_abilities(&mut self, sim: &mut Sim) {
        if let Some(imp) = self.pets.imp {
            if let Some(rank) = demon_rank(&FIREBOLT_RANKS, sim.rules) {
                self.register_firebolt_spell(sim, imp, rank);
            }
        }
        if let Some(succubus) = self.pets.succubus {
            // A Succubus summoned below 20, where its warlock cannot summon one, knows no Lash.
            if let Some(rank) = demon_rank(&LASH_OF_PAIN_RANKS, sim.rules) {
                self.register_lash_of_pain_spell(sim, succubus, rank);
            }
        }
    }

    /// The generator makes no table for pet spells, so Firebolt carries the client's rank rows
    /// ([`FIREBOLT_RANKS`], beta 1.60.1 values), at the rank the warlock's level teaches.
    /// Firebolt's cooldown is the Imp's pause between casts: beta logs time 281 Firebolts 2.435
    /// sec apart (median, 2.0 sec cast, next one starting 0.43 sec after the last lands), and
    /// levelling Imps at 12 to 18 the same (median 2.44 sec over 790 casts, ranks 2 and 3). The
    /// rotation polls every 100 ms, so 400 ms lands the next cast 2.4 sec after the last; 430
    /// would round up to 2.5. Improved Imp rides on its talent as a SpellMod.
    fn register_firebolt_spell(&mut self, sim: &mut Sim, pet: UnitId, rank: &DemonRank) {
        if let Some(state) = self.pets.state_mut(pet) {
            state.min_mana = f64::from(rank.cost);
        }
        let timer = sim.new_timer(pet);
        let spell = sim.register_spell(
            pet,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::FIRE,
                defense_type: DefenseType::Magic,
                proc_mask: ProcMask::SPELL_DAMAGE,
                class_spell_mask: masks::IMP_FIRE_BOLT,
                cost: CostOptions {
                    mana_flat_cost: rank.cost,
                    ..CostOptions::default()
                },
                cast: CastConfig {
                    default_cast: Cast {
                        gcd: GCD_MIN,
                        cast_time: 2 * SECOND,
                        ..Cast::default()
                    },
                    cd: Cooldown {
                        timer: Some(timer),
                        duration: 400 * MILLISECOND,
                    },
                    ..CastConfig::default()
                },
                damage_multiplier_additive: 1.0,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                bonus_coefficient: rank.coefficient,
                ..SpellConfig::default()
            },
        );
        if let Some(state) = self.pets.state_mut(pet) {
            state.auto_cast_abilities.push(spell);
        }
    }

    /// Lash of Pain carries the client's rank rows ([`LASH_OF_PAIN_RANKS`], beta 1.60.1 values),
    /// at the rank the warlock's level teaches. Improved Sayaad rides on its talent as a
    /// SpellMod.
    fn register_lash_of_pain_spell(&mut self, sim: &mut Sim, pet: UnitId, rank: &DemonRank) {
        if let Some(state) = self.pets.state_mut(pet) {
            state.min_mana = f64::from(rank.cost);
        }
        let timer = sim.new_timer(pet);
        let spell = sim.register_spell(
            pet,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::SHADOW,
                defense_type: DefenseType::Magic,
                proc_mask: ProcMask::SPELL_DAMAGE,
                class_spell_mask: masks::SUCCUBUS_LASH_OF_PAIN,
                cost: CostOptions {
                    mana_flat_cost: rank.cost,
                    ..CostOptions::default()
                },
                cast: CastConfig {
                    default_cast: Cast {
                        gcd: GCD_DEFAULT,
                        ..Cast::default()
                    },
                    ignore_haste: true,
                    cd: Cooldown {
                        timer: Some(timer),
                        duration: 12 * SECOND,
                    },
                    ..CastConfig::default()
                },
                damage_multiplier_additive: 1.0,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                bonus_coefficient: rank.coefficient,
                ..SpellConfig::default()
            },
        );
        if let Some(state) = self.pets.state_mut(pet) {
            state.auto_cast_abilities.push(spell);
        }
    }
}
