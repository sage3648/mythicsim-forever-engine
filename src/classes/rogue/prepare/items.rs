//! Go sim/rogue/items.go: the Rogue's item sets and the one item effect it registers.
//!
//! The shared item registry asks this module whether it knows a set bonus or an item: Go's
//! `core.NewItemSet` and `core.NewItemEffect` calls in `init`.

use std::rc::Rc;

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::character::{cooldown_type, MajorCooldown};
use crate::prepare::env::Environment;
use crate::prepare::item_sets::ItemSet;
use crate::prepare::sim::{AuraConfig, AuraId, BuildPhase, Cooldown, Sim, UnitId, SECOND};
use crate::prepare::spell::{
    school, CastConfig, DefenseType, DotConfig, ProcMask, SpellConfig, SpellFlag,
};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{Stat, Stats};

use super::masks;
use super::util::spell_action;
use super::{Rogue, RogueState};

/// The Rogue's `core.NewItemSet` calls. Go keeps only the sets whose items the loaded database
/// holds, which `data/go-tables.json` lists; the others are here for the day those items return.
pub(crate) static ITEM_SETS: &[ItemSet] = &[
    ItemSet {
        id: 577,
        name: "Gladiator's Vestments",
        alternative_name: "",
        bonuses: &[(4, pvp_set_4)],
        required_profession: "",
    },
    ItemSet {
        id: 620,
        name: "Assassination Armor",
        alternative_name: "",
        bonuses: &[(2, nothing), (4, dungeon3_4)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Netherblade",
        alternative_name: "",
        bonuses: &[(2, tier4_2), (4, tier4_4)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Deathmantle",
        alternative_name: "",
        bonuses: &[(2, tier5_2), (4, tier5_4)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Slayer's Armor",
        alternative_name: "",
        bonuses: &[(2, tier6_2), (4, tier6_4)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Nightslayer Armor",
        alternative_name: "",
        bonuses: &[(3, nightslayer_3), (5, nightslayer_5), (8, nightslayer_8)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Bloodfang Armor",
        alternative_name: "",
        bonuses: &[(3, bloodfang_3), (5, nothing), (8, bloodfang_8)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Madcap's Outfit",
        alternative_name: "",
        bonuses: &[(2, madcaps_2), (3, nothing), (5, madcaps_5)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Emblems of Veiled Shadows",
        alternative_name: "",
        bonuses: &[(3, emblems_3)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Darkmantle Armor",
        alternative_name: "",
        bonuses: &[
            (2, darkmantle_2),
            (3, nothing),
            (4, darkmantle_4),
            (5, nothing),
            (6, darkmantle_6),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Deathdealer's Embrace",
        alternative_name: "",
        bonuses: &[(3, nothing), (5, deathdealers_5)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Champion's Guard",
        alternative_name: "",
        bonuses: &[(2, pvp_attack_power), (4, nothing), (6, pvp_stamina)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Lieutenant Commander's Guard",
        alternative_name: "",
        bonuses: &[(2, pvp_attack_power), (4, nothing), (6, pvp_stamina)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Warlord's Vestments",
        alternative_name: "",
        bonuses: &[(6, pvp_attack_power), (4, nothing), (2, pvp_stamina)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Field Marshal's Vestments",
        alternative_name: "",
        bonuses: &[(6, pvp_attack_power), (4, nothing), (2, pvp_stamina)],
        required_profession: "",
    },
];

/// The Rogue's state: Go hands a set bonus the agent.
fn rogue_state(env: &mut Environment) -> Rc<RogueState> {
    env.agent
        .as_any_mut()
        .and_then(|agent| agent.downcast_mut::<Rogue>())
        .map(|rogue| Rc::clone(&rogue.state))
        .expect("a Rogue's item set bonus runs for a Rogue")
}

/// A set bonus Go leaves as an empty function, for an effect it does not model.
fn nothing(_: &mut Environment, _: AuraId) {}

fn flat_cost_mod(env: &mut Environment, aura: AuraId, class_mask: i64, value: i32) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::PowerCostFlat,
            class_mask,
            int_value: value,
            ..SpellModConfig::default()
        },
    );
}

fn attack_power(value: f64) -> Stats {
    Stats::from_pairs(&[(Stat::AttackPower, value), (Stat::RangedAttackPower, value)])
}

/// Gladiator's Vestments, 4 pieces: Go sets the flag after the energy bar is built.
fn pvp_set_4(env: &mut Environment, _: AuraId) {
    rogue_state(env).has_pvp_energy.set(true);
}

/// Assassination Armor, 4 pieces: Eviscerate costs 10 less energy. The 2 piece (Cheap Shot and
/// Kidney Shot grant haste) is not implemented in Go.
fn dungeon3_4(env: &mut Environment, aura: AuraId) {
    flat_cost_mod(env, aura, masks::EVISCERATE, -10);
}

/// Netherblade, 2 pieces: Slice and Dice lasts 3 seconds longer.
fn tier4_2(env: &mut Environment, aura: AuraId) {
    let state = rogue_state(env);
    let gain = Rc::clone(&state);
    env.sim.apply_on_gain(
        aura,
        Rc::new(move |_: &mut Sim, _| {
            gain.slice_and_dice_bonus_duration
                .set(gain.slice_and_dice_bonus_duration.get() + 3 * SECOND);
        }),
    );
    env.sim.apply_on_expire(
        aura,
        Rc::new(move |_: &mut Sim, _| {
            state
                .slice_and_dice_bonus_duration
                .set(state.slice_and_dice_bonus_duration.get() - 3 * SECOND);
        }),
    );
}

/// Netherblade, 4 pieces: finishers have a chance to give a combo point.
fn tier4_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Netherblade Combo Point".to_string(),
            action_id: spell_action(37168),
            proc_chance: 0.15,
            class_spell_mask: masks::FINISHER,
            callback: CallbackMask::ON_APPLY_EFFECTS,
            ..ProcTrigger::default()
        },
    );
}

/// Deathmantle, 2 pieces: Eviscerate does 40 more damage a combo point.
fn tier5_2(env: &mut Environment, _: AuraId) {
    rogue_state(env).deathmantle_bonus.set(40.0);
}

/// Deathmantle, 4 pieces: a melee proc makes the next finisher free.
fn tier5_4(env: &mut Environment, aura: AuraId) {
    let unit = env.player;
    let coup_de_grace = env.sim.get_or_register_aura(
        unit,
        AuraConfig {
            label: "Coup de Grace".to_string(),
            duration: 15 * SECOND,
            action_id: Some(spell_action(37171)),
            events: crate::prepare::sim::EventCallbacks {
                on_apply_effects: true,
                ..Default::default()
            },
            ..AuraConfig::default()
        },
    );
    env.sim.attach_spell_mod(
        coup_de_grace,
        SpellModConfig {
            kind: SpellModType::PowerCostPctAdd,
            class_mask: masks::FINISHER,
            float_value: -1.0,
            ..SpellModConfig::default()
        },
    );
    let dpm = env.sim.new_ppm_manager(unit, 0.5, ProcMask::MELEE);
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Deathmantle Proc Trigger".to_string(),
            // 37171 carries the bit.
            can_proc_from_procs: true,
            proc_mask: ProcMask::MELEE,
            outcome: HitOutcome::LANDED,
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            dpm: Some(Rc::new(dpm)),
            ..ProcTrigger::default()
        },
    );
}

/// Slayer's Armor, 2 pieces: Slice and Dice gives 5% more attack speed.
fn tier6_2(env: &mut Environment, aura: AuraId) {
    let state = rogue_state(env);
    let gain = Rc::clone(&state);
    env.sim.apply_on_gain(
        aura,
        Rc::new(move |_: &mut Sim, _| {
            gain.slice_and_dice_bonus_flat
                .set(gain.slice_and_dice_bonus_flat.get() + 0.05);
        }),
    );
    env.sim.apply_on_expire(
        aura,
        Rc::new(move |_: &mut Sim, _| {
            state
                .slice_and_dice_bonus_flat
                .set(state.slice_and_dice_bonus_flat.get() - 0.05);
        }),
    );
}

/// Slayer's Armor, 4 pieces: 6% more damage from the builders.
fn tier6_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::DamageDoneFlat,
            class_mask: masks::BACKSTAB
                | masks::SINISTER_STRIKE
                | masks::MUTILATE
                | masks::HEMORRHAGE,
            float_value: 0.06,
            ..SpellModConfig::default()
        },
    );
}

/// Nightslayer Armor, 3 pieces: Vanish's cooldown is 30 seconds shorter.
fn nightslayer_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::CooldownFlat,
            class_mask: masks::VANISH,
            time_value: -30 * SECOND,
            ..SpellModConfig::default()
        },
    );
}

