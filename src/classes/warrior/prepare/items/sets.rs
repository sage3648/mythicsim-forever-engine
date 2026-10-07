//! Go sim/warrior/items.go: the Warrior's item sets. A bonus function receives the permanent
//! "<set> <n>P" status aura the bonus attaches to.

use std::cell::Cell;
use std::rc::Rc;

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger, PseudoStatField};
use crate::prepare::character::constants::PARRY_RATING_PER_PARRY_PERCENT;
use crate::prepare::env::Environment;
use crate::prepare::item_sets::{ApplySetBonus, ItemSet};
use crate::prepare::sim::{AuraConfig, AuraId, Sim, NEVER_EXPIRES, SECOND};
use crate::prepare::spell::ProcMask;
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{Stat, Stats};

use super::super::helpers::spell_action;
use super::super::masks;
use super::super::Warrior;

/// The Warrior behind an environment's agent: Go's `agent.(WarriorAgent).GetWarrior()`.
fn warrior(env: &mut Environment) -> &mut Warrior {
    env.agent
        .as_any_mut()
        .and_then(|agent| agent.downcast_mut::<Warrior>())
        .expect("a Warrior set bonus applies to a Warrior")
}

/// A spell mod on the class spells a mask names; the caller states its value.
fn mod_for(class_mask: i64, kind: SpellModType) -> SpellModConfig {
    SpellModConfig {
        class_mask,
        kind,
        ..SpellModConfig::default()
    }
}

fn battlegear_of_might_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::BlockValue, 30.0);
}

fn battlegear_of_might_5(env: &mut Environment, aura: AuraId) {
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Battlegear of Might - 5PC".to_string(),
            action_id: spell_action(21838),
            callback: CallbackMask::ON_SPELL_HIT_TAKEN | CallbackMask::ON_PERIODIC_DAMAGE_TAKEN,
            outcome: HitOutcome::LANDED,
            require_damage_dealt: true,
            proc_chance: 0.2,
            ..ProcTrigger::default()
        },
    );
}

fn battlegear_of_might_8(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            float_value: 0.15,
            ..mod_for(masks::SUNDER_ARMOR, SpellModType::FlatThreatBonusPct)
        },
    );
}

/// Spell 23563 states 30 attack power on Battle Shout, which battle_shout.go adds through the
/// flag the HasBsT2 option sets. The set aura toggles it.
fn battlegear_of_wrath_3(env: &mut Environment, aura: AuraId) {
    let has_bs_t2 = warrior(env).has_bs_t2.clone();
    let from_options = has_bs_t2.get();
    let on_gain = has_bs_t2.clone();
    env.sim
        .apply_on_gain(aura, Rc::new(move |_: &mut Sim, _| on_gain.set(true)));
    env.sim.apply_on_expire(
        aura,
        Rc::new(move |_: &mut Sim, _| has_bs_t2.set(from_options)),
    );
}

fn battlegear_of_wrath_5(env: &mut Environment, set_aura: AuraId) {
    let unit = env.player;
    let sim = &mut env.sim;
    let buff = sim.register_aura(
        unit,
        AuraConfig {
            label: "Warrior's Wrath".to_string(),
            action_id: Some(spell_action(21887)),
            duration: 10 * SECOND,
            ..AuraConfig::default()
        },
    );
    sim.attach_spell_mod(
        buff,
        SpellModConfig {
            int_value: -5,
            ..mod_for(masks::OFFENSIVE_ABILITIES, SpellModType::PowerCostFlat)
        },
    );
    sim.attach_proc_trigger(
        buff,
        &ProcTrigger {
            name: "Warrior's Wrath - Consume".to_string(),
            class_spell_mask: masks::OFFENSIVE_ABILITIES,
            callback: CallbackMask::ON_CAST_COMPLETE,
            trigger_immediately: true,
            ..ProcTrigger::default()
        },
    );
    sim.attach_proc_trigger(
        set_aura,
        &ProcTrigger {
            name: "Battlegear of Wrath - 5PC".to_string(),
            action_id: spell_action(21890),
            class_spell_mask: masks::OFFENSIVE_ABILITIES,
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            proc_chance: 0.2,
            ..ProcTrigger::default()
        },
    );
}

