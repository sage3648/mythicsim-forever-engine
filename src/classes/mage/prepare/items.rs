//! Go sim/mage/items.go: the Mage's item sets and the one item effect it registers.
//!
//! The shared item registry asks this module whether it knows a set bonus or an item: Go's
//! `core.NewItemSet` and `core.NewItemEffect` calls in `init`.

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::character::{cooldown_type, MajorCooldown};
use crate::prepare::env::Environment;
use crate::prepare::sim::{AuraConfig, AuraId, Cooldown, Sim, UnitId, SECOND};
use crate::prepare::spell::{school, CastConfig, ProcMask, SpellConfig, SpellFlag};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{Stat, Stats};

use super::masks;

/// Go `core.ApplySetBonus` for a Mage set: the set bonus aura is the one the bonus attaches to.
pub(crate) type ApplySetBonus = fn(&mut Sim, UnitId, AuraId);

/// Go `core.ItemSet`, for the fields the bonus lookup reads.
pub(crate) struct MageItemSet {
    pub id: i32,
    pub name: &'static str,
    /// Set piece requirement and its bonus.
    pub bonuses: &'static [(i32, ApplySetBonus)],
}

/// Aldor Regalia (648), Tirisfal Regalia (649) and Tempest Regalia (671).
pub(crate) static ITEM_SETS: &[MageItemSet] = &[
    MageItemSet {
        id: 648,
        name: "Aldor Regalia",
        bonuses: &[(4, aldor_regalia_4)],
    },
    MageItemSet {
        id: 649,
        name: "Tirisfal Regalia",
        bonuses: &[(2, tirisfal_regalia_2), (4, tirisfal_regalia_4)],
    },
    MageItemSet {
        id: 671,
        name: "Tempest Regalia",
        bonuses: &[(2, tempest_regalia_2), (4, tempest_regalia_4)],
    },
];

fn aldor_regalia_4(sim: &mut Sim, _unit: UnitId, aura: AuraId) {
    for (class_mask, seconds) in [
        (masks::PRESENCE_OF_MIND, -24),
        (masks::BLAST_WAVE, -4),
        (masks::ICE_BLOCK, -40),
    ] {
        sim.attach_spell_mod(
            aura,
            SpellModConfig {
                kind: SpellModType::CastTimeFlat,
                time_value: SECOND * seconds,
                class_mask,
                ..SpellModConfig::default()
            },
        );
    }
}

fn tirisfal_regalia_2(sim: &mut Sim, _unit: UnitId, aura: AuraId) {
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::DamageDoneFlat,
            float_value: 0.20,
            class_mask: masks::ARCANE_BLAST,
            ..SpellModConfig::default()
        },
    );
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::PowerCostPctAdd,
            float_value: 0.20,
            class_mask: masks::ARCANE_BLAST,
            ..SpellModConfig::default()
        },
    );
}

fn tirisfal_regalia_4(sim: &mut Sim, unit: UnitId, aura: AuraId) {
    // Go's temporary stats aura, which the proc below activates in a fight.
    let _madness = sim.new_temporary_stats_aura(
        unit,
        "Arcane Madness",
        &ActionId {
            spell_id: 37444,
            ..ActionId::default()
        },
        Stats::from_pairs(&[(Stat::SpellDamage, 70.0)]),
        SECOND * 6,
    );
    sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Tirisfal 4PC".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            proc_mask: ProcMask::SPELL_DAMAGE,
            outcome: HitOutcome::CRIT,
            ..ProcTrigger::default()
        },
    );
}

fn tempest_regalia_2(sim: &mut Sim, _unit: UnitId, aura: AuraId) {
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::DotNumberOfTicksFlat,
            int_value: 1,
            class_mask: masks::EVOCATION,
            ..SpellModConfig::default()
        },
    );
}

fn tempest_regalia_4(sim: &mut Sim, _unit: UnitId, aura: AuraId) {
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::DamageDoneFlat,
            float_value: 0.05,
            class_mask: masks::FIREBALL | masks::FROSTBOLT | masks::ARCANE_MISSILES_TICK,
            ..SpellModConfig::default()
        },
    );
}

/// The Go `core.NewItemEffect(19959, ...)`: Hazza'rah's Charm of Magic. Use: increases the
/// critical hit chance of your Arcane spells by 5%, and the critical hit damage by 50% for
/// 20 sec (24544). The client's class mask names Arcane Explosion and Arcane Missiles only.
pub(crate) fn hazzarahs_charm(env: &mut Environment) {
    // The warlock tests pull the mage package in, and with it this mage-only charm.
    let unit = env.player;
    if env.sim.character(unit).class != "ClassMage" {
        return;
    }
    let sim = &mut env.sim;
    let duration = SECOND * 20;

    let aura = sim.register_aura(
        unit,
        AuraConfig {
            label: "Arcane Potency".to_string(),
            action_id: Some(ActionId {
                spell_id: 24544,
                ..ActionId::default()
            }),
            duration,
            ..AuraConfig::default()
        },
    );
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::BonusCritPercent,
            float_value: 5.0,
            class_mask: masks::ARCANE_EXPLOSION | masks::ARCANE_MISSILES,
            ..SpellModConfig::default()
        },
    );
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::CritMultiplierFlat,
            float_value: 0.5,
            class_mask: masks::ARCANE_EXPLOSION | masks::ARCANE_MISSILES,
            ..SpellModConfig::default()
        },
    );

    let timer = sim.new_timer(unit);
    let shared = sim.get_offensive_trinket_cd(unit);
    let spell = sim.register_spell(
        unit,
        SpellConfig {
            action_id: ActionId {
                item_id: 19959,
                ..ActionId::default()
            },
            spell_school: school::ARCANE,
            flags: SpellFlag::NO_ON_CAST_COMPLETE,
            cast: CastConfig {
                cd: Cooldown {
                    timer: Some(timer),
                    duration: SECOND * 180,
                },
                shared_cd: Cooldown {
                    timer: Some(shared),
                    duration,
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