/// Nightslayer Armor, 5 pieces: 10 more maximum energy. Applied at build time, before the energy
/// bar resets, and only when the set is worn at the start.
fn nightslayer_5(env: &mut Environment, aura: AuraId) {
    let unit = env.player;
    if env.sim.unit(unit).energy_bar.enabled && env.sim.aura(aura).build_phase == BuildPhase::GEAR {
        env.sim.unit_mut(unit).energy_bar.max_energy += 10.0;
    }
}

/// Nightslayer Armor, 8 pieces: Vanish heals the rogue, which a fight listens to.
fn nightslayer_8(env: &mut Environment, aura: AuraId) {
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Clean Escape".to_string(),
            callback: CallbackMask::ON_CAST_COMPLETE,
            class_spell_mask: masks::VANISH,
            ..ProcTrigger::default()
        },
    );
}

/// Bloodfang Armor, 3 pieces: 5% more chance to apply poisons.
fn bloodfang_3(env: &mut Environment, aura: AuraId) {
    let state = rogue_state(env);
    let gain = Rc::clone(&state);
    env.sim.apply_on_gain(
        aura,
        Rc::new(move |_: &mut Sim, _| {
            gain.additive_poison_bonus_chance
                .set(gain.additive_poison_bonus_chance.get() + 0.05);
        }),
    );
    let expire = Rc::clone(&state);
    env.sim.apply_on_expire(
        aura,
        Rc::new(move |_: &mut Sim, _| {
            expire
                .additive_poison_bonus_chance
                .set(expire.additive_poison_bonus_chance.get() - 0.05);
        }),
    );
    if env.sim.aura(aura).active {
        state
            .additive_poison_bonus_chance
            .set(state.additive_poison_bonus_chance.get() + 0.05);
    }
}

