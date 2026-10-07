//! Go sim/warlock/talents_affliction.go.

use crate::prepare::aura_helpers::{CallbackMask, ProcTrigger};
use crate::prepare::character::{cooldown_type, MajorCooldown};
use crate::prepare::dbcenums;
use crate::prepare::sim::{AuraConfig, Cooldown, Sim, UnitId};
use crate::prepare::spell::{CastConfig, SpellConfig, SpellFlag};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::Stat;

use super::super::masks;
use super::super::spell_data::spell_data;
use super::super::spells::{longest_cooldown, spell_action};
use super::super::Warlock;
use super::millis;

impl Warlock {
    pub(in super::super) fn register_affliction_talents(&mut self, sim: &mut Sim, unit: UnitId) {
        // Tier 1
        // Improved Life Tap: lifetap.go
        self.apply_suppression(sim, unit);
        self.apply_improved_corruption(sim, unit);

        // Tier 2
        self.apply_malediction(sim, unit);
        // Soul Harvest models nothing in Go either.
        self.apply_improved_drains(sim, unit);

        // Tier 3
        self.apply_improved_bane_of_agony(sim, unit);
        // Fel Concentration models nothing in Go either.
        self.register_amplify_curse(sim, unit);
        self.apply_pandemic(sim, unit);

        // Tier 4
        self.apply_malevolence(sim, unit);
        self.apply_nightfall(sim, unit);
        // Curse of Exhaustion models nothing in Go either.

        // Tier 5
        // Siphon Life: siphon_life.go
        // Soul Siphon: drain_life.go

        // Tier 6
        self.apply_shadow_mastery(sim, unit);

        // Tier 7
        // Wrack: wrack.go
    }

    /// 1% hit a point on every school the warlock casts plus 4% less threat a point.
    fn apply_suppression(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("suppression");
        if points == 0 {
            return;
        }
        let data = &spell_data().suppression;
        sim.add_stat(
            unit,
            Stat::SpellHitPercent,
            data.effect(dbcenums::A_MOD_SPELL_HIT_CHANCE, 0)
                .value_at(points),
        );
        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::ThreatMultiplierPct,
                float_value: data.effect(dbcenums::A_MOD_THREAT, 127).fraction_at(points),
                class_mask: masks::ALL,
                ..SpellModConfig::default()
            },
        );
    }

    /// A cast time cut and, in Forever, 2% more dot damage a point.
    fn apply_improved_corruption(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("improved_corruption");
        if points == 0 {
            return;
        }
        let data = &spell_data().improved_corruption;
        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::CastTimeFlat,
                time_value: millis(
                    data.effect(
                        dbcenums::A_ADD_FLAT_MODIFIER,
                        dbcenums::SPELLMOD_CASTING_TIME,
                    )
                    .value_at(points),
                ),
                class_mask: masks::CORRUPTION,
                ..SpellModConfig::default()
            },
        );
        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::DotDamageDonePct,
                float_value: data
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_DOT)
                    .fraction_at(points),
                class_mask: masks::CORRUPTION,
                ..SpellModConfig::default()
            },
        );
    }

    /// Forever moved Malediction off the curse: the beta client's 1225177 is a flat 1% a point
    /// on the warlock's own periodic damage. The mask names Hellfire, but its area hits are
    /// Hellfire Effect (11682), a direct School Damage effect, so a dot modifier never reaches
    /// them; only the self-burn is periodic.
    fn apply_malediction(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("malediction");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::DotDamageDonePct,
                float_value: spell_data()
                    .malediction
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_DOT)
                    .fraction_at(points),
                class_mask: masks::ALL & !masks::HELLFIRE,
                ..SpellModConfig::default()
            },
        );
    }

    /// Improved Drains: 7/13/20% more drain damage (403511, a dot modifier).
    fn apply_improved_drains(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("improved_drains");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::DotDamageDonePct,
                float_value: spell_data().improved_drains.fraction_at(points),
                class_mask: masks::DRAIN_SPELLS,
                ..SpellModConfig::default()
            },
        );
    }

    /// Improved Bane of Agony: 5/10% more periodic damage (18827).
    fn apply_improved_bane_of_agony(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("improved_bane_of_agony");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::DotDamageDonePct,
                float_value: spell_data().improved_bane_of_agony.fraction_at(points),
                class_mask: masks::CURSE_OF_AGONY,
                ..SpellModConfig::default()
            },
        );
    }

    /// Pandemic lets the Affliction dots crit for 33/67/100% more (427712).
    fn apply_pandemic(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("pandemic");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::CritMultiplierFlat,
                float_value: spell_data().pandemic.fraction_at(points),
                class_mask: masks::PERIODIC_SHADOW_DAMAGE,
                ..SpellModConfig::default()
            },
        );
    }

    /// 1% shadow crit a point (1310949).
    fn apply_malevolence(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("malevolence");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::BonusCritPercent,
                float_value: spell_data().malevolence.value_at(points),
                class_mask: masks::SHADOW_DAMAGE,
                ..SpellModConfig::default()
            },
        );
    }

    fn apply_nightfall(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("nightfall");
        if points == 0 {
            return;
        }
        let data = spell_data();
        let triggered = data.nightfall_triggered.highest();

        let aura = sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Shadow Trance".to_string(),
                metrics_action_id: spell_action(triggered.id),
                duration: triggered.duration(),
                class_spell_mask: masks::SHADOW_BOLT,
                callback: CallbackMask::ON_CAST_COMPLETE,
                ..ProcTrigger::default()
            },
        );
        sim.attach_spell_mod(
            aura,
            SpellModConfig {
                kind: SpellModType::CastTimePct,
                float_value: -1.0,
                class_mask: masks::SHADOW_BOLT,
                ..SpellModConfig::default()
            },
        );

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Nightfall".to_string(),
                class_spell_mask: masks::NIGHTFALL_SPELLS,
                // The per-rank chance is on the effect; ProcChanceAt reads the row's flat 100%.
                proc_chance: data.nightfall.fraction_at(points),
                callback: CallbackMask::ON_PERIODIC_DAMAGE_DEALT,
                ..ProcTrigger::default()
            },
        );
    }

    /// Shadow Mastery is 1% a point on both damage and dot damage in Forever: 18271 kept its op
    /// 0 and op 22 modifiers, so every shadow spell takes it the same way. Neither mask names
    /// Life Tap, so its mana is left alone.
    fn apply_shadow_mastery(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("shadow_mastery");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::DamageDoneFlat,
                float_value: spell_data()
                    .shadow_mastery
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_DAMAGE)
                    .fraction_at(points),
                class_mask: masks::SHADOW_DAMAGE,
                ..SpellModConfig::default()
            },
        );
    }

    fn register_amplify_curse(&self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("amplify_curse") {
            return;
        }
        let rank = spell_data().amplify_curse.highest();
        let action_id = spell_action(rank.id);

        // Spent by Bane of Agony, the only spell in 18288's mask the sim casts.
        sim.get_or_register_aura(
            unit,
            AuraConfig {
                label: "Amplify Curse".to_string(),
                action_id: Some(action_id.clone()),
                duration: rank.duration(),
                ..AuraConfig::default()
            },
        );

        let timer = sim.new_timer(unit);
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id,
                spell_school: rank.spell_school(),
                flags: SpellFlag::APL,
                cast: CastConfig {
                    cd: Cooldown {
                        timer: Some(timer),
                        duration: longest_cooldown(rank),
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
}
