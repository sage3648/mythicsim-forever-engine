//! Go sim/warrior/items.go: the Warrior's item sets. A bonus function receives the permanent
//! `"<set> <n>P"` status aura the bonus attaches to.

use std::rc::Rc;

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger, PseudoStatField};
use crate::prepare::character::constants::PARRY_RATING_PER_PARRY_PERCENT;
use crate::prepare::client_set_effects::HUMANOID;
use crate::prepare::env::Environment;
use crate::prepare::item_sets::{ApplySetBonus, ClientItemSet, ItemSet};
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
    let has_bs_t2 = warrior(env).has_bs_t2;
    let from_options = env.sim.state_flag(has_bs_t2);
    env.sim.apply_on_gain(
        aura,
        Rc::new(move |sim: &mut Sim, _| sim.set_state_flag(has_bs_t2, true)),
    );
    env.sim.apply_on_expire(
        aura,
        Rc::new(move |sim: &mut Sim, _| sim.set_state_flag(has_bs_t2, from_options)),
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
    let bonus = warrior(env).thunder_clap_effect_bonus;
    let sim = &mut env.sim;
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            float_value: 0.5,
            ..mod_for(masks::THUNDER_CLAP, SpellModType::DamageDoneFlat)
        },
    );
    // AttachAdditivePseudoStatBuff(&warrior.thunderClapEffectBonus, 0.5)
    sim.apply_on_gain(
        aura,
        Rc::new(move |sim: &mut Sim, _| sim.set_state(bonus, sim.state(bonus) + 0.5)),
    );
    sim.apply_on_expire(
        aura,
        Rc::new(move |sim: &mut Sim, _| sim.set_state(bonus, sim.state(bonus) - 0.5)),
    );
    if sim.aura(aura).active {
        sim.set_state(bonus, sim.state(bonus) + 0.5);
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
    // Go `NewLegacyPPMManager`, which an item swap rebuilds for the weapons in the hands.
    let dpm = Rc::new(
        env.sim
            .new_ppm_manager(unit, 1.0, ProcMask::MELEE)
            .rebuilt_by_swap(
                &env.sim,
                unit,
                crate::prepare::item_swap::ManagerChance::Ppm(1.0),
                crate::prepare::item_swap::ManagerMask::Static(ProcMask::MELEE),
            ),
    );
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

/// Battlegear of Glory (ItemSet 2102: Legplates 273944, Gauntlets 273945, Shoulders 273946,
/// Greaves 273947) and Battleplate of Glory (ItemSet 2103: Legguards 273948, Handguards 273949,
/// Pauldrons 273950, Sabatons 273951), the Warrior's Tier 1 of the Forever client, which the
/// reference does not register. The bonuses are the ItemSetSpell spells of `data/spells.jsonl`
/// (client 1.60.1.70425), beside the line the Wowhead Forever tooltips gave them on 13 November
/// 2026.
///
/// Battlegear of Glory (Arms and Fury):
///
/// - (2) "Increases your attack speed and casting speed by 1%." Spell 1300968: a melee and ranged
///   haste aura (342) and a casting speed aura (65) of 1.
/// - (3) "Reduces the cooldown of your Intimidating Shout ability by 30 sec." Spell 1301069: a
///   cooldown mod (107, misc 11) of -30000 ms on mask 262144 of family 4, which is Intimidating
///   Shout (5246).
/// - (4) "+36 Attack Power against Humanoids." Spell 1301095: melee attack power versus a
///   creature type (102, mask 64) of 36.
/// - (5) "Reduces the cooldown of your Recklessness ability by 30 sec." Spell 1301721: a cooldown
///   mod (107, misc 11) of -30000 ms on mask 16 of family 4, which is Recklessness (1719).
///
/// Battleplate of Glory (Protection):
///
/// - (2) "Increased Defense +7." Spell 1300970: a skill aura (30) of 7 on Defense (skill 95). The
///   row's family is 7 (the Druid's) and its tooltip reads "Requires Druid", though the spell is
///   named Tier 1 Warrior Protection; the effect has no class mask, so the family changes
///   nothing here.
/// - (3) "Reduces the cooldown of your Intervene ability by 10 sec." Spell 1301071: a cooldown
///   mod (107, misc 11) of -10000 ms on mask [0, 65536] of family 4. No spell of the client data
///   carries that mask and nothing here registers Intervene (a spell on another player), so the
///   bonus is the status aura alone.
/// - (4) "Reduces the chance for your melee attacks to be Dodged or Parried by 1.2%." Spell
///   1301096: a rating aura (189) of 12 on Expertise (mask 8388608), as Justice Battleplate's.
/// - (5) "Reduces the cooldown of your Shield Wall ability by 30 sec." Spell 1301722: a cooldown
///   mod (107, misc 11) of -30000 ms on mask 8192 of family 4, which is Shield Wall (871).
pub(crate) static CLIENT_ITEM_SETS: &[ClientItemSet] = &[
    ClientItemSet {
        set: ItemSet {
            id: 2102,
            name: "Battlegear of Glory",
            alternative_name: "",
            bonuses: &[
                (2, battlegear_of_glory_2),
                (3, battlegear_of_glory_3),
                (4, battlegear_of_glory_4),
                (5, battlegear_of_glory_5),
            ],
            required_profession: "",
        },
        not_prepared: &[],
    },
    ClientItemSet {
        set: ItemSet {
            id: 2103,
            name: "Battleplate of Glory",
            alternative_name: "",
            bonuses: &[
                (2, battleplate_of_glory_2),
                (3, battleplate_of_glory_3),
                (4, battleplate_of_glory_4),
                (5, battleplate_of_glory_5),
            ],
            required_profession: "",
        },
        not_prepared: &[],
    },
];

/// A cooldown mod of the given seconds on the class spells a mask names.
fn cooldown_mod(class_mask: i64, seconds: i64) -> SpellModConfig {
    SpellModConfig {
        time_value: seconds * SECOND,
        ..mod_for(class_mask, SpellModType::CooldownFlat)
    }
}

/// Tier 1 Arms and Fury: increases your attack speed and casting speed by 1%.
fn battlegear_of_glory_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_set_haste(aura);
    env.sim.expose_to_apl(aura, 1300968);
}

