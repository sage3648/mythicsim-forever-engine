//! Go sim/hunter/pet.go and pet_abilities.go: the hunter's pet, a `core.Pet` with a family's
//! abilities. Its rotation only runs in a fight; preparation builds the pet, registers its
//! abilities at initialization and leaves it enabled by the reset.

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::attack::{AutoAttackOptions, Weapon};
use crate::prepare::character::constants::{CHARACTER_LEVEL, MAX_MELEE_RANGE};
use crate::prepare::parse_effects::{parse_effects, ParseOptions};
use crate::prepare::pet::PetConfig;
use crate::prepare::sim::{
    school_array_index, AuraConfig, AuraId, Cooldown, Sim, SpellId, UnitId, UnitType,
    NEVER_EXPIRES, SECOND,
};
use crate::prepare::spell::{
    school, Cast, CastConfig, CostOptions, DefenseType, DotConfig, ProcMask, SpellConfig, SpellFlag,
};
use crate::prepare::spelldata::Spell as Row;
use crate::prepare::stats::{SchoolIndex, Stat, Stats};
use std::rc::Rc;

use super::spell_data::spell_data;
use super::{masks, Hunter};

/// Go `PetGCD`: pet AI doesn't use abilities immediately, so model this with a 1.6s GCD.
const PET_GCD: i64 = 1_600_000_000;

/// Go `PetAbilityType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetAbility {
    Unknown,
    Bite,
    Claw,
    LightningBreath,
    Screech,
    ScorpidPoison,
    SavageRend,
    Pinch,
    Dismember,
    Mine,
    TendonRip,
    DustCloud,
    Thunderstomp,
    Swipe,
    Web,
}

/// Go `PetConfig` of the hunter package: a family's name, abilities and scalars. Its custom
/// rotation is named by `rotation`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FamilyConfig {
    pub name: &'static str,
    pub special_ability: PetAbility,
    pub focus_dump: PetAbility,
    pub extra_ability: PetAbility,
    pub health: f64,
    pub armor: f64,
    pub damage: f64,
}

/// Go `DefaultPetConfigs`.
fn family(pet_type: &str) -> Option<FamilyConfig> {
    use PetAbility::*;
    let config = |name, special_ability, focus_dump, extra_ability, health, armor, damage| {
        Some(FamilyConfig {
            name,
            special_ability,
            focus_dump,
            extra_ability,
            health,
            armor,
            damage,
        })
    };
    match pet_type {
        "Cat" => config("Cat", Bite, Claw, Unknown, 0.98, 1.00, 1.10),
        "WindSerpent" => config(
            "Wind Serpent",
            Bite,
            LightningBreath,
            Unknown,
            1.00,
            1.00,
            1.07,
        ),
        "Wolf" => config("Wolf", Unknown, Bite, Unknown, 1.00, 1.05, 1.00),
        "Bat" => config("Bat", Bite, Screech, Unknown, 1.00, 1.00, 1.07),
        "Bear" => config("Bear", Bite, Claw, Swipe, 1.08, 1.05, 0.91),
        "Owl" => config("Owl", Unknown, Claw, Mine, 1.00, 1.00, 1.07),
        "Boar" => config("Boar", Unknown, Bite, Unknown, 1.04, 1.09, 0.90),
        "CarrionBird" => config("Carrion Bird", Bite, Claw, Unknown, 1.00, 1.05, 1.00),
        "Crab" => config("Crab", Unknown, Claw, Pinch, 0.96, 1.13, 0.95),
        "Crocolisk" => config("Crocolisk", Bite, Dismember, Unknown, 0.95, 1.10, 1.00),
        "Gorilla" => config("Gorilla", Unknown, Bite, Thunderstomp, 1.04, 1.00, 1.02),
        "Hyena" => config("Hyena", Unknown, Bite, TendonRip, 1.00, 1.05, 1.00),
        "Raptor" => config("Raptor", Bite, Claw, SavageRend, 0.95, 1.03, 1.10),
        "Scorpid" => config("Scorpid", ScorpidPoison, Claw, Unknown, 1.00, 1.10, 0.94),
        "Spider" => config("Spider", Unknown, Bite, Web, 1.00, 1.00, 1.07),
        "Tallstrider" => config("Tallstrider", Unknown, Bite, DustCloud, 1.05, 1.00, 1.00),
        "Turtle" => config("Turtle", Unknown, Bite, Unknown, 1.00, 1.13, 0.90),
        _ => None,
    }
}

