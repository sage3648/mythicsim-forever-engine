//! Go sim/hunter/items.go and item_sets.go: the Hunter's item effects and item sets, and the
//! exporter's description of Renataki's Charm of Beasts (tools/oracle-v2/hunter.go).
//!
//! The shared item registry asks this module whether it knows a set bonus or an item: Go's
//! `core.NewItemSet` and `core.NewItemEffect` calls in `init`.

use std::rc::Rc;

use serde_json::{json, Value};

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger, PseudoStatField};
use crate::prepare::character::{cooldown_type, MajorCooldown};
use crate::prepare::env::Environment;
use crate::prepare::item_sets::ItemSet;
use crate::prepare::items::slot;
use crate::prepare::sim::{
    AuraConfig, AuraId, BuildPhase, Cooldown, Sim, SpellId, UnitId, NEVER_EXPIRES, SECOND,
};
use crate::prepare::spell::{CastConfig, ProcMask, SpellConfig, SpellFlag};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{Stat, Stats};

use super::aspects::SPEED_15_MULTIPLIER;
use super::{masks, Hunter, THORIDAL_THE_STARS_FURY};

const BLACK_BOW_OF_THE_BETRAYER: i32 = 32336;
const ASHTONGUE_TALISMAN_OF_SWIFTNESS: i32 = 32487;
const TALON_OF_ALAR: i32 = 30448;
const RENATAKIS_CHARM_OF_BEASTS: i32 = 19953;

