//! Go sim/hunter/pet.go and pet_abilities.go: the hunter's pet, a `core.Pet` with a family's
//! abilities. Its rotation only runs in a fight; preparation builds the pet, registers its
//! abilities at initialization and leaves it enabled by the reset.

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::attack::{AutoAttackOptions, Weapon};
use crate::prepare::character::constants::MAX_MELEE_RANGE;
use crate::prepare::levels::character_level;
use crate::prepare::parse_effects::{parse_effects, ParseOptions};
use crate::prepare::pet::PetConfig;
use crate::prepare::sim::{
    school_array_index, AuraConfig, AuraId, Cooldown, Rules, Sim, SpellId, StateSlot, UnitId,
    UnitType, NEVER_EXPIRES, SECOND,
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
    FuriousHowl,
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
        "Wolf" => config("Wolf", Unknown, Bite, FuriousHowl, 1.00, 1.05, 1.00),
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

/// A pet's spell hit for each point of its hunter's ranged hit: the 17% spell hit cap over the 8%
/// ranged hit cap. Forever's developers on their tracker (22 October 2026): "Hunters give melee hit
/// equal to their ranged hit, and their spell hit to their pet equal to 17/8ths of their ranged hit."
pub(crate) const PET_SPELL_HIT_PER_RANGED_HIT: f64 = 17.0 / 8.0;

/// The client's creature auto attack dps for the character level, 60 (`ExpectedStat`, content set
/// 0, Lvl 60, CreatureAutoAttackDps, from wago.tools' wowf tables on 6 November 2026; the
/// reference's `CreatureAutoAttackDpsMaxLevel`).
const CREATURE_AUTO_ATTACK_DPS: f64 = 60.452_915_191_650_39;

/// A tamed pet's own attack power is this share of the creature dps, 14 attack power a point of
/// dps, with nothing from Strength: the pet sheets of a level 11, a level 17 and a level 30 pet
/// read 36, 61 and 103, each the row's dps (8.60, 14.56 and 24.69) times 4.2 rounded down.
const PET_OWN_ATTACK_POWER_SHARE: f64 = 0.3;

/// Attack power for each point of weapon dps.
const ATTACK_POWER_PER_DPS: f64 = 14.0;

/// The share of its owner's higher attack power (melee or ranged) a tamed pet inherits, read on
/// the pet sheets beside the pet's own: 10% rounded down at all three levels.
const PET_OWNER_ATTACK_POWER_SHARE: f64 = 0.1;

/// A tamed pet's swing spans a fifth either side of its average.
const PET_SWING_SPREAD: f64 = 0.2;

/// The creature auto attack dps the Forever pet and hawk models scale with.
pub(super) fn creature_auto_attack_dps() -> f64 {
    CREATURE_AUTO_ATTACK_DPS
}

/// A tamed pet's own attack power under Forever's rules: 4.2 a point of the creature dps.
pub(super) fn tamed_pet_attack_power() -> f64 {
    PET_OWN_ATTACK_POWER_SHARE * ATTACK_POWER_PER_DPS * CREATURE_AUTO_ATTACK_DPS
}

/// A tamed pet's swing before attack power under Forever's rules, as (low, high) damage a
/// second. The creature dps is the pet's auto attack damage with its own attack power in it, so
/// the weapon is the creature dps less that attack power over 14, 42.3 at level 60. A level 30
/// Cat's 152 logged swings before armor, 42-55 at 119 attack power and 46-59 at 146 with
/// Unleashed Fury 5/5, are this weapon (the level 30 row's 17.29 a second, 20% either side, over
/// the swing as sped up to 1.2 seconds) plus attack power over 14, times the Cat's 1.1, happiness
/// 1.25 and Unleashed Fury 1.15: 42.4 to 55.5 and 46.0 to 59.1. Using the whole creature dps as
/// the weapon puts 131 of those 152 outside the range. That the same shares hold at level 60 is
/// an extrapolation: the client's creature dps row is the only level 60 input.
pub(super) fn tamed_pet_weapon_dps() -> (f64, f64) {
    let average = CREATURE_AUTO_ATTACK_DPS - tamed_pet_attack_power() / ATTACK_POWER_PER_DPS;
    (
        average * (1.0 - PET_SWING_SPREAD),
        average * (1.0 + PET_SWING_SPREAD),
    )
}

/// What Go's `HunterPet` adds to its `core.Pet`.
#[derive(Clone)]
pub(crate) struct HunterPetState {
    pub config: FamilyConfig,
    pub bestial_wrath_aura: Option<AuraId>,
    pub special_ability: Option<SpellId>,
    pub focus_dump: Option<SpellId>,
    pub extra_ability: Option<SpellId>,
}

/// Go `Hunter.makeStatInheritance`. Forever is a Classic-era realm: the pet takes none of its
/// owner's stats but two. From 16 October 2026 its crit is its master's (patch 136) and the notes
/// say "Pets now inherit whichever crit is higher, melee or ranged" and "Pets now benefit from
/// Expertise" (patch 138): the pet takes PhysicalCritPercent plus the higher of the Auto Shot's
/// ranged-only crit and the main-hand swing's melee-only crit, and the owner's ExpertisePercent.
/// The max is not linear, so `offset` holds the offset on the pet and each update moves it to
/// what the owner's current stats and spell mods give. Go `PetsInheritHit` (patch 178, Hunter
/// Pet Scaling 415429): the pet also takes the owner's melee plus ranged hit, the higher of the
/// two since ranged hit only adds, which is linear. Spell hit is Rust only: Forever gives the pet
/// [`PET_SPELL_HIT_PER_RANGED_HIT`] of that, which Go never did.
fn stat_inheritance(owner: UnitId, offset: StateSlot) -> impl Fn(&mut Sim, &Stats) -> Stats {
    move |sim: &mut Sim, owner_stats: &Stats| {
        let now = pet_crit_offset_now(sim, owner);
        let change = now - sim.state(offset);
        sim.set_state(offset, now);
        let mut inherited = Stats::default();
        inherited[Stat::PhysicalCritPercent] = owner_stats[Stat::PhysicalCritPercent] + change;
        inherited[Stat::ExpertisePercent] = owner_stats[Stat::ExpertisePercent];
        inherited[Stat::PhysicalHitPercent] =
            owner_stats[Stat::PhysicalHitPercent] + owner_stats[Stat::RangedHitPercent];
        if sim.rules == Rules::Forever {
            // Each term on its own, so the sum is the terms' sum bit for bit, as the dynamic
            // inheritance replays it.
            inherited[Stat::SpellHitPercent] = owner_stats[Stat::PhysicalHitPercent]
                * PET_SPELL_HIT_PER_RANGED_HIT
                + owner_stats[Stat::RangedHitPercent] * PET_SPELL_HIT_PER_RANGED_HIT;
            // A tenth of the owner's higher attack power. The higher is the one the owner has
            // at the reset, which a linear inheritance cannot swap during a fight: a ranged
            // attack power above the melee one, as an Agility hunter's is, follows Aspect of
            // the Hawk and the other ranged auras.
            let owner_now = sim.stats(owner);
            let source = if owner_now[Stat::AttackPower] > owner_now[Stat::RangedAttackPower] {
                Stat::AttackPower
            } else {
                Stat::RangedAttackPower
            };
            inherited[Stat::AttackPower] = owner_stats[source] * PET_OWNER_ATTACK_POWER_SHARE;
        }
        inherited
    }
}

/// Go `Hunter.petCritOffsetNow`: the higher of the Auto Shot's ranged-only crit and the main-hand
/// swing's melee-only crit.
pub(super) fn pet_crit_offset_now(sim: &Sim, owner: UnitId) -> f64 {
    let autos = &sim.unit(owner).auto_attacks;
    let mut ranged = sim.stats(owner)[Stat::RangedCritPercent];
    if let Some(spell) = autos.ranged_spell {
        ranged += sim.spell(spell).bonus_crit_percent;
    }
    let melee = autos
        .mh_spell
        .map_or(0.0, |spell| sim.spell(spell).bonus_crit_percent);
    ranged.max(melee)
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
    // Go `Hunter.petCritOffset`, the offset the pet currently inherits.
    let offset = sim.new_state_slot(0.0);

    let forever = sim.rules == Rules::Forever;
    let mut base_stats = Stats::default();
    base_stats[Stat::Strength] = 136.0;
    base_stats[Stat::Agility] = 100.0;
    base_stats[Stat::Stamina] = 274.0;
    base_stats[Stat::Intellect] = 50.0;
    base_stats[Stat::Spirit] = 80.0;
    // Apparently pets and warriors have an AP penalty.
    base_stats[Stat::AttackPower] = -20.0;
    if forever {
        // The creature's own attack power, with nothing from Strength (docs/mechanics.md).
        base_stats[Stat::Strength] = 0.0;
        base_stats[Stat::AttackPower] = tamed_pet_attack_power();
    }
    let pet = sim.new_pet(PetConfig {
        name: family.name.to_string(),
        owner,
        base_stats,
        stat_inheritance: Rc::new(stat_inheritance(owner, offset)),
        enabled_on_start: true,
        is_guardian: false,
        is_dynamic: true,
        // Forever's rules: the pet swings at its owner's melee and attack speed on top of its
        // own. In Forever combat logs it swung every 1.965 s under the owner's 2% Wind Blessed,
        // 1.40 s under Rapid Fire's 40% and 1.08 s with its own Frenzy as well; the owner's
        // ranged haste left it alone.
        has_dynamic_melee_speed_inheritance: forever,
        has_dynamic_cast_speed_inheritance: false,
        has_resource_regen_inheritance: false,
        starts_at_owner_distance: true,
    });
    sim.unit_mut(pet).mob_type = "MobTypeBeast".to_string();

    let tables = crate::data::tables::tables();
    let crit_per_int = tables
        .crit_per_int_max_level
        .get("ClassWarrior")
        .copied()
        .unwrap_or(0.0);
    {
        let sdm = &mut sim.unit_mut(pet).sdm;
        if !forever {
            sdm.add_stat_dependency(Stat::Strength, Stat::AttackPower, 2.0);
        }
        // No Agility crit: the pet's physical crit is its owner's (patch 136).
        sdm.add_stat_dependency(Stat::Intellect, Stat::SpellCritPercent, crit_per_int);
    }
    // Go `trackPetCritOffset`: once the pet is dismissed the core has taken back everything it
    // inherited, offset included, so the next summon starts from nothing.
    sim.pet_data_mut(pet).on_pet_disable = Some(Rc::new(move |sim: &mut Sim, _| {
        sim.set_state(offset, 0.0);
    }));

    // Bestial Discipline buys the pet 10% focus regen a rank (19590 effect 1).
    let focus_multiplier = spell_data()
        .bestial_discipline
        .effect_at(1)
        .multiplier_at(hunter.t("bestial_discipline"));
    sim.enable_focus_bar(pet, focus_multiplier);

    // Classic's 18.17 to 27.66 a second under the reference's rules.
    let (weapon_dps_low, weapon_dps_high) = if forever {
        tamed_pet_weapon_dps()
    } else {
        (18.17, 27.66)
    };
    sim.enable_auto_attacks(
        pet,
        AutoAttackOptions {
            main_hand: Weapon {
                base_damage_min: weapon_dps_low * attack_speed,
                base_damage_max: weapon_dps_high * attack_speed,
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
        PetAbility::FuriousHowl => Some(new_furious_howl(sim, pet)),
        PetAbility::Unknown => None,
    }
}

/// The pet's Bite and Claw damage ranks, lowest first: a levelling hunter's pet, at its level,
/// bites with the rank that level teaches (prepare::levels). Rank 8 at level 60.
static BITE_RANKS: std::sync::LazyLock<crate::prepare::spelldata::Ladder> =
    std::sync::LazyLock::new(|| {
        crate::prepare::spelldata::Ladder::ranked(&[
            17253, 17255, 17256, 17257, 17258, 17259, 17260, 17261,
        ])
    });
static CLAW_RANKS: std::sync::LazyLock<crate::prepare::spelldata::Ladder> =
    std::sync::LazyLock::new(|| {
        crate::prepare::spelldata::Ladder::ranked(&[
            16827, 16828, 16829, 16830, 16831, 16832, 3010, 3009,
        ])
    });

/// Lightning Breath's ranks, lowest first: rank 6 (25012, 86 to 98) at level 60, and below it
/// the rank the pet's level teaches, as Bite and Claw.
static LIGHTNING_BREATH_RANKS: std::sync::LazyLock<crate::prepare::spelldata::Ladder> =
    std::sync::LazyLock::new(|| {
        crate::prepare::spelldata::Ladder::ranked(&[24844, 25008, 25009, 25010, 25011, 25012])
    });

/// Scorpid Poison's ranks, lowest first. Every rank's row ticks for 5 (`effects.rs`), so a
/// levelling pet's rank changes only the spell its poison is listed as.
pub(super) static SCORPID_POISON_RANKS: std::sync::LazyLock<crate::prepare::spelldata::Ladder> =
    std::sync::LazyLock::new(|| {
        crate::prepare::spelldata::Ladder::ranked(&[24640, 24583, 24586, 24587])
    });

/// Whether `id` is a rank of the pet's Bite or Claw.
pub(super) fn is_bite_or_claw(id: i32) -> bool {
    let mut found = false;
    for ladder in [&*BITE_RANKS, &*CLAW_RANKS] {
        ladder.each(|_, rank| found |= rank.id == id);
    }
    found
}

/// Whether `id` is a rank of the pet's Lightning Breath.
pub(super) fn is_lightning_breath(id: i32) -> bool {
    let mut found = false;
    LIGHTNING_BREATH_RANKS.each(|_, rank| found |= rank.id == id);
    found
}

/// Whether `id` is a rank of the pet's Scorpid Poison.
pub(super) fn is_scorpid_poison(id: i32) -> bool {
    let mut found = false;
    SCORPID_POISON_RANKS.each(|_, rank| found |= rank.id == id);
    found
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
            ..ability_base(
                ActionId::spell(BITE_RANKS.highest().id),
                school::PHYSICAL,
                DefenseType::Melee,
            )
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
            ..ability_base(
                ActionId::spell(CLAW_RANKS.highest().id),
                school::PHYSICAL,
                DefenseType::Melee,
            )
        },
    )
}

/// Beta client: every rank lower than Classic's, and no more growth per level. Rank 6 is 86-98;
/// a levelling pet breathes the rank its level teaches.
fn new_lightning_breath(sim: &mut Sim, pet: UnitId) -> SpellId {
    sim.register_spell(
        pet,
        SpellConfig {
            proc_mask: ProcMask::SPELL_DAMAGE,
            max_range: 20.0,
            cost: focus(50),
            cast: pet_cast(None),
            ..ability_base(
                ActionId::spell(LIGHTNING_BREATH_RANKS.highest().id),
                school::NATURE,
                DefenseType::Magic,
            )
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
            ..ability_base(
                ActionId::spell(SCORPID_POISON_RANKS.highest().id),
                school::NATURE,
                DefenseType::Melee,
            )
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
                level: character_level(),
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

/// Furious Howl is the Wolf's (the fork's patch 119): an attack power aura on the party around the
/// pet, which the pet and, within the radius, its owner take. The pet howls when the buff is not up
/// on it. Preparation keeps the two auras and the spell; the radius and the condition only matter
/// in a fight.
fn new_furious_howl(sim: &mut Sim, pet: UnitId) -> SpellId {
    let rank = spell_data().furious_howl_triggered.highest();
    let owner = sim.unit(pet).owner.expect("a pet has an owner");
    for unit in [pet, owner] {
        let aura = sim.get_or_register_aura(
            unit,
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
                level: character_level(),
                ..ParseOptions::default()
            },
        );
    }
    let timer = sim.new_timer(pet);
    sim.register_spell(
        pet,
        SpellConfig {
            action_id: ActionId::spell(rank.id),
            spell_school: rank.spell_school(),
            proc_mask: ProcMask::EMPTY,
            flags: SpellFlag::APL,
            cost: focus(rank.cost() as i32),
            cast: pet_cast(Some(Cooldown {
                timer: Some(timer),
                duration: rank.cooldown(),
            })),
            has_extra_cast_condition: true,
            ..SpellConfig::default()
        },
    )
}

#[allow(dead_code)]
const _NEVER: i64 = NEVER_EXPIRES;

#[cfg(test)]
mod tests {
    use super::*;

    /// The client's level 11, 17 and 30 creature rows, whose dps times 4.2 rounded down are the
    /// 36, 61 and 103 attack power of the pet sheets.
    #[test]
    fn the_own_attack_power_is_four_point_two_a_point_of_creature_dps() {
        for (dps, sheet) in [
            (8.59953212738, 36.0),
            (14.5638589859, 61.0),
            (24.69375419617, 103.0),
        ] {
            let own = PET_OWN_ATTACK_POWER_SHARE * ATTACK_POWER_PER_DPS * dps;
            assert_eq!(own.floor(), sheet, "creature dps {dps}");
        }
        // The level 60 row's 60.45 gives 253.9 attack power.
        assert!((tamed_pet_attack_power() - 253.902_243_8).abs() < 1e-6);
    }

    /// A level 30 Cat's logged swings, 42-55 at 119 attack power and 46-59 at 146, swinging at
    /// 1.2 seconds with Unleashed Fury 5/5, the Cat's 1.1 and happiness 1.25, before armor.
    #[test]
    fn the_weapon_is_the_creature_dps_less_its_attack_power_share() {
        let level_30_dps = 24.69375419617;
        let weapon = level_30_dps - PET_OWN_ATTACK_POWER_SHARE * level_30_dps;
        let (speed, multiplier) = (1.2, 1.1 * 1.25 * 1.15);
        for (attack_power, low, high) in [(119.0, 42.0, 55.0), (146.0, 46.0, 59.0)] {
            let fixed = attack_power / ATTACK_POWER_PER_DPS * speed;
            let swing_low = (weapon * (1.0 - PET_SWING_SPREAD) * speed + fixed) * multiplier;
            let swing_high = (weapon * (1.0 + PET_SWING_SPREAD) * speed + fixed) * multiplier;
            assert_eq!(swing_low.round(), low, "{swing_low}");
            assert_eq!(swing_high.round(), high, "{swing_high}");
        }
        // Level 60: 42.3 a second of weapon, a fifth either side, which with the pet's own 18.1
        // attack power a second is the creature row's 60.45.
        let (low, high) = tamed_pet_weapon_dps();
        let average = (low + high) / 2.0;
        assert!((average - 42.317_040_6).abs() < 1e-6, "{average}");
        assert!((high / average - 1.2).abs() < 1e-12);
        let total = average + tamed_pet_attack_power() / ATTACK_POWER_PER_DPS;
        assert!((total - creature_auto_attack_dps()).abs() < 1e-9);
    }
}