/// Go `petAttackSpeeds`.
fn pet_attack_speed(name: &str) -> f64 {
    match name {
        "One" => 1.0,
        "OneTwo" => 1.2,
        "OneThree" => 1.3,
        "OneFour" => 1.4,
        "OneFive" => 1.5,
        "OneSix" => 1.6,
        "OneSeven" => 1.7,
        "Two" => 2.0,
        "TwoFour" => 2.4,
        "TwoFive" => 2.5,
        _ => 0.0,
    }
}

/// What Go's `HunterPet` adds to its `core.Pet`.
pub(crate) struct HunterPetState {
    pub config: FamilyConfig,
    pub bestial_wrath_aura: Option<AuraId>,
    pub special_ability: Option<SpellId>,
    pub focus_dump: Option<SpellId>,
    pub extra_ability: Option<SpellId>,
}

/// Go `NewHunterPet`.
pub(super) fn new_hunter_pet(sim: &mut Sim, hunter: &mut Hunter) {
    let pet_type = hunter.options.enum_name("pet_type");
    if pet_type == "PetNone" {
        return;
    }
    if hunter.options.f64("pet_uptime") <= 0.0 {
        return;
    }
    let Some(family) = family(&pet_type) else {
        return;
    };
    let attack_speed = pet_attack_speed(&hunter.options.enum_name("pet_attack_speed"));
    let owner = hunter.unit;

    let mut base_stats = Stats::default();
    base_stats[Stat::Strength] = 136.0;
    base_stats[Stat::Agility] = 100.0;
    base_stats[Stat::Stamina] = 274.0;
    base_stats[Stat::Intellect] = 50.0;
    base_stats[Stat::Spirit] = 80.0;
    // Apparently pets and warriors have an AP penalty.
    base_stats[Stat::AttackPower] = -20.0;
    let pet = sim.new_pet(PetConfig {
        name: family.name.to_string(),
        owner,
        base_stats,
        // Forever is a Classic-era realm: the pet takes none of its owner's stats.
        stat_inheritance: Rc::new(|_| Stats::default()),
        enabled_on_start: true,
        is_guardian: false,
        is_dynamic: true,
        has_dynamic_melee_speed_inheritance: false,
        has_dynamic_cast_speed_inheritance: false,
        has_resource_regen_inheritance: false,
        starts_at_owner_distance: true,
    });
    sim.unit_mut(pet).mob_type = "MobTypeBeast".to_string();

    let tables = crate::data::tables::tables();
    let crit_per_agi = tables
        .crit_per_agi_max_level
        .get("ClassWarrior")
        .copied()
        .unwrap_or(0.0);
    let crit_per_int = tables
        .crit_per_int_max_level
        .get("ClassWarrior")
        .copied()
        .unwrap_or(0.0);
    {
        let sdm = &mut sim.unit_mut(pet).sdm;
        sdm.add_stat_dependency(Stat::Strength, Stat::AttackPower, 2.0);
        // Warrior crit scaling.
        sdm.add_stat_dependency(Stat::Agility, Stat::PhysicalCritPercent, crit_per_agi);
        sdm.add_stat_dependency(Stat::Intellect, Stat::SpellCritPercent, crit_per_int);
    }

    // Bestial Discipline buys the pet 10% focus regen a rank (19590 effect 1).
    let focus_multiplier = spell_data()
        .bestial_discipline
        .effect_at(1)
        .multiplier_at(hunter.t("bestial_discipline"));
    sim.enable_focus_bar(pet, focus_multiplier);

    sim.enable_auto_attacks(
        pet,
        AutoAttackOptions {
            main_hand: Weapon {
                base_damage_min: 18.17 * attack_speed,
                base_damage_max: 27.66 * attack_speed,
                swing_speed: attack_speed,
                max_range: MAX_MELEE_RANGE,
                ..Weapon::default()
            },
            auto_swing_melee: true,
            ..AutoAttackOptions::default()
        },
    );

    // Happiness.
    sim.unit_mut(pet).pseudo_stats.damage_dealt_multiplier *= 1.25;

    // Family scalars.
    let pseudo = &mut sim.unit_mut(pet).pseudo_stats;
    pseudo.school_damage_dealt_multiplier[school_array_index(SchoolIndex::Physical)] *=
        family.damage;
    pseudo.armor_multiplier *= family.armor;
    sim.unit_mut(pet)
        .sdm
        .multiply_stat(Stat::Health, family.health);

    sim.add_pet(owner, pet);
    hunter.pet = Some(pet);
    hunter.pet_state = Some(HunterPetState {
        config: family,
        bestial_wrath_aura: None,
        special_ability: None,
        focus_dump: None,
        extra_ability: None,
    });
}