/// The hunter's tier and PvP sets: Go's `core.NewItemSet` calls.
pub(crate) static ITEM_SETS: &[ItemSet] = &[
    ItemSet {
        id: 530,
        name: "Cryptstalker Armor",
        alternative_name: "",
        bonuses: &[
            (2, cryptstalker_2),
            (4, cryptstalker_4),
            (6, cryptstalker_6),
            (8, cryptstalker_8),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Giantstalker Armor",
        alternative_name: "",
        bonuses: &[(5, giantstalker_5), (8, giantstalker_8)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Predator's Armor",
        alternative_name: "",
        bonuses: &[(2, predators_2), (5, predators_5)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Beastmaster Armor",
        alternative_name: "",
        bonuses: &[
            (2, beastmaster_2),
            (3, beastmaster_3),
            (4, beastmaster_4),
            (6, beastmaster_6),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Striker's Garb",
        alternative_name: "",
        bonuses: &[(3, strikers_garb_3), (5, strikers_garb_5)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Lieutenant Commander's Pursuit",
        alternative_name: "",
        bonuses: &[(2, pursuit_agility), (6, pursuit_stamina)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Champion's Pursuit",
        alternative_name: "",
        bonuses: &[(2, pursuit_agility), (6, pursuit_stamina)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Champion's Pursuance",
        alternative_name: "",
        bonuses: &[(2, pursuit_agility), (6, pursuit_stamina)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Lieutenant Commander's Pursuance",
        alternative_name: "",
        bonuses: &[(2, pursuit_agility), (6, pursuit_stamina)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Warlord's Pursuit",
        alternative_name: "",
        bonuses: &[(2, pursuit_stamina), (6, pursuit_agility)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Field Marshal's Pursuit",
        alternative_name: "",
        bonuses: &[(2, pursuit_stamina), (6, pursuit_agility)],
        required_profession: "",
    },
];

fn mod_config(kind: SpellModType, class_mask: i64) -> SpellModConfig {
    SpellModConfig {
        kind,
        class_mask,
        ..SpellModConfig::default()
    }
}

/// The hunter's pet, which a hunter's only pet is.
fn pet_of(env: &Environment) -> Option<UnitId> {
    env.sim.unit(env.player).pets.first().copied()
}

fn cryptstalker_2(env: &mut Environment, aura: AuraId) {
    // Increases the duration of your Rapid Fire by 4 secs.
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            time_value: 4 * SECOND,
            ..mod_config(SpellModType::BuffDurationFlat, masks::RAPID_FIRE)
        },
    );
    env.sim.expose_to_apl(aura, 28755);
}

fn cryptstalker_4(env: &mut Environment, set_bonus_aura: AuraId) {
    // While your pet is active, increases Attack Power by 50 for both you and your pet.
    let Some(pet) = pet_of(env) else { return };
    let unit = env.player;
    let sim = &mut env.sim;
    let mut ap_buff = Stats::default();
    ap_buff[Stat::AttackPower] = 50.0;
    ap_buff[Stat::RangedAttackPower] = 50.0;
    let build_phase = sim.aura(set_bonus_aura).build_phase;
    let owner_aura = sim.register_aura(
        unit,
        AuraConfig {
            label: "Stalker's Ally".to_string(),
            action_id: Some(ActionId::spell(28757)),
            duration: NEVER_EXPIRES,
            build_phase,
            ..AuraConfig::default()
        },
    );
    sim.attach_stats_buff(owner_aura, ap_buff);
    let pet_aura = sim.register_aura(
        pet,
        AuraConfig {
            label: "Stalker's Ally".to_string(),
            action_id: Some(ActionId::spell(28758)),
            duration: NEVER_EXPIRES,
            build_phase,
            ..AuraConfig::default()
        },
    );
    sim.attach_stats_buff(pet_aura, ap_buff);
    if build_phase == BuildPhase::GEAR {
        sim.make_permanent(owner_aura);
        sim.make_permanent(pet_aura);
    } else {
        sim.attach_dependent_aura(set_bonus_aura, owner_aura);
        sim.attach_dependent_aura(set_bonus_aura, pet_aura);
    }
    sim.expose_to_apl(set_bonus_aura, 28756);
}

fn cryptstalker_6(env: &mut Environment, aura: AuraId) {
    // Your ranged critical hits cause an Adrenaline Rush, granting you 50 mana.
    let sim = &mut env.sim;
    sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Adrenaline Rush".to_string(),
            metrics_action_id: ActionId::spell(28752),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::CRIT,
            proc_mask: ProcMask::RANGED,
            ..ProcTrigger::default()
        },
    );
    sim.expose_to_apl(aura, 28752);
}

fn cryptstalker_8(env: &mut Environment, aura: AuraId) {
    // Reduces the mana cost of your Multi-Shot and Aimed Shot by 20.
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            int_value: -20,
            ..mod_config(
                SpellModType::PowerCostFlat,
                masks::MULTI_SHOT | masks::AIMED_SHOT,
            )
        },
    );
    env.sim.expose_to_apl(aura, 28751);
}

/// Go `attachNaturesAlly`: pet stamina and all resistances.
fn natures_ally(
    env: &mut Environment,
    set_bonus_aura: AuraId,
    spell_id: i32,
    stamina: f64,
    resistance: f64,
) {
    let Some(pet) = pet_of(env) else { return };
    let sim = &mut env.sim;
    let build_phase = sim.aura(set_bonus_aura).build_phase;
    let pet_aura = sim.register_aura(
        pet,
        AuraConfig {
            label: "Nature's Ally".to_string(),
            action_id: Some(ActionId::spell(spell_id)),
            duration: NEVER_EXPIRES,
            build_phase,
            ..AuraConfig::default()
        },
    );
    let mut stats = Stats::default();
    stats[Stat::Stamina] = stamina;
    stats[Stat::ArcaneResistance] = resistance;
    stats[Stat::FireResistance] = resistance;
    stats[Stat::FrostResistance] = resistance;
    stats[Stat::NatureResistance] = resistance;
    stats[Stat::ShadowResistance] = resistance;
    sim.attach_stats_buff(pet_aura, stats);
    if build_phase == BuildPhase::GEAR {
        sim.make_permanent(pet_aura);
    } else {
        sim.attach_dependent_aura(set_bonus_aura, pet_aura);
    }
}

fn giantstalker_5(env: &mut Environment, aura: AuraId) {
    // Increases your pet's stamina by 30 and all spell resistances by 40.
    natures_ally(env, aura, 21926, 30.0, 40.0);
}

fn giantstalker_8(env: &mut Environment, aura: AuraId) {
    // Increases the damage of Multi-shot and Volley by 15%.
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            float_value: 0.15,
            ..mod_config(
                SpellModType::DamageDoneFlat,
                masks::MULTI_SHOT | masks::VOLLEY,
            )
        },
    );
}

fn predators_2(env: &mut Environment, aura: AuraId) {
    let mut stats = Stats::default();
    stats[Stat::AttackPower] = 20.0;
    stats[Stat::RangedAttackPower] = 20.0;
    env.sim.attach_stats_buff(aura, stats);
}

fn predators_5(env: &mut Environment, aura: AuraId) {
    // Increases the duration of Serpent Sting by 3 sec, one tick.
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            int_value: 1,
            ..mod_config(SpellModType::DotNumberOfTicksFlat, masks::SERPENT_STING)
        },
    );
}

