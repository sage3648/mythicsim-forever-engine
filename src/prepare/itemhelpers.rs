//! Go sim/common/itemhelpers/weaponprocs.go: weapon proc helpers. Every helper rolls the proc on
//! the weapon's own PPM manager, which follows the item through item swaps (preparation refuses
//! item swapping), and registers the trigger aura.
//!
//! Go registers each helper's effect with `core.NewItemEffect`; here a helper applies its effect
//! to the player, and the caller's registry decides which item it applies for.

use super::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use super::env::Environment;
use super::shared_items::dynamic_legacy_proc_for_weapon;
use super::shared_procs::{apply_proc_damage_effect, ProcDamageEffect, TriggerDpmFn};
use super::sim::{AuraId, Duration, SpellId};
use super::spell::{DefenseType, SpellFlag};
use std::rc::Rc;

/// Go `WeaponProcTrigger`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct WeaponProcTrigger {
    pub item_id: i32,
    pub name: &'static str,
    pub ppm: f64,
    /// Set for an "Equip" proc, which is an aura the game matches by proc flags and which never
    /// hears proc hits. Left unset it means a "Chance on hit" proc: a weapon proc that rolls on
    /// every hit of its weapon except those suppressing weapon procs.
    pub equip_proc: bool,
    pub trigger_immediately: bool,
    /// A cooldown between procs, for a proc whose spell the client puts on a cooldown. Zero means
    /// none.
    pub icd: Duration,
}

/// Go `CreateWeaponProcTrigger`: a weapon proc whose handler runs on every landed hit that
/// passes the weapon's PPM roll. `setup` is the call of the config's `Handler`, which runs once
/// per character and may opt it out by answering false.
pub(crate) fn create_weapon_proc_trigger(
    env: &mut Environment,
    config: &WeaponProcTrigger,
    setup: impl FnOnce(&mut Environment) -> bool,
) -> Option<AuraId> {
    if !setup(env) {
        return None;
    }
    let unit = env.player;
    let dpm = dynamic_legacy_proc_for_weapon(&env.sim, unit, config.item_id, config.ppm, 0.0);
    Some(env.sim.make_proc_trigger_aura(
        unit,
        &ProcTrigger {
            name: format!("{} Proc", config.name),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            dpm: Some(dpm),
            icd: config.icd,
            is_weapon_proc: !config.equip_proc,
            trigger_immediately: config.trigger_immediately,
            ..ProcTrigger::default()
        },
    ))
}

/// Go `WeaponProcDamage`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct WeaponProcDamage {
    pub item_id: i32,
    pub name: &'static str,
    pub ppm: f64,
    pub spell_id: i32,
    pub school: u8,
    /// From SpellCategories. Picks the hit table and crit multiplier.
    pub defense_type: DefenseType,
    pub min_dmg: f64,
    pub max_dmg: f64,
    pub bonus_coefficient: f64,
    /// See `WeaponProcTrigger`: `create_weapon_equip_proc_damage` sets it.
    pub equip_proc: bool,
}

/// Go `CreateWeaponProcDamage`: a weapon proc that deals flat damage.
pub(crate) fn create_weapon_proc_damage(env: &mut Environment, config: &WeaponProcDamage) {
    let item_id = config.item_id;
    let ppm = config.ppm;
    let trigger_dpm: TriggerDpmFn =
        Rc::new(move |sim, unit| dynamic_legacy_proc_for_weapon(sim, unit, item_id, ppm, 0.0));
    apply_proc_damage_effect(
        env,
        &ProcDamageEffect {
            item_id: config.item_id,
            spell_id: config.spell_id,
            school: config.school,
            defense_type: config.defense_type,
            min_dmg: config.min_dmg,
            max_dmg: config.max_dmg,
            bonus_coefficient: config.bonus_coefficient,
            flags: SpellFlag::NO_ON_CAST_COMPLETE | SpellFlag::PASSIVE_SPELL | SpellFlag::PROC,
            trigger: ProcTrigger {
                name: format!("{} Proc", config.name),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                outcome: HitOutcome::LANDED,
                is_weapon_proc: !config.equip_proc,
                ..ProcTrigger::default()
            },
            trigger_dpm: Some(trigger_dpm),
            ..ProcDamageEffect::default()
        },
    );
}

/// Go `CreateWeaponCoHProcDamage`: a "Chance on hit" weapon damage proc.
#[allow(dead_code)]
pub(crate) fn create_weapon_coh_proc_damage(env: &mut Environment, config: &WeaponProcDamage) {
    create_weapon_proc_damage(
        env,
        &WeaponProcDamage {
            equip_proc: false,
            ..*config
        },
    );
}

/// Go `CreateWeaponEquipProcDamage`: an "Equip" weapon damage proc.
#[allow(dead_code)]
pub(crate) fn create_weapon_equip_proc_damage(env: &mut Environment, config: &WeaponProcDamage) {
    create_weapon_proc_damage(
        env,
        &WeaponProcDamage {
            equip_proc: true,
            ..*config
        },
    );
}

/// Go `CreateWeaponProcSpell`: a "Chance on hit" weapon proc that casts a custom spell on the
/// target that was hit. `spell` runs once per character and answers the spell, or none to opt
/// the character out (a resource proc on a class without that resource).
pub(crate) fn create_weapon_proc_spell(
    env: &mut Environment,
    item_id: i32,
    name: &'static str,
    ppm: f64,
    spell: impl FnOnce(&mut Environment) -> Option<SpellId>,
) {
    create_weapon_proc_trigger(
        env,
        &WeaponProcTrigger {
            item_id,
            name,
            ppm,
            trigger_immediately: true,
            ..WeaponProcTrigger::default()
        },
        |env| {
            let Some(proc_spell) = spell(env) else {
                return false;
            };
            env.sim.spell_mut(proc_spell).flags |=
                SpellFlag::NO_ON_CAST_COMPLETE | SpellFlag::PASSIVE_SPELL | SpellFlag::PROC;
            true
        },
    );
}

/// Go `AddWeaponProcAura`: adds a "Chance on hit" weapon proc for a custom aura to an existing
/// item effect. `aura` runs first and answers the aura the proc activates on the wearer.
pub(crate) fn add_weapon_proc_aura(
    env: &mut Environment,
    item_id: i32,
    name: &'static str,
    ppm: f64,
    aura: impl FnOnce(&mut Environment) -> AuraId,
) {
    let unit = env.player;
    aura(env);
    let dpm = dynamic_legacy_proc_for_weapon(&env.sim, unit, item_id, ppm, 0.0);
    env.sim.make_proc_trigger_aura(
        unit,
        &ProcTrigger {
            name: format!("{name} Proc"),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            dpm: Some(dpm),
            is_weapon_proc: true,
            ..ProcTrigger::default()
        },
    );
}
