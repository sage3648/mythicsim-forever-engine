//! Go sim/priest/shadowfiend_pet.go: the Shadowfiend pet every priest registers. The summon
//! spell that enables it is in `spells.rs`.

use std::rc::Rc;

use crate::prepare::attack::{AutoAttackOptions, Weapon};
use crate::prepare::aura_helpers::{CallbackMask, ProcTrigger};
use crate::prepare::character::constants::DEFAULT_ATTACK_POWER_PER_DPS;
use crate::prepare::pet::PetConfig;
use crate::prepare::sim::{Sim, UnitId, NEVER_EXPIRES};
use crate::prepare::spell::school;
use crate::prepare::stats::{Stat, Stats};

/// Go `Shadowfiend`'s `baseStats`. The attack power carries a negative base offset: the
/// Strength dependency brings the displayed attack power to 286.
fn base_stats() -> Stats {
    Stats::from_pairs(&[
        (Stat::Strength, 153.0),
        (Stat::Agility, 108.0),
        (Stat::Stamina, 297.0),
        (Stat::Intellect, 175.0),
        (Stat::Spirit, 122.0),
        (Stat::AttackPower, -20.0),
        (Stat::Armor, 5290.0),
    ])
}

/// Go `Priest.shadowfiendStatInheritance`: attack power from the priest's spell damage and
/// shadow damage.
fn stat_inheritance(owner_stats: &Stats) -> Stats {
    let mut inherited = Stats::default();
    inherited[Stat::AttackPower] =
        (owner_stats[Stat::SpellDamage] + owner_stats[Stat::ShadowDamage]) * 0.57;
    inherited
}

/// Go `Priest.NewShadowfiend`.
pub(super) fn new_shadowfiend(sim: &mut Sim, owner: UnitId) -> UnitId {
    let pet = sim.new_pet(PetConfig {
        name: "Shadowfiend".to_string(),
        owner,
        base_stats: base_stats(),
        stat_inheritance: Rc::new(stat_inheritance),
        enabled_on_start: false,
        is_guardian: false,
        is_dynamic: false,
        has_dynamic_melee_speed_inheritance: false,
        has_dynamic_cast_speed_inheritance: false,
        has_resource_regen_inheritance: false,
        starts_at_owner_distance: false,
    });

    // Client 401977: "Caster receives $401988s1% mana when the Shadowfiend attacks"; 401988 is
    // an energize-percent effect of 5, i.e. 5% of the priest's maximum mana per landed attack.
    let mana_restore = sim.make_proc_trigger_aura(
        pet,
        &ProcTrigger {
            name: "Shadowfiend Mana Restore".to_string(),
            duration: NEVER_EXPIRES,
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            ..ProcTrigger::default()
        },
    );

    sim.enable_auto_attacks(
        pet,
        AutoAttackOptions {
            main_hand: Weapon {
                base_damage_min: 68.0,
                base_damage_max: 92.0,
                swing_speed: 1.5,
                normalized_swing_speed: 1.5,
                spell_school: school::SHADOW,
                attack_power_per_dps: DEFAULT_ATTACK_POWER_PER_DPS,
                ..Weapon::default()
            },
            auto_swing_melee: true,
            ..AutoAttackOptions::default()
        },
    );

    // `shadowfiend.AutoAttacks.MHConfig().BonusCoefficient = 1.0` is the config's default.
    sim.add_pet(owner, pet);

    let data = sim.pet_data_mut(pet);
    data.on_pet_enable = Some(Rc::new(move |sim: &mut Sim, _| sim.activate(mana_restore)));
    data.on_pet_disable = Some(Rc::new(move |sim: &mut Sim, _| {
        sim.deactivate(mana_restore)
    }));
    pet
}

/// Go `Shadowfiend.Initialize`.
pub(super) fn initialize(sim: &mut Sim, pet: UnitId) {
    sim.unit_mut(pet)
        .sdm
        .add_stat_dependency(Stat::Strength, Stat::AttackPower, 2.0);
}

/// Go `Shadowfiend.Reset`.
pub(super) fn reset(sim: &mut Sim, pet: UnitId) {
    sim.disable_pet(pet);
}
