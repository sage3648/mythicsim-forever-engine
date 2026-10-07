//! Go sim/core/buffs/flametongue_totem.go: Flametongue Totem, as client 1.60.1.70170 states it.
//!
//! Rank 4 (16387) summons the totem, and the totem's party aura (15036, an area
//! A_PROC_TRIGGER_SPELL) states 100% on a landed melee auto attack and triggers 16389,
//! "Flametongue Totem Proc". The proc has one effect, a dummy of 1363 that does not scale with
//! level: hundredths of damage per second of weapon speed, with the speed held to 1.3 to 4.0.
//! Neither dummy deals damage, so the engine treats the totem's hit as the imbue's Magic fire
//! spell, which takes none of the caster's spell power.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;

use super::super::resolve_proc::proc_trigger;
use super::super::sim::{AuraConfig, AuraId, EffectId, Sim, SpellId, UnitId, NEVER_EXPIRES};
use super::super::spell::{school, DefenseType, ProcMask, SpellConfig, SpellFlag};
use super::super::spelldata::must_find;
use super::generated::FLAMETONGUE_TOTEM;

/// The party's Flametongue Totem row: the aura that states the trigger.
const FLAMETONGUE_TOTEM_PARTY: i32 = 15036;
/// "Flametongue Totem Proc".
const FLAMETONGUE_TOTEM_PROC: i32 = 16389;

/// Go `FlametongueAttackTraits`: what a class's own Flametongue Attack carries that its talents
/// and threat modifiers key on. The shaman sets its own (the class mask Flametongue Weapon's hit
/// has, so Elemental Fury and Elemental Weapons reach the totem's hit as they reach the imbue's,
/// and its spell flag). Any other class has none, and its totem hit takes no talent.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct FlametongueAttackTraits {
    pub class_spell_mask: i64,
    pub flags: SpellFlag,
}

/// Go `FlametongueTotemTriggerLabel`: names the aura that listens for the main-hand hits. A
/// character has one, whichever way the totem reaches it, since a second Flametongue Totem does
/// not add a second hit.
pub(crate) const FLAMETONGUE_TOTEM_TRIGGER_LABEL: &str = "Flametongue Totem Trigger";

/// Go `flametongueTotemValue`: the proc's dummy value at the character's level, 1363, flat, since
/// the row has no per-level gain.
fn flametongue_totem_value() -> f64 {
    must_find(FLAMETONGUE_TOTEM_PROC)
        .effect_n(1)
        .average(super::super::character::constants::CHARACTER_LEVEL)
}

/// Go `FlametongueTotemBaseDamage`: what a hit adds under a main-hand weapon of this swing
/// speed. The value is hundredths of damage per second of speed, held to 1.3 to 4.0 as the
/// tooltip's "(X / 77 - 1) to (X / 25)" are.
#[allow(dead_code)]
pub(crate) fn flametongue_totem_base_damage(weapon_speed: f64) -> f64 {
    weapon_speed.clamp(1.3, 4.0) * flametongue_totem_value() / 100.0
}

/// Go `FlametongueTotemPriority`: what a Flametongue Totem bids for the personal benefit.
pub(crate) fn flametongue_totem_priority() -> f64 {
    flametongue_totem_value()
}

/// Go `DisableFlametongueTotem`: makes `aura` switch a character's own Flametongue Totem benefit
/// off while it is up. It is a main-hand Flametongue Weapon: "When applied to main hand, disables
/// any benefit you personally receive from Flametongue Totem". It bids twice what the totem
/// does, so the totem's effect never becomes the active one while the imbue stands.
#[allow(dead_code)]
pub(crate) fn disable_flametongue_totem(sim: &mut Sim, aura: AuraId) {
    sim.new_exclusive_effect(
        aura,
        FLAMETONGUE_TOTEM.category,
        false,
        2.0 * flametongue_totem_priority(),
        None,
        None,
    );
}

