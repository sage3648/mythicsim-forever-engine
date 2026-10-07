//! Go sim/common/classic/emerald_dragon_whelp.go: the Emerald Dragon Whelp Dragon's Call
//! summons. The client has no creature data, so its stats, melee and spit rate are master's
//! guesses from Classic logs.
//!
//! The whelp is a guardian a gear pet constructor adds right after the class agent is built
//! (Go `RegisterGearPetConstructor`). Preparation leaves it disabled at the reset; what summons
//! it is the weapon proc `classic_weapons.rs` registers, and the exporter describes both.

use std::rc::Rc;

use super::character::constants::{
    PHYSICAL_CRIT_RATING_PER_CRIT_PERCENT, SPELL_CRIT_RATING_PER_CRIT_PERCENT,
};
use crate::contracts::prepared_v2::ActionId;

use super::attack::{AutoAttackOptions, Weapon};
use super::classic_weapons::DRAGONS_CALL;
use super::items::slot;
use super::pet::PetConfig;
use super::sim::{Sim, UnitId, SECOND};
use super::spell::{school, Cast, CastConfig, CostOptions, DefenseType, ProcMask, SpellConfig};
use super::spell::{SpellFlag, GCD_DEFAULT};
use super::stats::{Stat, Stats};

/// The whelp's name, which the exporter recognizes as a guardian an effect summons.
pub(crate) const WHELP_NAME: &str = "Emerald Dragon Whelp";

/// Go `core.RegisterGearPetConstructor`'s function of this package: only a Dragon's Call
/// equipped in a hand at the start gets a whelp.
pub(crate) fn construct_gear_pets(sim: &mut Sim, player: UnitId) {
    let equipment = &sim.character(player).equipment;
    if equipment[slot::MAIN_HAND].id == DRAGONS_CALL || equipment[slot::OFF_HAND].id == DRAGONS_CALL
    {
        new_emerald_dragon_whelp(sim, player);
    }
}

/// Whether the player has the whelp: Go's search of `character.PetAgents`.
pub(crate) fn has_whelp(sim: &Sim, player: UnitId) -> bool {
    sim.unit(player)
        .pets
        .iter()
        .any(|pet| sim.pet_data(*pet).name == WHELP_NAME)
}

/// Go `newEmeraldDragonWhelp`.
fn new_emerald_dragon_whelp(sim: &mut Sim, owner: UnitId) {
    let mut base_stats = Stats::default();
    base_stats[Stat::Health] = 1500.0;
    base_stats[Stat::Intellect] = 20.0;
    base_stats[Stat::Mana] = 500.0;
    // Master tuned 220 on its flat 374 to hit the log (~594 a spit); 438.5 + 155 keeps that.
    base_stats[Stat::SpellDamage] = 155.0;
    base_stats[Stat::MeleeCritRating] = 4.5 * PHYSICAL_CRIT_RATING_PER_CRIT_PERCENT;
    base_stats[Stat::SpellCritRating] = 13.0 * SPELL_CRIT_RATING_PER_CRIT_PERCENT;
    let whelp = sim.new_pet(PetConfig {
        name: WHELP_NAME.to_string(),
        owner,
        base_stats,
        stat_inheritance: Rc::new(|_| Stats::default()),
        enabled_on_start: false,
        is_guardian: true,
        is_dynamic: false,
        has_dynamic_melee_speed_inheritance: false,
        has_dynamic_cast_speed_inheritance: false,
        has_resource_regen_inheritance: false,
        starts_at_owner_distance: false,
    });
    sim.unit_mut(whelp).level = 55;

    sim.enable_pet_mana_bar(whelp);
    sim.enable_auto_attacks(
        whelp,
        AutoAttackOptions {
            main_hand: Weapon {
                base_damage_min: 80.0,
                base_damage_max: 100.0,
                swing_speed: 2.0,
                spell_school: school::PHYSICAL,
                ..Weapon::default()
            },
            auto_swing_melee: true,
            ..AutoAttackOptions::default()
        },
    );
    sim.pet_data_mut(whelp).on_initialize = Some(Rc::new(initialize));
    sim.add_pet(owner, whelp);
}

/// Go `EmeraldDragonWhelp.Initialize`: Acid Spit (9591), 438.5 +-29.3%, so 374 to 503 Nature.
fn initialize(sim: &mut Sim, whelp: UnitId) {
    sim.register_spell(
        whelp,
        SpellConfig {
            action_id: ActionId::spell(9591),
            spell_school: school::NATURE,
            defense_type: DefenseType::Magic,
            proc_mask: ProcMask::SPELL_DAMAGE,
            flags: SpellFlag::IGNORE_MODIFIERS,
            cost: CostOptions {
                mana_flat_cost: 90,
                ..CostOptions::default()
            },
            cast: CastConfig {
                default_cast: Cast {
                    gcd: GCD_DEFAULT,
                    cast_time: 3 * SECOND,
                    ..Cast::default()
                },
                ..CastConfig::default()
            },
            damage_multiplier: 1.0,
            threat_multiplier: 1.0,
            bonus_coefficient: 1.0,
            ..SpellConfig::default()
        },
    );
}