impl Hunter {
    /// Go `HunterPet.Initialize`.
    pub(super) fn initialize_hunter_pet(&mut self, sim: &mut Sim, pet: UnitId) {
        let Some(state) = self.pet_state.as_ref() else {
            return;
        };
        let (special, dump, extra) = (
            state.config.special_ability,
            state.config.focus_dump,
            state.config.extra_ability,
        );
        let special_ability = new_pet_ability(sim, pet, special);
        let focus_dump = new_pet_ability(sim, pet, dump);
        let extra_ability = new_pet_ability(sim, pet, extra);
        if let Some(state) = self.pet_state.as_mut() {
            state.special_ability = special_ability;
            state.focus_dump = focus_dump;
            state.extra_ability = extra_ability;
        }
    }
}

/// The fields every pet ability shares.
fn ability_base(row_action: ActionId, school: u8, defense: DefenseType) -> SpellConfig {
    SpellConfig {
        action_id: row_action,
        spell_school: school,
        defense_type: defense,
        class_spell_mask: masks::PET_DAMAGE,
        max_range: MAX_MELEE_RANGE,
        damage_multiplier: 1.0,
        threat_multiplier: 1.0,
        has_extra_cast_condition: true,
        ..SpellConfig::default()
    }
}

fn pet_cast(cd: Option<Cooldown>) -> CastConfig {
    CastConfig {
        default_cast: Cast {
            gcd: PET_GCD,
            ..Cast::default()
        },
        ignore_haste: true,
        cd: cd.unwrap_or_default(),
        ..CastConfig::default()
    }
}

fn focus(cost: i32) -> CostOptions {
    CostOptions {
        focus_cost: cost,
        ..CostOptions::default()
    }
}

/// Go `NewPetAbility`.
fn new_pet_ability(sim: &mut Sim, pet: UnitId, ability: PetAbility) -> Option<SpellId> {
    let data = spell_data();
    match ability {
        PetAbility::Bite => Some(new_bite(sim, pet)),
        PetAbility::Claw => Some(new_claw(sim, pet)),
        PetAbility::LightningBreath => Some(new_lightning_breath(sim, pet)),
        // Demoralizing Screech: a single melee hit off the client row. The attack power
        // reduction is left out.
        PetAbility::Screech => Some(new_pet_strike(
            sim,
            pet,
            data.demoralizing_screech_triggered.highest(),
        )),
        PetAbility::ScorpidPoison => Some(new_scorpid_poison(sim, pet)),
        PetAbility::SavageRend => Some(new_pet_bleed(
            sim,
            pet,
            data.savage_rend_triggered.highest(),
        )),
        PetAbility::TendonRip => Some(new_pet_bleed(sim, pet, data.tendon_rip_triggered.highest())),
        PetAbility::Web => Some(new_pet_bleed(sim, pet, data.web_triggered.highest())),
        PetAbility::Pinch => Some(new_pet_strike(sim, pet, data.pinch_triggered.highest())),
        PetAbility::Dismember => Some(new_pet_strike(sim, pet, data.dismember_triggered.highest())),
        PetAbility::Mine => Some(new_pet_strike(sim, pet, data.mine_triggered.highest())),
        PetAbility::DustCloud => Some(new_dust_cloud(sim, pet)),
        PetAbility::Thunderstomp => Some(new_thunderstomp(sim, pet)),
        PetAbility::Swipe => Some(new_swipe(sim, pet)),
        PetAbility::Unknown => None,
    }
}