fn beastmaster_2(env: &mut Environment, aura: AuraId) {
    // +8 All Resistances.
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

fn beastmaster_3(env: &mut Environment, aura: AuraId) {
    // Restores 8 mana per 5 sec.
    env.sim.attach_stat_buff(aura, Stat::MP5, 8.0);
}

fn beastmaster_4(env: &mut Environment, aura: AuraId) {
    // Melee and ranged autoattacks have a 5% chance of restoring 200 mana.
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Hunter Armor Energize".to_string(),
            action_id: ActionId::spell(450577),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            proc_mask: ProcMask::WHITE_HIT,
            proc_chance: 0.05,
            ..ProcTrigger::default()
        },
    );
}

fn beastmaster_6(env: &mut Environment, aura: AuraId) {
    // +40 Attack Power.
    let mut stats = Stats::default();
    stats[Stat::AttackPower] = 40.0;
    stats[Stat::RangedAttackPower] = 40.0;
    env.sim.attach_stats_buff(aura, stats);
}

fn strikers_garb_3(env: &mut Environment, aura: AuraId) {
    // Reduces the cost of your Arcane Shots by 10%.
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            float_value: -0.10,
            ..mod_config(SpellModType::PowerCostPctAdd, masks::ARCANE_SHOT)
        },
    );
}

fn strikers_garb_5(env: &mut Environment, aura: AuraId) {
    // Reduces the cooldown of your Rapid Fire ability by 2 minutes.
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            time_value: -120 * SECOND,
            ..mod_config(SpellModType::CooldownFlat, masks::RAPID_FIRE)
        },
    );
}

/// The PvP sets pair 20 Agility with 20 Stamina.
fn pursuit_agility(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::Agility, 20.0);
}

fn pursuit_stamina(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::Stamina, 20.0);
}

impl Hunter {
    /// The `core.NewItemEffect` calls of sim/hunter: applies the effect of an equipped item and
    /// answers true, or answers false for an item the Hunter registers nothing for.
    pub(super) fn apply_hunter_item_effect(
        &mut self,
        sim: &mut Sim,
        unit: UnitId,
        item: i32,
    ) -> bool {
        match item {
            // Knight-Lieutenant's and Blood Guard's Chain Gauntlets: reduces the mana cost of
            // your Arcane Shot by 15 (23157).
            16403 | 16530 => {
                sim.add_static_mod(
                    unit,
                    SpellModConfig {
                        int_value: -15,
                        ..mod_config(SpellModType::PowerCostFlat, masks::ARCANE_SHOT)
                    },
                );
                true
            }
            RENATAKIS_CHARM_OF_BEASTS => {
                self.register_renatakis_charm(sim, unit);
                true
            }
            THORIDAL_THE_STARS_FURY => {
                self.register_thoridal(sim, unit);
                true
            }
            BLACK_BOW_OF_THE_BETRAYER => {
                sim.make_proc_trigger_aura(
                    unit,
                    &ProcTrigger {
                        name: "Black Bow of the Betrayer".to_string(),
                        metrics_action_id: ActionId::item(46939),
                        spell_flags_exclude: SpellFlag::SUPPRESS_WEAPON_PROCS,
                        callback: CallbackMask::ON_SPELL_HIT_DEALT,
                        outcome: HitOutcome::LANDED,
                        proc_mask: ProcMask::RANGED,
                        ..ProcTrigger::default()
                    },
                );
                true
            }
            ASHTONGUE_TALISMAN_OF_SWIFTNESS => {
                let mut stats = Stats::default();
                stats[Stat::AttackPower] = 275.0;
                stats[Stat::RangedAttackPower] = 275.0;
                sim.new_temporary_stats_aura(
                    unit,
                    "Deadly Aim",
                    &ActionId::spell(40487),
                    stats,
                    8 * SECOND,
                );
                sim.make_proc_trigger_aura(
                    unit,
                    &ProcTrigger {
                        name: "Ashtongue Talisman of Swiftness".to_string(),
                        metrics_action_id: ActionId::spell(40485),
                        callback: CallbackMask::ON_SPELL_HIT_DEALT,
                        class_spell_mask: masks::STEADY_SHOT,
                        outcome: HitOutcome::LANDED,
                        proc_chance: 0.15,
                        ..ProcTrigger::default()
                    },
                );
                // AddStatProcBuff and the item swap proc only bookkeep for the sim's item proc
                // manager and item swapping, which preparation refuses.
                true
            }
            TALON_OF_ALAR => {
                self.talon_of_alar_aura = Some(sim.register_aura(
                    unit,
                    AuraConfig {
                        label: "Shot Power".to_string(),
                        action_id: Some(ActionId::spell(37508)),
                        duration: 6 * SECOND + 1,
                        ..AuraConfig::default()
                    },
                ));
                sim.make_proc_trigger_aura(
                    unit,
                    &ProcTrigger {
                        name: "Improved Shots".to_string(),
                        metrics_action_id: ActionId::spell(37507),
                        callback: CallbackMask::ON_SPELL_HIT_DEALT,
                        class_spell_mask: masks::ARCANE_SHOT,
                        outcome: HitOutcome::LANDED,
                        ..ProcTrigger::default()
                    },
                );
                true
            }
            // The PvP gloves register an effect that does nothing: `addPvpGloves` reads them.
            23279 | 22862 | 16463 | 16571 => true,
            _ => false,
        }
    }

