//! Go sim/priest/talents_shadow.go.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger, PseudoStatField};
use crate::prepare::character::constants::CHARACTER_LEVEL;
use crate::prepare::dbcenums;
use crate::prepare::sim::{
    AuraConfig, AuraId, Cooldown, EventCallbacks, Sim, UnitId, UnitType, NEVER_EXPIRES,
};
use crate::prepare::spell::{
    school, Cast, CastConfig, CostOptions, DefenseType, ProcMask, SpellConfig, SpellFlag,
    GCD_DEFAULT,
};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::SchoolIndex;

use super::super::masks;
use super::super::spell_data::spell_data;
use super::super::spells::{flat_cost, longest_cooldown};
use super::super::Priest;
use super::millis;

impl Priest {
    /// Go `registerShadowTalents`.
    pub(super) fn register_shadow_talents(&mut self, sim: &mut Sim, unit: UnitId) {
        // Tier 1
        self.apply_shadow_focus(sim, unit);
        // Blackout and Spirit Tap model nothing in Go either.

        // Tier 2
        self.apply_shadow_affinity(sim, unit);
        self.apply_improved_shadow_word_pain(sim, unit);
        // Shadow Reach models nothing in Go either.

        // Tier 3
        self.apply_improved_mind_blast(sim, unit);
        // Improved Psychic Scream models nothing in Go either.
        self.apply_mind_flay(sim, unit);
        self.apply_improved_mind_flay(sim, unit);

        // Tier 4
        // Improved Fade models nothing in Go either.
        self.apply_vampiric_embrace(sim, unit);
        self.apply_shadow_weaving(sim, unit);

        // Tier 5
        // Silence models nothing in Go either.
        self.apply_devouring_contagion(sim, unit);

        // Tier 6
        // Early Demise is read where Shadow Word: Death casts.
        self.apply_darkness(sim, unit);

        // Tier 7
        self.apply_shadowform(sim, unit);
    }

    /// A school-specific hit bonus goes through the pseudo-stat.
    fn apply_shadow_focus(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("shadow_focus");
        if points == 0 {
            return;
        }
        sim.unit_mut(unit).pseudo_stats.school_bonus_hit_chance[SchoolIndex::Shadow as usize] +=
            spell_data()
                .shadow_focus
                .effect(
                    dbcenums::A_ADD_FLAT_MODIFIER,
                    dbcenums::SPELLMOD_RESIST_MISS_CHANCE,
                )
                .value_at(points);
    }