/// Bloodfang Armor, 8 pieces: a melee proc hits for 283 to 317 and heals the rogue over 6
/// seconds.
fn bloodfang_8(env: &mut Environment, aura: AuraId) {
    let unit = env.player;
    let proc_config = |id: i32| SpellConfig {
        action_id: spell_action(id),
        spell_school: school::PHYSICAL,
        defense_type: DefenseType::Melee,
        proc_mask: ProcMask::EMPTY,
        flags: SpellFlag::NO_ON_CAST_COMPLETE | SpellFlag::PASSIVE_SPELL,
        damage_multiplier: 1.0,
        ..SpellConfig::default()
    };
    env.sim.get_or_register_spell(
        unit,
        SpellConfig {
            hot: DotConfig {
                aura: AuraConfig {
                    label: "Bloodfang".to_string(),
                    ..AuraConfig::default()
                },
                number_of_ticks: 6,
                tick_length: SECOND,
                ..DotConfig::default()
            },
            ..proc_config(23580)
        },
    );
    env.sim.get_or_register_spell(
        unit,
        SpellConfig {
            threat_multiplier: 1.0,
            ..proc_config(23581)
        },
    );
    let dpm = env.sim.new_ppm_manager(unit, 1.0, ProcMask::MELEE);
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Bloodfang".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            is_weapon_proc: true,
            dpm: Some(Rc::new(dpm)),
            ..ProcTrigger::default()
        },
    );
}

