//! Go sim/mage/talents_frost.go.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::character::constants::CHARACTER_LEVEL;
use crate::prepare::dbcenums;
use crate::prepare::sim::{AuraConfig, EventCallbacks, Sim, UnitId};
use crate::prepare::spell::{school, ProcMask};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::SchoolIndex;

use super::super::masks;
use super::super::spell_data::spell_data;
use super::super::Mage;
use super::millis;

impl Mage {
    /// Go `registerFrostTalents`.
    pub(super) fn register_frost_talents(&self, sim: &mut Sim, unit: UnitId) {
        // Tier 1: Frost Warding models nothing in Go either.
        self.register_improved_frostbolt(sim, unit);
        self.register_elemental_precision(sim, unit);

        // Tier 2: Permafrost and Frostbite model nothing in Go either.
        self.register_ice_shards(sim, unit);
        self.register_improved_frost_nova(sim, unit);

        // Tier 3
        self.register_piercing_ice(sim, unit);
        self.register_frost_channeling(sim, unit);
        // Ice Lance: ice_lance.go
        // Improved Blizzard: blizzard.go

        // Tier 4: Arctic Reach and Ice Block model nothing in Go either.
        // Shatter: with Fingers of Frost below

        // Tier 5
        self.register_improved_cone_of_cold(sim, unit);
        // Cold Snap: cold_snap.go
        self.register_fingers_of_frost(sim, unit);

        // Tier 6
        self.register_winter_chill(sim, unit);

        // Tier 7: Ice Barrier models nothing in Go either.
    }