/// Go `FlametongueTotemAttack`: the damage a main-hand auto attack adds under the totem, the
/// imbue's spell with the totem's base damage: Magic fire, so it rolls the spell hit and crit
/// tables, with no spell power coefficient. It keeps the totem's own id (16389) so a report lists
/// it apart from the imbue.
pub(crate) fn flametongue_totem_attack(
    sim: &mut Sim,
    unit: UnitId,
    traits: FlametongueAttackTraits,
) -> SpellId {
    sim.get_or_register_spell(
        unit,
        SpellConfig {
            action_id: ActionId::spell(FLAMETONGUE_TOTEM_PROC),
            spell_school: school::FIRE,
            defense_type: DefenseType::Magic,
            proc_mask: ProcMask::SPELL_DAMAGE_PROC,
            class_spell_mask: traits.class_spell_mask,
            flags: SpellFlag::PASSIVE_SPELL | SpellFlag::PROC | traits.flags,
            damage_multiplier: 1.0,
            threat_multiplier: 1.0,
            ..SpellConfig::default()
        },
    )
}

/// Go `FlametongueTotemTrigger`: the aura that casts the hit off the character's main-hand auto
/// attacks. It is not on by itself: the totem aura that holds the personal benefit turns it on
/// (`join_flametongue_totem`), so that a main-hand Flametongue Weapon can turn it off. The row's
/// own proc flags say what it hears, a landed melee auto attack, narrowed to the main hand as the
/// tooltip says: an off-hand swing does not add the damage.
pub(crate) fn flametongue_totem_trigger(
    sim: &mut Sim,
    unit: UnitId,
    traits: FlametongueAttackTraits,
) -> AuraId {
    if let Some(aura) = sim.get_aura(unit, FLAMETONGUE_TOTEM_TRIGGER_LABEL) {
        return aura;
    }
    flametongue_totem_attack(sim, unit, traits);
    let mut trigger = proc_trigger(sim, Some(unit), must_find(FLAMETONGUE_TOTEM_PARTY), &[]);
    trigger.name = FLAMETONGUE_TOTEM_TRIGGER_LABEL.to_string();
    trigger.action_id = ActionId::default();
    trigger.proc_mask = ProcMask(trigger.proc_mask.0 & ProcMask::MELEE_MH.0);
    trigger.duration = NEVER_EXPIRES;
    trigger.trigger_immediately = true;
    sim.make_proc_trigger_aura(unit, &trigger)
}

/// Go `JoinFlametongueTotem`: makes `aura` a Flametongue Totem the character benefits from: while
/// it is up and no main-hand Flametongue Weapon disables it, the trigger is on. The party's totem
/// and the shaman's own cast share the category and the trigger, so a totem from both sources
/// adds one hit.
pub(crate) fn join_flametongue_totem(
    sim: &mut Sim,
    unit: UnitId,
    aura: AuraId,
    traits: FlametongueAttackTraits,
) {
    let trigger = flametongue_totem_trigger(sim, unit, traits);
    sim.new_exclusive_effect(
        aura,
        FLAMETONGUE_TOTEM.category,
        false,
        flametongue_totem_priority(),
        Some(Rc::new(move |sim: &mut Sim, _: EffectId| {
            sim.activate(trigger);
        })),
        Some(Rc::new(move |sim: &mut Sim, _: EffectId| {
            sim.deactivate(trigger);
        })),
    );
}

/// Go `driveFlametongueTotem`: a Flametongue Totem another shaman keeps down for the party. It is
/// a fire totem, so it sits in no air slot and does not interact with the party's Windfury Totem
/// or Grace of Air; the same character's own cast Flametongue Totem and the party's one are the
/// same effect and the category keeps one of them.
pub(crate) fn drive_flametongue_totem(
    sim: &mut Sim,
    unit: UnitId,
    traits: FlametongueAttackTraits,
) {
    // The trigger is registered before the permanent aura that switches it on, so that it is
    // reset first.
    flametongue_totem_trigger(sim, unit, traits);
    let aura = sim.get_or_register_aura(
        unit,
        AuraConfig {
            label: "Flametongue Totem".to_string(),
            action_id: Some(ActionId {
                spell_id: 16387,
                tag: -1,
                ..ActionId::default()
            }),
            duration: NEVER_EXPIRES,
            ..AuraConfig::default()
        },
    );
    join_flametongue_totem(sim, unit, aura, traits);
    sim.make_permanent(aura);
}