fn new_bite(sim: &mut Sim, pet: UnitId) -> SpellId {
    let timer = sim.new_timer(pet);
    sim.register_spell(
        pet,
        SpellConfig {
            proc_mask: ProcMask::MELEE_MH_SPECIAL,
            flags: SpellFlag::MELEE_METRICS,
            cost: focus(35),
            cast: pet_cast(Some(Cooldown {
                timer: Some(timer),
                duration: 10 * SECOND,
            })),
            bonus_coefficient: 1.0,
            ..ability_base(ActionId::spell(17261), school::PHYSICAL, DefenseType::Melee)
        },
    )
}

fn new_claw(sim: &mut Sim, pet: UnitId) -> SpellId {
    sim.register_spell(
        pet,
        SpellConfig {
            proc_mask: ProcMask::MELEE_MH_SPECIAL,
            flags: SpellFlag::MELEE_METRICS,
            cost: focus(25),
            cast: pet_cast(None),
            bonus_coefficient: 1.0,
            ..ability_base(ActionId::spell(3009), school::PHYSICAL, DefenseType::Melee)
        },
    )
}

/// Beta client: every rank lower than Classic's, and no more growth per level. Rank 6 is 86-98.
fn new_lightning_breath(sim: &mut Sim, pet: UnitId) -> SpellId {
    sim.register_spell(
        pet,
        SpellConfig {
            proc_mask: ProcMask::SPELL_DAMAGE,
            max_range: 20.0,
            cost: focus(50),
            cast: pet_cast(None),
            ..ability_base(ActionId::spell(25012), school::NATURE, DefenseType::Magic)
        },
    )
}

/// Beta client: 5 a tick at rank 4, down from 8.
fn new_scorpid_poison(sim: &mut Sim, pet: UnitId) -> SpellId {
    let timer = sim.new_timer(pet);
    sim.register_spell(
        pet,
        SpellConfig {
            proc_mask: ProcMask::MELEE_MH_SPECIAL,
            flags: SpellFlag::PASSIVE_SPELL | SpellFlag::POISON,
            cost: focus(30),
            cast: pet_cast(Some(Cooldown {
                timer: Some(timer),
                duration: 4 * SECOND,
            })),
            dot: DotConfig {
                aura: AuraConfig {
                    label: "Scorpid Poison".to_string(),
                    max_stacks: 5,
                    duration: 10 * SECOND,
                    ..AuraConfig::default()
                },
                number_of_ticks: 5,
                tick_length: 2 * SECOND,
                ..DotConfig::default()
            },
            ..ability_base(ActionId::spell(24587), school::NATURE, DefenseType::Melee)
        },
    )
}

/// Savage Rend, Tendon Rip and Web: a melee or ranged hit that lands a bleed, everything read off
/// the client row.
fn new_pet_bleed(sim: &mut Sim, pet: UnitId, rank: &'static Row) -> SpellId {
    let tick = rank.periodic_effect();
    let tick_length = tick.period();
    let timer = sim.new_timer(pet);
    sim.register_spell(
        pet,
        SpellConfig {
            proc_mask: ProcMask::MELEE_MH_SPECIAL,
            flags: SpellFlag::MELEE_METRICS,
            cost: focus(rank.cost() as i32),
            cast: pet_cast(Some(Cooldown {
                timer: Some(timer),
                duration: rank.cooldown(),
            })),
            dot: DotConfig {
                aura: AuraConfig {
                    label: rank.name.clone(),
                    ..AuraConfig::default()
                },
                number_of_ticks: (rank.duration() / tick_length) as i32,
                tick_length,
                ..DotConfig::default()
            },
            ..ability_base(
                ActionId::spell(rank.id),
                rank.spell_school(),
                rank.defense_type_core(),
            )
        },
    )
}

