//! Go sim/warlock/talents_demonology.go.

use std::rc::Rc;

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger, PseudoStatField};
use crate::prepare::character::constants::CHARACTER_LEVEL;
use crate::prepare::dbcenums;
use crate::prepare::periodic_action::PeriodicActionOptions;
use crate::prepare::sim::{AuraConfig, EventCallbacks, Sim, UnitId, NEVER_EXPIRES};
use crate::prepare::spell::DefenseType;
use crate::prepare::spell::{school, ProcMask, SpellConfig, SpellFlag};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{SchoolIndex, Stat};

use super::super::curses::new_enemy_aura_array;
use super::super::masks;
use super::super::spell_data::spell_data;
use super::super::spells::spell_action;
use super::super::Warlock;

impl Warlock {
    pub(in super::super) fn register_demonology_talents(&mut self, sim: &mut Sim, unit: UnitId) {
        // Tier 1
        // Improved Health Funnel models nothing in Go either.
        self.apply_improved_imp(sim);
        self.apply_demonic_embrace(sim, unit);
        self.apply_unholy_power(sim);

        // Tier 2
        // Demonic Aegis: armors.go
        // Improved Voidwalker models nothing in Go either.
        self.apply_fel_vitality(sim, unit);
        // Demonic Energies: lifetap.go

        // Tier 3
        self.apply_improved_sayaad(sim);
        self.apply_demonic_sacrifice(sim, unit);
        // Master Summoner models nothing in Go either.

        // Tier 4
        self.apply_decimation(sim, unit);
        // Fel Domination models nothing in Go either.
        self.apply_demonic_brand(sim, unit);

        // Tier 5
        // Improved Felhunter models nothing in Go either.
        self.apply_soul_link(sim, unit);
        self.apply_demonic_knowledge(sim, unit);

        // Tier 6
        self.apply_master_demonologist(sim, unit);

        // Tier 7
        // Demonic Pact: its sacrifice is applied in applyDemonicSacrifice.
    }

    /// The Firebolt half of 18694; its first effect carries the same ladder for Blood Pact,
    /// which the party buff handles.
    fn apply_improved_imp(&self, sim: &mut Sim) {
        let points = self.talents.i32("improved_imp");
        if points == 0 || self.sacrifice_summon() {
            return;
        }
        let Some(imp) = self.pets.imp else {
            return;
        };
        sim.add_static_mod(
            imp,
            SpellModConfig {
                kind: SpellModType::DamageDoneFlat,
                float_value: spell_data().improved_imp.effect_at(2).fraction_at(points),
                class_mask: masks::IMP_FIRE_BOLT,
                ..SpellModConfig::default()
            },
        );
    }

    /// Forever drops Classic's spirit penalty: 18697 only raises stamina.
    fn apply_demonic_embrace(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("demonic_embrace");
        if points == 0 {
            return;
        }
        sim.unit_mut(unit).sdm.multiply_stat(
            Stat::Stamina,
            spell_data()
                .demonic_embrace
                .effect_at(1)
                .multiplier_at(points),
        );
    }

    /// 2% more pet damage a point (18769).
    fn apply_unholy_power(&self, sim: &mut Sim) {
        let points = self.talents.i32("unholy_power");
        if points == 0 || self.sacrifice_summon() {
            return;
        }
        let multiplier = spell_data().unholy_power.multiplier_at(points);
        for pet in &self.pets.base {
            sim.unit_mut(*pet).pseudo_stats.damage_dealt_multiplier *= multiplier;
        }
    }

    /// 5% more mana for the warlock and 5% more health and mana for the demon, a point (18731).
    fn apply_fel_vitality(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("fel_vitality");
        if points == 0 {
            return;
        }
        let multiplier = spell_data().fel_vitality.effect_at(1).multiplier_at(points);
        sim.unit_mut(unit).sdm.multiply_stat(Stat::Mana, multiplier);
        for pet in &self.pets.base {
            let sdm = &mut sim.unit_mut(*pet).sdm;
            sdm.multiply_stat(Stat::Health, multiplier);
            sdm.multiply_stat(Stat::Mana, multiplier);
        }
    }

