//! Go sim/priest/items.go: the Priest's item sets.
//!
//! The shared item registry asks this module whether it knows a set bonus: Go's
//! `core.NewItemSet` calls in `init`.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, ProcTrigger};
use crate::prepare::client_set_effects::UNDEAD;
use crate::prepare::env::Environment;
use crate::prepare::item_sets::{ClientItemSet, ItemSet};
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

/// Raiments of Conviction (ItemSet 2105: Pants 273920, Gloves 273921, Mantle 273922, Treads
/// 273923), the Shadow Priest's Tier 1 of the Forever client, which the reference does not
/// register. The bonuses are the ItemSetSpell spells of `data/spells.jsonl` (client
/// 1.60.1.70425), beside the line the Wowhead Forever tooltips gave them on 13 November 2026.
///
/// - (2) "Improves your chance to hit by 1.0%." Spell 1300955: a melee and a spell hit aura of 1.
/// - (3) "Increases the duration of your Shackle Undead spell by 10 sec." Spell 1301045: a
///   duration mod (107, misc 1) of +10000 ms on Shackle Undead (9484, 9485, 10955). Applied with
///   no effect: nothing registers Shackle Undead for a Priest here (a crowd control spell, which
///   no rotation casts).
/// - (4) "Increases damage done by your spells and effects by up to 21 when fighting Undead."
///   Spell 1301087: spell damage versus a creature type (59, mask 32) of 21.
/// - (5) "Reduces the cooldown of your Devouring Plague spell by 60 sec." Spell 1301706: a
///   cooldown mod (107, misc 11) of -60000 ms on mask [0, 0, 0, 16] of family 6, which is
///   Devouring Plague (2944 to 19278). Not prepared: the 16 October 2026 beta notes cut the
///   cooldown to 24 sec and the row states 60 sec (`register_devouring_plague_spell`), and the
///   bonus was written against the 60 sec one. A flat 60 sec off 24 sec leaves a negative
///   cooldown here, and whether the game clamps it to none, or the notes changed this bonus
///   with the cooldown, is not stated anywhere.
pub(crate) static CLIENT_ITEM_SETS: &[ClientItemSet] = &[ClientItemSet {
    set: ItemSet {
        id: 2105,
        name: "Raiments of Conviction",
        alternative_name: "",
        bonuses: &[
            (2, raiments_of_conviction_2),
            (3, raiments_of_conviction_3),
            (4, raiments_of_conviction_4),
        ],
        required_profession: "",
    },
    not_prepared: &[5],
}];

/// Tier 1 Shadow: improves your chance to hit by 1.0%.
fn raiments_of_conviction_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_set_hit(aura);
    env.sim.expose_to_apl(aura, 1300955);
}

/// Tier 1 Shadow: increases the duration of your Shackle Undead spell by 10 sec. Shackle Undead
/// is not a spell this engine registers, so the bonus is the status aura alone.
fn raiments_of_conviction_3(env: &mut Environment, aura: AuraId) {
    env.sim.expose_to_apl(aura, 1301045);
}

/// Tier 1 Shadow: increases damage done by your spells and effects by up to 21 when fighting
/// Undead.
fn raiments_of_conviction_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_versus_spell_damage(aura, UNDEAD, 21.0);
    env.sim.expose_to_apl(aura, 1301087);
}