    /// Renataki's Charm of Beasts: its use resets the cooldowns of the shots it names.
    fn register_renatakis_charm(&mut self, sim: &mut Sim, unit: UnitId) {
        let timer = sim.new_timer(unit);
        let shared = sim.get_offensive_trinket_cd(unit);
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: ActionId::item(RENATAKIS_CHARM_OF_BEASTS),
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
    }

    /// Thori'dal, the Star's Fury.
    fn register_thoridal(&mut self, sim: &mut Sim, unit: UnitId) {
        let is_equipped = sim.character(unit).equipment[slot::RANGED].id == THORIDAL_THE_STARS_FURY;
        let build_phase = if is_equipped {
            BuildPhase::GEAR
        } else {
            BuildPhase::NONE
        };
        let quiver = self.quiver_bonus_aura;
        let haste_aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Legendary Bow Haste".to_string(),
                action_id: Some(ActionId::spell(44972)),
                duration: NEVER_EXPIRES,
                build_phase,
                on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                    if let Some(quiver) = quiver {
                        sim.deactivate(quiver);
                    }
                })),
                on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                    if let Some(quiver) = quiver {
                        if sim.current_time > 0 {
                            sim.activate(quiver);
                        }
                    }
                })),
                ..AuraConfig::default()
            },
        );
        sim.attach_multiplicative_pseudo_stat_buff(
            haste_aura,
            PseudoStatField::RangedSpeedMultiplier,
            SPEED_15_MULTIPLIER,
        );
        // Requires No Ammo: its gain zeroes the ammo damage bonus, which only item swapping
        // reads.
        let ammo_aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Requires No Ammo".to_string(),
                action_id: Some(ActionId::spell(46699)),
                duration: NEVER_EXPIRES,
                on_gain: Some(Rc::new(|_: &mut Sim, _| {})),
                ..AuraConfig::default()
            },
        );
        if is_equipped {
            sim.make_permanent(haste_aura);
            sim.make_permanent(ammo_aura);
        }
    }

    /// The exporter's `hunterRenatakisCharm`: Aimed Shot's, Multi-Shot's and Arcane Shot's
    /// cooldowns, as the hunter has them, reset at once; the major cooldown waits for one of them
    /// to be cooling.
    pub(super) fn renatakis_charm_effect(&self, sim: &Sim, spell: SpellId) -> Value {
        let shots: Vec<i32> = [self.aimed_shot, self.multi_shot, self.arcane_shot]
            .into_iter()
            .flatten()
            .map(|shot| sim.spell(shot).action_id.spell_id)
            .collect();
        json!({"kind": "renatakis_charm", "item_id": sim.spell(spell).action_id.item_id,
            "shots": shots})
    }
}