    /// 10% more Lash of Pain damage a point: 18754 effect 0 (the tooltip's $s1).
    fn apply_improved_sayaad(&self, sim: &mut Sim) {
        let points = self.talents.i32("improved_sayaad");
        if points == 0 || self.sacrifice_summon() {
            return;
        }
        let Some(succubus) = self.pets.succubus else {
            return;
        };
        sim.add_static_mod(
            succubus,
            SpellModConfig {
                kind: SpellModType::DamageDoneFlat,
                float_value: spell_data()
                    .improved_sayaad
                    .effect_at(1)
                    .fraction_at(points),
                class_mask: masks::SUCCUBUS_LASH_OF_PAIN,
                ..SpellModConfig::default()
            },
        );
    }

    /// Each demon leaves behind the opposing aspect, and Forever's pairing is the reverse of
    /// Classic's. The demon is sacrificed before the pull, so the buff is simply permanent and
    /// no pet is ever summoned.
    fn apply_demonic_sacrifice(&self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("demonic_sacrifice") {
            return;
        }

        let mut demon = self.options.enum_name("summon");
        if !self.sacrifice_summon() {
            // Demonic Pact (425464): a demon sacrificed before the pull keeps its buff while a
            // different one is out; summoning the sacrificed demon again cancels it.
            if !self.talents.bool("demonic_pact")
                || self.options.enum_name("pact_sacrifice") == self.options.enum_name("summon")
            {
                return;
            }
            demon = self.options.enum_name("pact_sacrifice");
        }

        let (spell_id, school_index) = match demon.as_str() {
            "Imp" => (18789, SchoolIndex::Shadow),
            "Succubus" => (18791, SchoolIndex::Fire),
            "Voidwalker" => {
                self.apply_fel_energy(sim, unit);
                return;
            }
            // The Felhunter's health is survival only; it is left out until the sim needs it.
            _ => return,
        };

        let row = spell_data().demonic_sacrifice_triggered.by_id(spell_id);
        let multiplier = 1.0 + row.effect_n(1).percent();

        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Demonic Sacrifice".to_string(),
                action_id: Some(spell_action(spell_id)),
                duration: row.duration(),
                ..AuraConfig::default()
            },
        );
        sim.attach_multiplicative_pseudo_stat_buff(
            aura,
            PseudoStatField::SchoolDamageDealtMultiplier(school_index),
            multiplier,
        );
        sim.make_permanent(aura);
    }

    /// The Voidwalker's sacrifice, Fel Energy (18792): 2% of total mana every 4 s.
    fn apply_fel_energy(&self, sim: &mut Sim, unit: UnitId) {
        let row = spell_data().demonic_sacrifice_triggered.by_id(18792);
        let period = row.effect_n(1).period();

        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Demonic Sacrifice".to_string(),
                action_id: Some(spell_action(row.id)),
                duration: row.duration(),
                on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.start_periodic_action(PeriodicActionOptions {
                        period,
                        ..PeriodicActionOptions::default()
                    });
                })),
                ..AuraConfig::default()
            },
        );
        sim.make_permanent(aura);
    }

    /// Shadow Bolt and Searing Pain hit 3% harder a point below 35% health, and Soul Fire casts
    /// 20% a point faster and comes off cooldown 45% a point sooner (440870 / 440873).
    fn apply_decimation(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("decimation");
        if points == 0 {
            return;
        }
        let data = &spell_data().decimation;
        let triggered = spell_data().decimation_triggered.highest();

        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::CooldownMultiplier,
                float_value: 1.0
                    + data
                        .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_COOLDOWN)
                        .fraction_at(points),
                class_mask: masks::SOUL_FIRE,
                ..SpellModConfig::default()
            },
        );

        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Decimation".to_string(),
                action_id: Some(spell_action(triggered.id)),
                duration: triggered.duration(),
                ..AuraConfig::default()
            },
        );
        sim.attach_spell_mod(
            aura,
            SpellModConfig {
                kind: SpellModType::DamageDoneFlat,
                float_value: data.effect_at(4).fraction_at(points),
                class_mask: masks::SHADOW_BOLT | masks::SEARING_PAIN,
                ..SpellModConfig::default()
            },
        );
        sim.attach_spell_mod(
            aura,
            SpellModConfig {
                kind: SpellModType::CastTimePct,
                float_value: data.effect_at(1).fraction_at(points),
                class_mask: masks::SOUL_FIRE,
                ..SpellModConfig::default()
            },
        );
        self.decimation_aura = Some(aura);

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Decimation Trigger".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                class_spell_mask: masks::SHADOW_BOLT | masks::SEARING_PAIN,
                outcome: HitOutcome::LANDED,
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    /// Searing Pain brands its target for 10 seconds with 2/4/6 charges. The Imp uses Fire
    /// damage (1293698); the other demons use Shadow (1293697).
    fn apply_demonic_brand(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("demonic_brand");
        if points == 0 {
            return;
        }
        let data = &spell_data().demonic_brand;
        let triggered = spell_data().demonic_brand_triggered.highest();
        let action_id = spell_action(triggered.id);

        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::ThreatMultiplierPct,
                float_value: data
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_THREAT)
                    .fraction_at(points),
                class_mask: masks::SEARING_PAIN,
                ..SpellModConfig::default()
            },
        );

        if self.sacrifice_summon() {
            return;
        }

        let charges = data
            .effect(dbcenums::A_ADD_FLAT_MODIFIER, dbcenums::SPELLMOD_CHARGES)
            .value_at(points) as i32;
        let index = sim.unit(unit).unit_index;
        let duration = triggered.duration();
        self.demonic_brand_auras = new_enemy_aura_array(sim, |sim, target| {
            sim.register_aura(
                target,
                AuraConfig {
                    label: format!("Demonic Brand-{index}"),
                    action_id: Some(action_id.clone()),
                    duration,
                    max_stacks: charges,
                    ..AuraConfig::default()
                },
            )
        });

        // Keep the old pet aura available to saved APLs, including auraIsKnown talent guards.
        sim.register_reset_effect(unit, Rc::new(|_: &mut Sim| {}));
        let imp = self.pets.imp;
        for pet in self.pets.base.clone() {
            let (brand_id, brand_school) = if Some(pet) == imp {
                (1293698, school::FIRE)
            } else {
                (1293697, school::SHADOW)
            };
            sim.register_spell(
                pet,
                SpellConfig {
                    action_id: spell_action(brand_id),
                    spell_school: brand_school,
                    defense_type: DefenseType::Magic,
                    proc_mask: ProcMask::EMPTY,
                    flags: SpellFlag::PASSIVE_SPELL | SpellFlag::NO_ON_CAST_COMPLETE,
                    damage_multiplier: 1.0,
                    threat_multiplier: 3.0,
                    ..SpellConfig::default()
                },
            );

            sim.register_aura(
                pet,
                AuraConfig {
                    label: "Demonic Brand".to_string(),
                    action_id: Some(action_id.clone()),
                    duration,
                    max_stacks: charges,
                    ..AuraConfig::default()
                },
            );
            let consumer = sim.register_aura(
                pet,
                AuraConfig {
                    label: "Demonic Brand consumer".to_string(),
                    events: EventCallbacks {
                        on_spell_hit_dealt: true,
                        ..EventCallbacks::default()
                    },
                    ..AuraConfig::default()
                },
            );
            sim.make_permanent(consumer);
        }

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Demonic Brand Trigger".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                class_spell_mask: masks::SEARING_PAIN,
                outcome: HitOutcome::LANDED,
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    /// 3% more damage dealt and 30% of the damage taken split with the demon (25228).
    fn apply_soul_link(&self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("soul_link") || self.sacrifice_summon() {
            return;
        }
        let row = spell_data().soul_link_triggered.by_id(25228);
        let damage_dealt = 1.0 + row.effect_n(1).percent();
        let damage_taken = 1.0 - row.effect_n(2).percent();

        let config = move || AuraConfig {
            label: "Soul Link".to_string(),
            action_id: Some(spell_action(19028)),
            duration: NEVER_EXPIRES,
            on_gain: Some(Rc::new(move |sim: &mut Sim, aura| {
                let unit = sim.aura(aura).unit;
                let pseudo = &mut sim.unit_mut(unit).pseudo_stats;
                pseudo.damage_dealt_multiplier *= damage_dealt;
                pseudo.damage_taken_multiplier *= damage_taken;
            })),
            on_expire: Some(Rc::new(move |sim: &mut Sim, aura| {
                let unit = sim.aura(aura).unit;
                let pseudo = &mut sim.unit_mut(unit).pseudo_stats;
                pseudo.damage_dealt_multiplier /= damage_dealt;
                pseudo.damage_taken_multiplier /= damage_taken;
            })),
            ..AuraConfig::default()
        };

        let aura = sim.register_aura(unit, config());
        sim.make_permanent(aura);
        for pet in &self.pets.base {
            let aura = sim.register_aura(*pet, config());
            sim.make_permanent(aura);
        }
    }

    /// 33/67/100% of the warlock's level in spell power for the warlock and the demon while it
    /// is out (412732).
    fn apply_demonic_knowledge(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("demonic_knowledge");
        if points == 0 || self.sacrifice_summon() {
            return;
        }
        let bonus = spell_data().demonic_knowledge.fraction_at(points) * f64::from(CHARACTER_LEVEL);

        let config = || AuraConfig {
            label: "Demonic Knowledge".to_string(),
            action_id: Some(spell_action(412732)),
            duration: NEVER_EXPIRES,
            ..AuraConfig::default()
        };
        let aura = sim.register_aura(unit, config());
        sim.attach_stat_buff(aura, Stat::SpellDamage, bonus);
        sim.make_permanent(aura);
        for pet in &self.pets.base {
            let aura = sim.register_aura(*pet, config());
            sim.attach_stat_buff(aura, Stat::SpellDamage, bonus);
            sim.make_permanent(aura);
        }
    }

    /// 2% a point, on the school the demon out matches (23785): Fire for the Imp, Shadow for the
    /// Succubus. Both the warlock and the demon get it.
    fn apply_master_demonologist(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("master_demonologist");
        if points == 0 || self.sacrifice_summon() {
            return;
        }
        let fraction = spell_data()
            .master_demonologist
            .effect_at(1)
            .fraction_at(points);

        let (label, tag, school_index) = match self.options.enum_name("summon").as_str() {
            "Imp" => ("Master Demonologist (Imp)", 1, SchoolIndex::Fire),
            "Succubus" => ("Master Demonologist (Succubus)", 3, SchoolIndex::Shadow),
            // The Voidwalker's and the Felhunter's halves only cut damage taken.
            _ => return,
        };

        let buff = |sim: &mut Sim, target: UnitId| {
            let aura = sim.register_aura(
                target,
                AuraConfig {
                    label: label.to_string(),
                    action_id: Some(crate::contracts::prepared_v2::ActionId {
                        spell_id: 23785,
                        tag,
                        ..Default::default()
                    }),
                    duration: NEVER_EXPIRES,
                    ..AuraConfig::default()
                },
            );
            sim.attach_multiplicative_pseudo_stat_buff(
                aura,
                PseudoStatField::SchoolDamageDealtMultiplier(school_index),
                1.0 + fraction,
            );
            sim.make_permanent(aura)
        };

        buff(sim, unit);
        if let Some(active) = self.pets.active {
            buff(sim, active);
        }
    }
}