/// Madcap's Outfit, 2 pieces: +20 attack power.
fn madcaps_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(aura, attack_power(20.0));
}

/// Madcap's Outfit, 5 pieces: Eviscerate and Rupture cost 5 less energy.
fn madcaps_5(env: &mut Environment, aura: AuraId) {
    flat_cost_mod(env, aura, masks::EVISCERATE | masks::RUPTURE, -5);
}

/// Emblems of Veiled Shadows, 3 pieces: Slice and Dice costs 10 less energy.
fn emblems_3(env: &mut Environment, aura: AuraId) {
    flat_cost_mod(env, aura, masks::SLICE_AND_DICE, -10);
}

/// Darkmantle Armor, 2 pieces: +8 to every resistance.
fn darkmantle_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        Stats::from_pairs(&[
            (Stat::ArcaneResistance, 8.0),
            (Stat::FireResistance, 8.0),
            (Stat::FrostResistance, 8.0),
            (Stat::NatureResistance, 8.0),
            (Stat::ShadowResistance, 8.0),
        ]),
    );
}

/// Darkmantle Armor, 4 pieces: a white hit has a chance to restore 20 energy.
fn darkmantle_4(env: &mut Environment, aura: AuraId) {
    let unit = env.player;
    let dpm = env
        .sim
        .new_ppm_manager(unit, 1.0, ProcMask::MELEE_WHITE_HIT);
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            action_id: spell_action(27787),
            name: "Rogue Armor Energize".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            dpm: Some(Rc::new(dpm)),
            ..ProcTrigger::default()
        },
    );
}

/// Darkmantle Armor, 6 pieces: +40 attack power.
fn darkmantle_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(aura, attack_power(40.0));
}

/// Deathdealer's Embrace, 5 pieces: 15% more damage to Eviscerate.
fn deathdealers_5(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::DamageDoneFlat,
            class_mask: masks::EVISCERATE,
            float_value: 0.15,
            ..SpellModConfig::default()
        },
    );
}

/// The PvP sets' attack power bonus: +40 attack power.
fn pvp_attack_power(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(aura, attack_power(40.0));
}

/// The PvP sets' stamina bonus: +20 stamina.
fn pvp_stamina(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::Stamina, 20.0);
}

/// Go `core.NewItemEffect` in sim/rogue's `init`: Renataki's Charm of Trickery
/// (https://www.wowhead.com/forever/item=19954/renatakis-charm-of-trickery). Use: instantly
/// increases your energy by 60 (24532), 3 minute cooldown, 10 seconds on the burst trinket
/// category. Answers whether the item is one it registers.
pub(super) fn apply_item_effect(sim: &mut Sim, unit: UnitId, item: i32) -> bool {
    if item != 19954 {
        return false;
    }
    let timer = sim.new_timer(unit);
    let shared = sim.get_offensive_trinket_cd(unit);
    let spell = sim.register_spell(
        unit,
        SpellConfig {
            action_id: crate::contracts::prepared_v2::ActionId {
                item_id: 19954,
                ..Default::default()
            },
            proc_mask: ProcMask::EMPTY,
            flags: SpellFlag::NO_ON_CAST_COMPLETE,
            cast: CastConfig {
                cd: Cooldown {
                    timer: Some(timer),
                    duration: 180 * SECOND,
                },
                shared_cd: Cooldown {
                    timer: Some(shared),
                    duration: 10 * SECOND,
                },
                ..CastConfig::default()
            },
            ..SpellConfig::default()
        },
    );
    sim.add_major_cooldown(
        unit,
        MajorCooldown {
            spell,
            priority: 0,
            cooldown_type: cooldown_type::DPS,
            allow_spell_queueing: false,
            timings: Vec::new(),
        },
    );
    true
}
