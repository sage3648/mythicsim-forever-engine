//! Under Forever's rules: equip effects of the live client that restore the wearer's mana when a
//! spell of theirs lands, which the reference leaves unregistered (docs/forever-chance-on-hit.md
//! lists them as "restores mana or rage").
//!
//! - Energetic Rod (18321), Demonic Energy (1302190, SpellAuraOptions 246697): "Chance on
//!   landing a damaging spell to restore 48 mana to you." The row states ProcChance 100, no proc
//!   recovery and no procs per minute, and casts 1302191, an Energize of 48 on mana. The owner's
//!   decision of 13 November 2026 reads such rows literally (`forever_tank_procs`): a ProcChance
//!   of 100 is the rate, gated only by the row's own recovery, here none. Every landed hit of a
//!   damaging spell restores 48 mana, a spell batch window later.
//!
//! The row's ProcTypeMask is 65536, harmful spells dealt, which alone decodes to the cast as the
//! trigger (`decode_proc_type_mask`), as Farraki Kris's "Chance on harmful spell cast" is read.
//! The rod's tooltip names the landing of a damaging spell instead, so the listener hears landed
//! hits of spells with a damaging component (`export_items::deals_damage`): a miss or a full
//! resist restores nothing, and neither does a spell that deals no damage. A ground area spell
//! the runtime runs as an area damage over time (Consecration) is left out: its cast lands on no
//! unit, and its ticks are periodic damage (Forever's Consecration ticks, 1280345 to 1280349,
//! carry the treat as periodic attribute), which the mask does not name.
//!
//! A wearer with no mana bar gains nothing, and wears the rod as a plain mace.

use serde_json::{json, Value};

use super::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use super::common_effects::SPELL_BATCH_WINDOW;
use super::consumable_effects::proc_trigger_spells;
use super::env::Environment;
use super::export_items::{aura_named, deals_damage};
use super::resolve_proc::{chance, proc_trigger};
use super::shared_items::EffectSource;
use super::spelldata::must_find;

/// Energetic Rod.
pub(crate) const ENERGETIC_ROD: i32 = 18_321;

/// Demonic Energy, the equip aura, and the Energize it casts.
const DEMONIC_ENERGY: i32 = 1_302_190;
const DEMONIC_ENERGY_MANA: i32 = 1_302_191;

/// The label the listener goes by: the item's name.
const ENERGETIC_ROD_LABEL: &str = "Energetic Rod";

/// The item effects of this module, applied under Forever's rules.
pub(crate) fn has_item_effect(item: i32) -> bool {
    item == ENERGETIC_ROD
}

/// Applies the equip effect of one of [`has_item_effect`]'s items, answering whether it is one.
pub(crate) fn apply_item(env: &mut Environment, item: i32) -> bool {
    match item {
        ENERGETIC_ROD => energetic_rod(env),
        _ => return false,
    }
    true
}

/// Energetic Rod: the listener on the item that restores mana when a damaging spell lands. A
/// wearer with no mana bar gains nothing, so the rod is a plain item and no listener is
/// registered.
fn energetic_rod(env: &mut Environment) {
    let unit = env.player;
    if !env.sim.unit(unit).mana_bar.enabled {
        return;
    }
    let source = EffectSource {
        id: ENERGETIC_ROD,
        is_enchant: false,
    };
    let mut listener = energetic_rod_listener(env);
    listener.action_id = source.action_id();
    source.register_trigger(env, &listener);
}

/// The rod's listener: Demonic Energy's row, heard on the landing of a spell.
fn energetic_rod_listener(env: &Environment) -> ProcTrigger {
    // The store files the row's 100 beside the tooltip's "chance" as no stated rate; it is the
    // rate.
    let rate = [chance(1.0)];
    let mut listener = proc_trigger(&env.sim, Some(env.player), must_find(DEMONIC_ENERGY), &rate);
    listener.name = ENERGETIC_ROD_LABEL.to_string();
    // The landing of the spell, not its cast.
    listener.callback = CallbackMask::ON_SPELL_HIT_DEALT;
    listener.outcome = HitOutcome::LANDED;
    listener.require_damage_dealt = false;
    listener
}

/// The exported effects of this module's listeners the character wears.
pub(crate) fn export_effects(env: &Environment) -> Vec<Value> {
    let Some(aura) = aura_named(env, ENERGETIC_ROD_LABEL) else {
        return Vec::new();
    };
    let label = env.sim.aura(aura).label.clone();
    let listener = energetic_rod_listener(env);
    let chances: Vec<Value> = proc_trigger_spells(env, &listener)
        .into_iter()
        .filter(|&position| {
            let spell = env.sim.spell(env.sim.unit(env.player).spellbook[position]);
            spell.aoe_dot.is_none() && deals_damage(env, position)
        })
        .map(|spell| json!({"spell": spell, "chance": 1.0}))
        .collect();
    let energize = must_find(DEMONIC_ENERGY_MANA).energize_effect();
    let level = env.sim.unit(env.player).level;
    vec![json!({
        "kind": "mana_proc", "trigger_aura": label, "rng_label": label,
        "chances": chances, "mana": energize.average(level),
        "metrics_action_id": json!({"spell_id": DEMONIC_ENERGY_MANA}),
        "delay_ns": SPELL_BATCH_WINDOW,
    })]
}
