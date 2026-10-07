//! Go sim/priest/items.go: the Priest's item sets.
//!
//! The shared item registry asks this module whether it knows a set bonus: Go's
//! `core.NewItemSet` calls in `init`.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, ProcTrigger};
use crate::prepare::env::Environment;
use crate::prepare::item_sets::ItemSet;
use crate::prepare::sim::{AuraConfig, AuraId, EventCallbacks, SECOND};
use crate::prepare::spell::ProcMask;
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{Stat, Stats};

use super::masks;

/// Incarnate Regalia (664), Avatar Regalia (666) and Absolution Regalia (674): Go's
/// `core.NewItemSet` calls.
pub(crate) static ITEM_SETS: &[ItemSet] = &[
    ItemSet {
        id: 664,
        name: "Incarnate Regalia",
        alternative_name: "",
        bonuses: &[(2, incarnate_regalia_2), (4, incarnate_regalia_4)],
        required_profession: "",
    },
    ItemSet {
        id: 666,
        name: "Avatar Regalia",
        alternative_name: "",
        bonuses: &[(2, avatar_regalia_2), (4, avatar_regalia_4)],
        required_profession: "",
    },
    ItemSet {
        id: 674,
        name: "Absolution Regalia",
        alternative_name: "",
        bonuses: &[(2, absolution_regalia_2), (4, absolution_regalia_4)],
        required_profession: "",
    },
];

/// Your Shadowfiend now has 75 more stamina and lasts 3 sec. longer.
fn incarnate_regalia_2(env: &mut Environment, aura: AuraId) {
    let owner = env.player;
    let pet = env.sim.unit(owner).pets.first().copied();
    let sim = &mut env.sim;
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::BuffDurationFlat,
            time_value: SECOND * 3,
            class_mask: masks::SHADOWFIEND,
            ..SpellModConfig::default()
        },
    );
    sim.apply_on_gain(
        aura,
        Rc::new(move |sim, _| {
            if let Some(pet) = pet {
                sim.add_stat_dynamic(pet, Stat::Stamina, 75.0);
            }
        }),
    );
    sim.apply_on_expire(
        aura,
        Rc::new(move |sim, _| {
            if let Some(pet) = pet {
                sim.add_stat_dynamic(pet, Stat::Stamina, -75.0);
            }
        }),
    );
}

/// Your Mind Flay and Smite spells deal 5% more damage.
fn incarnate_regalia_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::DamageDoneFlat,
            float_value: 0.05,
            class_mask: masks::MIND_FLAY | masks::SMITE,
            ..SpellModConfig::default()
        },
    );
}

/// Each time you cast an offensive spell, there is a chance your next spell will cost 150
/// less mana. 6% proc rate. The discount is consumed on the next spell cast.
fn avatar_regalia_2(env: &mut Environment, aura: AuraId) {
    let unit = env.player;
    let sim = &mut env.sim;
    let discount = sim.register_aura(
        unit,
        AuraConfig {
            label: "Avatar Regalia 2pc Discount".to_string(),
            action_id: Some(ActionId::spell(37601)),
            duration: SECOND * 15,
            events: EventCallbacks {
                on_cast_complete: true,
                ..EventCallbacks::default()
            },
            ..AuraConfig::default()
        },
    );
    sim.attach_spell_mod(
        discount,
        SpellModConfig {
            kind: SpellModType::PowerCostFlat,
            int_value: -150,
            ..SpellModConfig::default()
        },
    );
    sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Avatar Regalia 2pc".to_string(),
            proc_chance: 0.06,
            proc_mask: ProcMask::SPELL_DAMAGE,
            callback: CallbackMask::ON_CAST_COMPLETE,
            ..ProcTrigger::default()
        },
    );
}

/// Each time your Shadow Word: Pain deals damage, it has a chance to grant your next spell
/// cast within 15 sec up to 100 damage and healing. 40% proc rate. The buff is consumed on
/// the next spell cast.
fn avatar_regalia_4(env: &mut Environment, aura: AuraId) {
    let unit = env.player;
    let sim = &mut env.sim;
    let buff = sim.register_aura(
        unit,
        AuraConfig {
            label: "Avatar Regalia 4pc".to_string(),
            action_id: Some(ActionId::spell(37604)),
            duration: SECOND * 15,
            events: EventCallbacks {
                on_cast_complete: true,
                ..EventCallbacks::default()
            },
            ..AuraConfig::default()
        },
    );
    sim.attach_stats_buff(
        buff,
        Stats::from_pairs(&[(Stat::SpellDamage, 100.0), (Stat::HealingPower, 100.0)]),
    );
    // Trigger the buff from Shadow Word: Pain's periodic ticks.
    sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Avatar Regalia 4pc Trigger".to_string(),
            proc_chance: 0.40,
            class_spell_mask: masks::SHADOW_WORD_PAIN,
            callback: CallbackMask::ON_PERIODIC_DAMAGE_DEALT,
            ..ProcTrigger::default()
        },
    );
}

/// Increases the duration of your Shadow Word: Pain ability by 3 sec. SWP ticks every 3 sec,
/// so +3 sec is one more tick.
fn absolution_regalia_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::DotNumberOfTicksFlat,
            int_value: 1,
            class_mask: masks::SHADOW_WORD_PAIN,
            ..SpellModConfig::default()
        },
    );
}

/// Increases the damage from your Mind Blast ability by 10%.
fn absolution_regalia_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::DamageDoneFlat,
            float_value: 0.10,
            class_mask: masks::MIND_BLAST,
            ..SpellModConfig::default()
        },
    );
}
