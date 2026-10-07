//! The Paladin's talents: Go `talents_holy.go`, `talents_protection.go` and
//! `talents_retribution.go`. The abilities a talent point buys register in
//! `talent_spells.rs`.

mod holy;
mod protection;
mod retribution;

use std::rc::Rc;

use crate::prepare::sim::{AuraId, Duration, Sim, UnitId, MILLISECOND};
use crate::prepare::spell::school;
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};

/// `time.Duration(value) * time.Millisecond` for a client value in milliseconds.
pub(super) fn millis(value: f64) -> Duration {
    (value as i64).wrapping_mul(MILLISECOND)
}

/// Go `applyWeaponSpecialization`'s mod: a physical damage bonus that follows the weapon in
/// the main hand, active when the weapon is of the hand type. Item swaps are refused, so the
/// swap callback Go registers has nothing to react to.
pub(super) fn apply_weapon_specialization(
    sim: &mut Sim,
    unit: UnitId,
    bonus: f64,
    main_hand_matches: bool,
) {
    let weapon_mod = sim.add_dynamic_mod(
        unit,
        SpellModConfig {
            school: school::PHYSICAL,
            kind: SpellModType::DamageDonePct,
            float_value: bonus,
            ..SpellModConfig::default()
        },
    );
    if main_hand_matches {
        sim.activate_spell_mod(weapon_mod);
    }
}

/// Go `Unit.NewPassiveMovementSpeedAura`: a permanent aura whose movement speed bonus holds
/// while no stronger passive speed effect shares the category.
pub(super) fn new_passive_movement_speed_aura(
    sim: &mut Sim,
    unit: UnitId,
    label: &str,
    action_id: crate::contracts::prepared_v2::ActionId,
    multiplier: f64,
) -> AuraId {
    let aura = sim.get_or_register_aura(
        unit,
        crate::prepare::sim::AuraConfig {
            label: label.to_string(),
            action_id: Some(action_id),
            ..Default::default()
        },
    );
    sim.make_permanent(aura);
    sim.new_exclusive_effect(
        aura,
        "PassiveMovementSpeed",
        true,
        multiplier,
        Some(Rc::new(move |sim: &mut Sim, effect| {
            let unit = sim.aura(sim.effects[effect.0].aura).unit;
            sim.multiply_movement_speed(unit, 1.0 + multiplier);
        })),
        Some(Rc::new(move |sim: &mut Sim, effect| {
            let unit = sim.aura(sim.effects[effect.0].aura).unit;
            sim.multiply_movement_speed(unit, 1.0 / (1.0 + multiplier));
        })),
    );
    aura
}