    fn register_improved_frostbolt(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("improved_frostbolt");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::FROSTBOLT,
                time_value: millis(
                    spell_data()
                        .improved_frostbolt
                        .effect(
                            dbcenums::A_ADD_FLAT_MODIFIER,
                            dbcenums::SPELLMOD_CASTING_TIME,
                        )
                        .value_at(rank),
                ),
                kind: SpellModType::CastTimeFlat,
                ..SpellModConfig::default()
            },
        );
    }

    fn register_elemental_precision(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("elemental_precision");
        if rank == 0 {
            return;
        }
        let hit = spell_data().elemental_precision.value_at(rank);
        let pseudo = &mut sim.unit_mut(unit).pseudo_stats;
        pseudo.school_bonus_hit_chance[SchoolIndex::Fire as usize] += hit;
        pseudo.school_bonus_hit_chance[SchoolIndex::Frost as usize] += hit;
    }

    fn register_ice_shards(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("ice_shards");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::FROST,
                float_value: spell_data().ice_shards.fraction_at(rank),
                kind: SpellModType::CritMultiplierFlat,
                ..SpellModConfig::default()
            },
        );
    }

    /// Improved Frost Nova (11165): Frost Nova's cooldown -2/-4 s.
    fn register_improved_frost_nova(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("improved_frost_nova");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::FROST_NOVA,
                time_value: millis(
                    spell_data()
                        .improved_frost_nova
                        .effect(dbcenums::A_ADD_FLAT_MODIFIER, dbcenums::SPELLMOD_COOLDOWN)
                        .value_at(rank),
                ),
                kind: SpellModType::CooldownFlat,
                ..SpellModConfig::default()
            },
        );
    }

    /// Effect 0 (SPELLMOD_DAMAGE) raises Frost spell hits, Blizzard's too. Effect 1
    /// (SPELLMOD_DOT) names Blizzard and Frostfire Bolt but has no rank curve in the client, so
    /// Frostfire Bolt's DoT gets its base 2% at every rank.
    fn register_piercing_ice(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("piercing_ice");
        if rank == 0 {
            return;
        }
        let data = &spell_data().piercing_ice;
        let hit = data
            .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_DAMAGE)
            .fraction_at(rank);
        let dot = data
            .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_DOT)
            .fraction_at(rank);
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL & !masks::FROSTFIRE_BOLT,
                school: school::FROST,
                float_value: hit,
                kind: SpellModType::DamageDoneFlat,
                ..SpellModConfig::default()
            },
        );
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::FROSTFIRE_BOLT,
                float_value: dot,
                kind: SpellModType::DamageDoneFlat,
                ..SpellModConfig::default()
            },
        );
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::FROSTFIRE_BOLT,
                float_value: hit - dot,
                kind: SpellModType::DirectDamageDoneFlat,
                ..SpellModConfig::default()
            },
        );
    }

    fn register_frost_channeling(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("frost_channeling");
        if rank == 0 {
            return;
        }
        let data = &spell_data().frost_channeling;
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::FROST,
                float_value: data
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_COST)
                    .fraction_at(rank),
                kind: SpellModType::PowerCostPctAdd,
                ..SpellModConfig::default()
            },
        );
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::FROST,
                float_value: -data.effect(dbcenums::A_MOD_THREAT, 16).fraction_at(rank),
                kind: SpellModType::ThreatMultiplierPct,
                ..SpellModConfig::default()
            },
        );
    }

    /// Improved Cone of Cold (11190): Cone of Cold damage +12/23/35%.
    fn register_improved_cone_of_cold(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("improved_cone_of_cold");
        if rank == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::CONE_OF_COLD,
                float_value: spell_data()
                    .improved_cone_of_cold
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_DAMAGE)
                    .fraction_at(rank),
                kind: SpellModType::DamageDoneFlat,
                ..SpellModConfig::default()
            },
        );
    }

    /// Raid bosses cannot be chilled or frozen, so Fingers of Frost is the only thing that gets
    /// Shatter and the Ice Lance bonus going on one; Shatter is folded in here because the two
    /// only ever fire together.
    fn register_fingers_of_frost(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("fingers_of_frost");
        if rank == 0 {
            return;
        }
        let data = spell_data();
        let proc_chance = data.fingers_of_frost.effect_at(2).fraction_at(rank);
        let fof_rank = data.fingers_of_frost_triggered.highest();
        let shatter_crit = data.shatter.value_at(self.talents.i32("shatter"));

        let shatter_mod = sim.add_dynamic_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                float_value: shatter_crit,
                kind: SpellModType::BonusCritPercent,
                ..SpellModConfig::default()
            },
        );

        // Go also holds a cast in flight out of the Shatter bonus when the aura is gained
        // mid-cast. Preparation never casts, so no cast is in flight when it gains the aura.
        sim.register_aura(
            unit,
            AuraConfig {
                label: "Fingers of Frost".to_string(),
                action_id: Some(ActionId {
                    spell_id: fof_rank.id,
                    ..ActionId::default()
                }),
                duration: fof_rank.duration(),
                max_stacks: data.fingers_of_frost.effect_at(1).value_at(rank) as i32,
                on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.activate_spell_mod(shatter_mod)
                })),
                on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.deactivate_spell_mod(shatter_mod)
                })),
                events: EventCallbacks {
                    on_cast_complete: true,
                    ..EventCallbacks::default()
                },
                ..AuraConfig::default()
            },
        );

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Fingers of Frost Trigger".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                class_spell_mask: masks::CHILL,
                outcome: HitOutcome::LANDED,
                proc_chance,
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    /// In Forever Winter's Chill only helps the mage's own Frostbolt and Ice Lance: 2% crit a
    /// stack, one stack per talent point.
    fn register_winter_chill(&self, sim: &mut Sim, unit: UnitId) {
        let rank = self.talents.i32("winters_chill");
        if rank == 0 {
            return;
        }
        let data = spell_data();
        let chill_rank = data.winters_chill_triggered.highest();
        let crit_per_stack = chill_rank.effect_n(1).average(CHARACTER_LEVEL);

        let crit_mod = sim.add_dynamic_mod(
            unit,
            SpellModConfig {
                class_mask: masks::FROSTBOLT | masks::ICE_LANCE,
                kind: SpellModType::BonusCritPercent,
                ..SpellModConfig::default()
            },
        );

        sim.register_aura(
            unit,
            AuraConfig {
                label: "Winter's Chill".to_string(),
                action_id: Some(ActionId {
                    spell_id: chill_rank.id,
                    ..ActionId::default()
                }),
                duration: chill_rank.duration(),
                max_stacks: data.winters_chill.effect_at(1).value_at(rank) as i32,
                on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.activate_spell_mod(crit_mod)
                })),
                on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.deactivate_spell_mod(crit_mod)
                })),
                on_stacks_change: Some(Rc::new(move |sim: &mut Sim, _, _, new_stacks| {
                    sim.update_spell_mod_float_value(
                        crit_mod,
                        crit_per_stack * f64::from(new_stacks),
                    )
                })),
                ..AuraConfig::default()
            },
        );

        // Forever states a flat SpellAuraOptions.ProcChance of 100 on the talent spell and puts
        // the real per-rank chance on effect 2 (by position); effect 1 is the stack count.
        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Winters Chill Talent".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                proc_mask: ProcMask::SPELL_DAMAGE,
                outcome: HitOutcome::LANDED,
                proc_chance: data.winters_chill.effect_at(2).fraction_at(rank),
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }
}