    fn apply_shadow_affinity(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("shadow_affinity");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::SHADOW,
                float_value: spell_data().shadow_affinity.fraction_at(points),
                kind: SpellModType::ThreatMultiplierPct,
                ..SpellModConfig::default()
            },
        );
    }

    /// +3 sec of duration per point, which is one more tick at Shadow Word: Pain's 3 second
    /// cadence.
    fn apply_improved_shadow_word_pain(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("improved_shadow_word_pain");
        if points == 0 {
            return;
        }
        let data = spell_data();
        let added = millis(data.improved_shadow_word_pain.value_at(points));
        let tick_length = data.shadow_word_pain.highest().periodic_effect().period();
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::SHADOW_WORD_PAIN,
                int_value: (added / tick_length) as i32,
                kind: SpellModType::DotNumberOfTicksFlat,
                ..SpellModConfig::default()
            },
        );
    }

    fn apply_improved_mind_blast(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("improved_mind_blast");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::MIND_BLAST,
                time_value: millis(spell_data().improved_mind_blast.value_at(points)),
                kind: SpellModType::CooldownFlat,
                ..SpellModConfig::default()
            },
        );
    }

    fn apply_mind_flay(&self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("mind_flay") {
            return;
        }
        spell_data().mind_flay.each(|_, rank| {
            Self::register_mind_flay_spell(sim, unit, rank);
        });
    }

    /// Improved Mind Flay is new in Forever: +10% damage per point, plus range and slow the sim
    /// ignores.
    fn apply_improved_mind_flay(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("improved_mind_flay");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::MIND_FLAY,
                float_value: spell_data()
                    .improved_mind_flay
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_DOT)
                    .fraction_at(points),
                kind: SpellModType::DamageDoneFlat,
                ..SpellModConfig::default()
            },
        );
    }

    /// The Forever client heals the priest's whole party for a share of the Shadow damage it
    /// deals. Only the priest who applied the debuff heals from it.
    fn apply_vampiric_embrace(&mut self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("vampiric_embrace") {
            return;
        }
        let rank = spell_data().vampiric_embrace.highest();

        // Go `NewEnemyAuraArray`: one debuff for each enemy, by unit index.
        let units = sim.all_units();
        let mut auras: Vec<Option<AuraId>> = vec![None; units.len()];
        for target in units {
            if sim.unit(target).unit_type != UnitType::Enemy {
                continue;
            }
            let label = format!("Vampiric Embrace - {}", sim.unit(target).label);
            let aura = sim.get_or_register_aura(
                target,
                AuraConfig {
                    label,
                    action_id: Some(ActionId::spell(rank.id)),
                    duration: rank.duration(),
                    ..AuraConfig::default()
                },
            );
            sim.attach_proc_trigger_callback(
                aura,
                target,
                &ProcTrigger {
                    name: "Vampiric Embrace Proc".to_string(),
                    callback: CallbackMask::ON_SPELL_HIT_TAKEN
                        | CallbackMask::ON_PERIODIC_DAMAGE_TAKEN,
                    require_damage_dealt: true,
                    ..ProcTrigger::default()
                },
            );
            let index = sim.unit(target).unit_index as usize;
            auras[index] = Some(aura);
        }
        let mut related = crate::prepare::spell::LabeledAuraArrays::new();
        if let Some(first) = auras.iter().flatten().next() {
            related.insert(sim.aura(*first).label.clone(), auras);
        }

        let timer = sim.new_timer(unit);
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: ActionId::spell(rank.id),
                spell_school: school::SHADOW,
                defense_type: DefenseType::Magic,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::APL,
                class_spell_mask: masks::VAMPIRIC_EMBRACE,
                cost: flat_cost(rank),
                cast: CastConfig {
                    default_cast: Cast {
                        gcd: GCD_DEFAULT,
                        ..Cast::default()
                    },
                    cd: Cooldown {
                        timer: Some(timer),
                        duration: longest_cooldown(rank),
                    },
                    ..CastConfig::default()
                },
                related_aura_arrays: related,
                ..SpellConfig::default()
            },
        );
        self.vampiric_embrace = Some(spell);
    }

    /// The raid debuff version of Shadow Weaving is a Classic mechanic; in Forever the stacks
    /// raise the Shadow damage the priest deals, 2% a stack to five.
    fn apply_shadow_weaving(&mut self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("shadow_weaving");
        if points == 0 {
            return;
        }
        let data = spell_data();
        let stack_aura = data.shadow_weaving_triggered.highest();
        let per_stack = stack_aura
            .effect(dbcenums::A_MOD_SCHOOL_MASK_DAMAGE_FROM_CASTER, 32)
            .average(CHARACTER_LEVEL)
            / 100.0;

        let damage_mod = sim.add_dynamic_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::SHADOW,
                kind: SpellModType::DamageDonePct,
                ..SpellModConfig::default()
            },
        );

        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Shadow Weaving".to_string(),
                action_id: Some(ActionId::spell(stack_aura.id)),
                duration: stack_aura.duration(),
                max_stacks: 5,
                on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.activate_spell_mod(damage_mod);
                })),
                on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                    sim.deactivate_spell_mod(damage_mod);
                })),
                on_stacks_change: Some(Rc::new(move |sim: &mut Sim, _, _, new_stacks| {
                    sim.update_spell_mod_float_value(damage_mod, per_stack * f64::from(new_stacks));
                })),
                ..AuraConfig::default()
            },
        );
        self.shadow_weaving_aura = Some(aura);

        sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Shadow Weaving Trigger".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                class_spell_mask: masks::SHADOW_SPELLS,
                outcome: HitOutcome::LANDED,
                proc_chance: data.shadow_weaving.fraction_at(points),
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
    }

    /// Devouring Contagion is new in Forever: Devouring Plague costs 25% less per point.
    fn apply_devouring_contagion(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("devouring_contagion");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::DEVOURING_PLAGUE,
                float_value: spell_data()
                    .devouring_contagion
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_COST)
                    .fraction_at(points),
                kind: SpellModType::PowerCostPctAdd,
                ..SpellModConfig::default()
            },
        );
    }

    /// Classic's Darkness raised the damage of five named Shadow spells. The Forever client's
    /// raises all Shadow damage done, 2% per point.
    fn apply_darkness(&self, sim: &mut Sim, unit: UnitId) {
        let points = self.talents.i32("darkness");
        if points == 0 {
            return;
        }
        sim.add_static_mod(
            unit,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::SHADOW,
                float_value: spell_data().darkness.fraction_at(points),
                kind: SpellModType::DamageDonePct,
                ..SpellModConfig::default()
            },
        );
    }

    /// The beta client's 15473: +10% Shadow damage, -50% Shadow mana cost, +100% Shadow
    /// critical strike damage bonus, -15% Physical damage taken. Only healing is blocked, so
    /// Smite and Holy Fire stay castable inside it and do not break it.
    fn apply_shadowform(&mut self, sim: &mut Sim, unit: UnitId) {
        if !self.talents.bool("shadowform") {
            return;
        }
        let rank = spell_data().shadowform.highest();

        let pre_shadowform = self.pre_shadowform;
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Shadowform".to_string(),
                action_id: Some(ActionId::spell(rank.id)),
                duration: NEVER_EXPIRES,
                on_reset: Some(Rc::new(move |sim: &mut Sim, aura| {
                    if pre_shadowform {
                        sim.activate(aura);
                    }
                })),
                events: EventCallbacks {
                    on_cast_complete: true,
                    ..EventCallbacks::default()
                },
                ..AuraConfig::default()
            },
        );
        self.shadowform_aura = Some(aura);
        sim.attach_spell_mod(
            aura,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::SHADOW,
                float_value: rank
                    .effect(dbcenums::A_MOD_DAMAGE_PERCENT_DONE, 32)
                    .average(CHARACTER_LEVEL)
                    / 100.0,
                kind: SpellModType::DamageDonePct,
                ..SpellModConfig::default()
            },
        );
        sim.attach_spell_mod(
            aura,
            SpellModConfig {
                class_mask: masks::ALL,
                school: school::SHADOW,
                float_value: rank
                    .effect(dbcenums::A_MOD_POWER_COST_SCHOOL_PCT, 32)
                    .average(CHARACTER_LEVEL)
                    / 100.0,
                kind: SpellModType::PowerCostPct,
                ..SpellModConfig::default()
            },
        );
        sim.attach_spell_mod(
            aura,
            SpellModConfig {
                class_mask: masks::MIND_BLAST
                    | masks::MIND_FLAY
                    | masks::SHADOW_WORD_PAIN
                    | masks::DEVOURING_PLAGUE
                    | masks::SHADOW_WORD_DEATH,
                float_value: rank
                    .effect(
                        dbcenums::A_ADD_PCT_MODIFIER,
                        dbcenums::SPELLMOD_CRIT_DAMAGE_BONUS,
                    )
                    .average(CHARACTER_LEVEL)
                    / 100.0,
                kind: SpellModType::CritMultiplierFlat,
                ..SpellModConfig::default()
            },
        );
        sim.attach_multiplicative_pseudo_stat_buff(
            aura,
            PseudoStatField::SchoolDamageTakenMultiplier(SchoolIndex::Physical),
            1.0 + rank
                .effect(dbcenums::A_MOD_DAMAGE_PERCENT_TAKEN, 1)
                .average(CHARACTER_LEVEL)
                / 100.0,
        );

        let mana = rank.mana_cost();
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: ActionId::spell(rank.id),
                spell_school: school::SHADOW,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::APL,
                class_spell_mask: masks::SHADOWFORM,
                cost: CostOptions {
                    mana_base_cost_percent: mana.base_cost_percent,
                    mana_flat_cost: mana.flat_cost,
                    ..CostOptions::default()
                },
                cast: CastConfig {
                    default_cast: Cast {
                        gcd: GCD_DEFAULT,
                        ..Cast::default()
                    },
                    ..CastConfig::default()
                },
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
    }
}