fn battlegear_of_wrath_8(env: &mut Environment, set_aura: AuraId) {
    let unit = env.player;
    let sim = &mut env.sim;
    let parry = sim.register_aura(
        unit,
        AuraConfig {
            label: "Battlegear of Wrath Parry".to_string(),
            action_id: Some(spell_action(23547)),
            duration: NEVER_EXPIRES,
            ..AuraConfig::default()
        },
    );
    sim.attach_stat_buff(
        parry,
        Stat::ParryRating,
        100.0 * PARRY_RATING_PER_PARRY_PERCENT,
    );
    sim.attach_proc_trigger(
        parry,
        &ProcTrigger {
            name: "Battlegear of Wrath - 8PC Consume".to_string(),
            proc_mask: ProcMask::MELEE,
            callback: CallbackMask::ON_SPELL_HIT_TAKEN,
            trigger_immediately: true,
            ..ProcTrigger::default()
        },
    );
    sim.attach_proc_trigger(
        set_aura,
        &ProcTrigger {
            name: "Battlegear of Wrath - 8PC".to_string(),
            action_id: spell_action(23548),
            callback: CallbackMask::ON_SPELL_HIT_TAKEN,
            outcome: HitOutcome::BLOCK,
            proc_chance: 0.04,
            ..ProcTrigger::default()
        },
    );
}

fn conquerors_battlegear_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            float_value: -0.35,
            ..mod_for(masks::SHOUTS, SpellModType::PowerCostPctAdd)
        },
    );
}

fn conquerors_battlegear_5(env: &mut Environment, aura: AuraId) {
    let bonus = warrior(env).thunder_clap_effect_bonus.clone();
    let sim = &mut env.sim;
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            float_value: 0.5,
            ..mod_for(masks::THUNDER_CLAP, SpellModType::DamageDoneFlat)
        },
    );
    // AttachAdditivePseudoStatBuff(&warrior.thunderClapEffectBonus, 0.5)
    let on_gain: Rc<Cell<f64>> = bonus.clone();
    sim.apply_on_gain(
        aura,
        Rc::new(move |_: &mut Sim, _| on_gain.set(on_gain.get() + 0.5)),
    );
    let on_expire = bonus.clone();
    sim.apply_on_expire(
        aura,
        Rc::new(move |_: &mut Sim, _| on_expire.set(on_expire.get() - 0.5)),
    );
    if sim.aura(aura).active {
        bonus.set(bonus.get() + 0.5);
    }
}

/// Spell 28844 states 75 Revenge damage.
fn dreadnaughts_battlegear_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            float_value: 75.0,
            ..mod_for(masks::REVENGE, SpellModType::BaseDamageFlat)
        },
    );
}

/// Spell 28843 states 5% chance to hit with Taunt and Challenging Shout.
fn dreadnaughts_battlegear_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            float_value: 5.0,
            ..mod_for(
                masks::TAUNT | masks::CHALLENGING_SHOUT,
                SpellModType::BonusHitPercent,
            )
        },
    );
}

/// Spell 28842 states 5% chance to hit with Sunder Armor, Heroic Strike, Revenge and Shield
/// Slam.
fn dreadnaughts_battlegear_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            float_value: 5.0,
            ..mod_for(
                masks::SUNDER_ARMOR | masks::HEROIC_STRIKE | masks::REVENGE | masks::SHIELD_SLAM,
                SpellModType::BonusHitPercent,
            )
        },
    );
}

/// Spell 28845: below 20% health, healing cast on the warrior gains 160 healing for 5 seconds.
fn dreadnaughts_battlegear_8(env: &mut Environment, set_aura: AuraId) {
    let unit = env.player;
    let sim = &mut env.sim;
    let cheat_death = sim.register_aura(
        unit,
        AuraConfig {
            label: "Cheat Death".to_string(),
            action_id: Some(spell_action(28846)),
            duration: 5 * SECOND,
            ..AuraConfig::default()
        },
    );
    sim.attach_additive_pseudo_stat_buff(cheat_death, PseudoStatField::BonusHealingTaken, 160.0);
    sim.attach_proc_trigger(
        set_aura,
        &ProcTrigger {
            name: "Cheat Death - Trigger".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_TAKEN | CallbackMask::ON_PERIODIC_DAMAGE_TAKEN,
            outcome: HitOutcome::LANDED,
            ..ProcTrigger::default()
        },
    );
}

/// Increases your chance to block attacks with a shield by 2%.
fn vindicators_battlegear_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::BlockPercent, 2.0);
}

/// Decreases the cooldown of Intimidating Shout by 15 sec.
fn vindicators_battlegear_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            time_value: -15 * SECOND,
            ..mod_for(masks::INTIMIDATING_SHOUT, SpellModType::CooldownFlat)
        },
    );
}

/// Decreases the rage cost of Whirlwind by 3.
fn vindicators_battlegear_5(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            int_value: -3,
            ..mod_for(masks::WHIRLWIND, SpellModType::PowerCostFlat)
        },
    );
}