/// Tier 1 Arms and Fury: reduces the cooldown of your Intimidating Shout ability by 30 sec.
fn battlegear_of_glory_3(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_spell_mod(aura, cooldown_mod(masks::INTIMIDATING_SHOUT, -30));
    sim.expose_to_apl(aura, 1301069);
}

/// Tier 1 Arms and Fury: +36 Attack Power against Humanoids.
fn battlegear_of_glory_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_versus_attack_power(aura, HUMANOID, 36.0);
    env.sim.expose_to_apl(aura, 1301095);
}

/// Tier 1 Arms and Fury: reduces the cooldown of your Recklessness ability by 30 sec.
fn battlegear_of_glory_5(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_spell_mod(aura, cooldown_mod(masks::RECKLESSNESS, -30));
    sim.expose_to_apl(aura, 1301721);
}

/// Tier 1 Protection: increased Defense +7.
fn battleplate_of_glory_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_set_defense(aura, 7.0);
    env.sim.expose_to_apl(aura, 1300970);
}

/// Tier 1 Protection: reduces the cooldown of your Intervene ability by 10 sec. Intervene is not
/// a spell this engine registers, so the bonus is the status aura alone.
fn battleplate_of_glory_3(env: &mut Environment, aura: AuraId) {
    env.sim.expose_to_apl(aura, 1301071);
}

/// Tier 1 Protection: reduces the chance for your melee attacks to be Dodged or Parried by 1.2%.
fn battleplate_of_glory_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_set_expertise(aura);
    env.sim.expose_to_apl(aura, 1301096);
}

/// Tier 1 Protection: reduces the cooldown of your Shield Wall ability by 30 sec.
fn battleplate_of_glory_5(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_spell_mod(aura, cooldown_mod(masks::SHIELD_WALL, -30));
    sim.expose_to_apl(aura, 1301722);
}