/// Pinch, Dismember, Mine! and Demoralizing Screech: a single melee hit with the damage, focus
/// cost and cooldown read off the client row.
fn new_pet_strike(sim: &mut Sim, pet: UnitId, rank: &'static Row) -> SpellId {
    let timer = sim.new_timer(pet);
    sim.register_spell(
        pet,
        SpellConfig {
            proc_mask: ProcMask::MELEE_MH_SPECIAL,
            flags: SpellFlag::MELEE_METRICS,
            cost: focus(rank.cost() as i32),
            cast: pet_cast(Some(Cooldown {
                timer: Some(timer),
                duration: rank.cooldown(),
            })),
            ..ability_base(
                ActionId::spell(rank.id),
                rank.spell_school(),
                rank.defense_type_core(),
            )
        },
    )
}

/// Dust Cloud is new in Forever and the Tallstrider's alone: the target's armor down by the
/// client row's amount for 30 sec. The pet recasts it when it falls off.
fn new_dust_cloud(sim: &mut Sim, pet: UnitId) -> SpellId {
    let rank = spell_data().dust_cloud_triggered.highest();
    // NewEnemyAuraArray: an aura on each enemy unit, by unit index.
    let units = sim.all_units();
    let mut auras: Vec<Option<AuraId>> = vec![None; units.len()];
    for target in units {
        if sim.unit(target).unit_type != UnitType::Enemy {
            continue;
        }
        let aura = sim.get_or_register_aura(
            target,
            AuraConfig {
                label: rank.name.clone(),
                action_id: Some(ActionId::spell(rank.id)),
                duration: rank.duration(),
                ..AuraConfig::default()
            },
        );
        parse_effects(
            sim,
            None,
            aura,
            rank,
            ParseOptions {
                level: CHARACTER_LEVEL,
                ignore_stacks: true,
                exclusive: Some(("DustCloud".to_string(), true)),
                ..ParseOptions::default()
            },
        );
        let index = sim.unit(target).unit_index as usize;
        auras[index] = Some(aura);
    }
    let mut related = std::collections::BTreeMap::new();
    if auras.iter().any(Option::is_some) {
        related.insert(rank.name.clone(), auras);
    }
    sim.register_spell(
        pet,
        SpellConfig {
            action_id: ActionId::spell(rank.id),
            spell_school: rank.spell_school(),
            defense_type: rank.defense_type_core(),
            proc_mask: ProcMask::MELEE_MH_SPECIAL,
            flags: SpellFlag::MELEE_METRICS,
            max_range: MAX_MELEE_RANGE,
            cost: focus(rank.cost() as i32),
            cast: pet_cast(None),
            threat_multiplier: 1.0,
            has_extra_cast_condition: true,
            related_aura_arrays: related,
            ..SpellConfig::default()
        },
    )
}

/// Thunderstomp is the Gorilla's: Nature damage to up to the row's enemies around the pet.
fn new_thunderstomp(sim: &mut Sim, pet: UnitId) -> SpellId {
    let rank = spell_data().thunderstomp_triggered.highest();
    let timer = sim.new_timer(pet);
    sim.register_spell(
        pet,
        SpellConfig {
            proc_mask: ProcMask::SPELL_DAMAGE,
            cost: focus(rank.cost() as i32),
            cast: pet_cast(Some(Cooldown {
                timer: Some(timer),
                duration: rank.cooldown(),
            })),
            ..ability_base(
                ActionId::spell(rank.id),
                rank.spell_school(),
                rank.defense_type_core(),
            )
        },
    )
}

/// Swipe is new in Forever and the Bear's alone: a melee hit on up to 3 enemies.
fn new_swipe(sim: &mut Sim, pet: UnitId) -> SpellId {
    let rank = spell_data().swipe_triggered.highest();
    let timer = sim.new_timer(pet);
    sim.register_spell(
        pet,
        SpellConfig {
            proc_mask: ProcMask::MELEE_MH_SPECIAL,
            flags: SpellFlag::MELEE_METRICS,
            cost: focus(rank.cost() as i32),
            cast: pet_cast(Some(Cooldown {
                timer: Some(timer),
                duration: rank.cooldown(),
            })),
            ..ability_base(
                ActionId::spell(rank.id),
                rank.spell_school(),
                rank.defense_type_core(),
            )
        },
    )
}

#[allow(dead_code)]
const _NEVER: i64 = NEVER_EXPIRES;