/// +8 All Resistances.
fn battlegear_of_heroism_2(env: &mut Environment, aura: AuraId) {
    let mut stats = Stats::default();
    for stat in [
        Stat::ArcaneResistance,
        Stat::FireResistance,
        Stat::FrostResistance,
        Stat::NatureResistance,
        Stat::ShadowResistance,
    ] {
        stats[stat] = 8.0;
    }
    env.sim.attach_stats_buff(aura, stats);
}

/// Chance on melee attack to heal for 88 to 132, and in Forever to return 10 Rage.
fn battlegear_of_heroism_4(env: &mut Environment, aura: AuraId) {
    let unit = env.player;
    let dpm = Rc::new(env.sim.new_ppm_manager(unit, 1.0, ProcMask::MELEE));
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Warrior's Resolve".to_string(),
            action_id: spell_action(450587),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            proc_mask: ProcMask::MELEE,
            dpm: Some(dpm),
            ..ProcTrigger::default()
        },
    );
}

/// +20 Strength, where Classic gave +40 attack power.
fn battlegear_of_heroism_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::Strength, 20.0);
}

fn pvp_attack_power(env: &mut Environment, aura: AuraId) {
    let mut stats = Stats::default();
    stats[Stat::AttackPower] = 40.0;
    stats[Stat::RangedAttackPower] = 40.0;
    env.sim.attach_stats_buff(aura, stats);
}

/// Reduces the cooldown of your Intercept ability by 5 sec.
fn pvp_intercept(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            time_value: -5 * SECOND,
            ..mod_for(masks::INTERCEPT, SpellModType::CooldownFlat)
        },
    );
}

fn pvp_stamina(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::Stamina, 20.0);
}

const fn pvp(name: &'static str, bonuses: &'static [(i32, ApplySetBonus)]) -> ItemSet {
    ItemSet {
        id: 0,
        name,
        alternative_name: "",
        bonuses,
        required_profession: "",
    }
}

/// The four PvP sets share one set of bonuses, in two orders: attack power at 2 and stamina at
/// 6, or the other way round.
static PVP_ATTACK_POWER_FIRST: &[(i32, ApplySetBonus)] =
    &[(2, pvp_attack_power), (4, pvp_intercept), (6, pvp_stamina)];
static PVP_STAMINA_FIRST: &[(i32, ApplySetBonus)] =
    &[(6, pvp_attack_power), (4, pvp_intercept), (2, pvp_stamina)];

/// Go's `core.NewItemSet` calls in sim/warrior/items.go, in Go's order.
pub(crate) static ITEM_SETS: &[ItemSet] = &[
    ItemSet {
        id: 209,
        name: "Battlegear of Might",
        alternative_name: "",
        bonuses: &[
            (3, battlegear_of_might_3),
            (5, battlegear_of_might_5),
            (8, battlegear_of_might_8),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 218,
        name: "Battlegear of Wrath",
        alternative_name: "",
        bonuses: &[
            (3, battlegear_of_wrath_3),
            (5, battlegear_of_wrath_5),
            (8, battlegear_of_wrath_8),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 496,
        name: "Conqueror's Battlegear",
        alternative_name: "",
        bonuses: &[(3, conquerors_battlegear_3), (5, conquerors_battlegear_5)],
        required_profession: "",
    },
    ItemSet {
        id: 523,
        name: "Dreadnaught's Battlegear",
        alternative_name: "",
        bonuses: &[
            (2, dreadnaughts_battlegear_2),
            (4, dreadnaughts_battlegear_4),
            (6, dreadnaughts_battlegear_6),
            (8, dreadnaughts_battlegear_8),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 474,
        name: "Vindicator's Battlegear",
        alternative_name: "",
        bonuses: &[
            (2, vindicators_battlegear_2),
            (3, vindicators_battlegear_3),
            (5, vindicators_battlegear_5),
        ],
        required_profession: "",
    },
    // Both of the client's ids for the set carry this name, so it is matched by name.
    pvp(
        "Battlegear of Heroism",
        &[
            (2, battlegear_of_heroism_2),
            (4, battlegear_of_heroism_4),
            (6, battlegear_of_heroism_6),
        ],
    ),
    pvp("Champion's Battlegear", PVP_ATTACK_POWER_FIRST),
    pvp("Lieutenant Commander's Battlegear", PVP_ATTACK_POWER_FIRST),
    pvp("Warlord's Battlegear", PVP_STAMINA_FIRST),
    pvp("Field Marshal's Battlegear", PVP_STAMINA_FIRST),
    pvp("Champion's Battlearmor", PVP_ATTACK_POWER_FIRST),
    pvp("Lieutenant Commander's Battlearmor", PVP_ATTACK_POWER_FIRST),
];
