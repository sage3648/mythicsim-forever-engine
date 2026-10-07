//! Go `sim/paladin/item_sets.go`: the Paladin's item sets, Go's `core.NewItemSet` calls.
//!
//! The Justice Battlegear 4-piece bonus is left out: it adds attack power against Undead to the
//! attack tables, which the exporter reports as unrepresented, so a request that reaches it is
//! refused.

use crate::prepare::aura_helpers::{CallbackMask, ProcTrigger};
use crate::prepare::character::constants::{
    DEFENSE_RATING_PER_DEFENSE_LEVEL, PHYSICAL_CRIT_RATING_PER_CRIT_PERCENT,
    SPELL_CRIT_RATING_PER_CRIT_PERCENT,
};
use crate::prepare::env::Environment;
use crate::prepare::item_sets::ItemSet;
use crate::prepare::sim::{AuraId, MILLISECOND, SECOND};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{Stat, Stats};

use super::masks;
use super::Paladin;

pub(crate) static ITEM_SETS: &[ItemSet] = &[
    ItemSet {
        id: 2106,
        name: "Justice Battlegear",
        alternative_name: "",
        bonuses: &[(2, justice_battlegear_2), (5, justice_battlegear_5)],
        required_profession: "",
    },
    ItemSet {
        id: 2107,
        name: "Justice Armor",
        alternative_name: "",
        bonuses: &[
            (2, justice_armor_2),
            (4, justice_armor_4),
            (5, justice_armor_5),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 2108,
        name: "Justice Battleplate",
        alternative_name: "",
        bonuses: &[
            (2, justice_battleplate_2),
            (4, justice_battleplate_4),
            (5, justice_battleplate_5),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Lawbringer Armor",
        alternative_name: "",
        bonuses: &[(5, lawbringer_armor_5)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Freethinker's Armor",
        alternative_name: "",
        bonuses: &[(2, freethinkers_armor_2)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Avenger's Battlegear",
        alternative_name: "",
        bonuses: &[(5, avengers_battlegear_5)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Battlegear of Eternal Justice",
        alternative_name: "",
        bonuses: &[(3, battlegear_of_eternal_justice_3)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Lieutenant Commander's Redoubt",
        alternative_name: "",
        bonuses: &[
            (2, lieutenant_commanders_redoubt_2),
            (6, lieutenant_commanders_redoubt_6),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Field Marshal's Aegis",
        alternative_name: "",
        bonuses: &[(2, field_marshals_aegis_2), (6, field_marshals_aegis_6)],
        required_profession: "",
    },
];

/// Tier 1 Retribution: increases your attack speed and casting speed by 1%.
fn justice_battlegear_2(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_multiply_attack_speed(aura, 1.01);
    sim.attach_multiply_cast_speed(aura, 1.01);
    sim.expose_to_apl(aura, 1300951);
}

/// Tier 1 Retribution: reduces the cooldown on your Judgement spell by 0.5 sec.
fn justice_battlegear_5(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            class_mask: masks::JUDGEMENT,
            kind: SpellModType::CooldownFlat,
            time_value: -500 * MILLISECOND,
            ..SpellModConfig::default()
        },
    );
    sim.expose_to_apl(aura, 1301702);
}

/// Tier 1 Holy: increased Spirit +10.
fn justice_armor_2(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_stat_buff(aura, Stat::Spirit, 10.0);
    sim.expose_to_apl(aura, 1300948);
}

/// Tier 1 Holy: increases healing done by up to 26 and damage done by up to 9.
fn justice_armor_4(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_stats_buff(
        aura,
        Stats::from_pairs(&[(Stat::HealingPower, 26.0), (Stat::SpellDamage, 9.0)]),
    );
    sim.expose_to_apl(aura, 1301080);
}

/// Tier 1 Holy: reduces the cooldown on your Holy Shock spell by 1 sec.
fn justice_armor_5(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            class_mask: masks::HOLY_SHOCK | masks::HOLY_SHOCK_HEAL,
            kind: SpellModType::CooldownFlat,
            time_value: -SECOND,
            ..SpellModConfig::default()
        },
    );
    sim.expose_to_apl(aura, 1301694);
}

/// Tier 1 Protection: increased Defense +7.
fn justice_battleplate_2(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_stat_buff(
        aura,
        Stat::DefenseRating,
        7.0 * DEFENSE_RATING_PER_DEFENSE_LEVEL,
    );
    sim.expose_to_apl(aura, 1300949);
}

/// Tier 1 Protection: reduces the chance for your melee attacks to be Dodged or Parried by
/// 1.2%: 12 expertise rating.
fn justice_battleplate_4(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_stat_buff(aura, Stat::ExpertiseRating, 12.0);
    sim.expose_to_apl(aura, 1301083);
}

/// Tier 1 Protection: reduces the duration of Forbearance any time you gain it by 10 sec.
fn justice_battleplate_5(env: &mut Environment, aura: AuraId) {
    if let Some(paladin) = env
        .agent
        .as_any_mut()
        .and_then(|agent| agent.downcast_mut::<Paladin>())
    {
        paladin.forbearance_reduction += 10 * SECOND;
    }
    env.sim.expose_to_apl(aura, 1301697);
}

/// Improves your chance to get a critical strike with spells by 1%, and with melee by 1%.
fn lawbringer_armor_5(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        Stats::from_pairs(&[
            (Stat::MeleeCritRating, PHYSICAL_CRIT_RATING_PER_CRIT_PERCENT),
            (Stat::SpellCritRating, SPELL_CRIT_RATING_PER_CRIT_PERCENT),
        ]),
    );
}

/// Restores 4 mana per 5 sec.
fn freethinkers_armor_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::MP5, 4.0);
}

/// Increases damage and healing done by magical spells and effects by up to 71.
fn avengers_battlegear_5(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        Stats::from_pairs(&[(Stat::SpellDamage, 71.0), (Stat::HealingPower, 71.0)]),
    );
}

/// 20% chance to regain 100 mana when you cast a Judgement.
fn battlegear_of_eternal_justice_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Battlegear of Eternal Justice - 3PC".to_string(),
            class_spell_mask: masks::JUDGEMENT_OF_COMMAND | masks::JUDGEMENT_OF_RIGHTEOUSNESS,
            callback: CallbackMask::ON_CAST_COMPLETE,
            proc_chance: 0.2,
            ..ProcTrigger::default()
        },
    );
}

/// Increases damage and healing done by magical spells and effects by up to 23.
fn lieutenant_commanders_redoubt_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        Stats::from_pairs(&[(Stat::SpellDamage, 23.0), (Stat::HealingPower, 23.0)]),
    );
}

/// +20 Stamina.
fn lieutenant_commanders_redoubt_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::Stamina, 20.0);
}

/// +20 Stamina.
fn field_marshals_aegis_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::Stamina, 20.0);
}

/// Increases healing done by up to 44 and damage done by up to 15 for all magical spells and
/// effects.
fn field_marshals_aegis_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        Stats::from_pairs(&[(Stat::HealingPower, 44.0), (Stat::SpellDamage, 15.0)]),
    );
}
