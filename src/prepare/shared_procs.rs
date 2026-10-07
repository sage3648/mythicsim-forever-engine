//! Go sim/common/shared/shared_utils.go: the procs items register by hand rather than from the
//! client's rows: `NewProcDamageEffect` and `NewProcStatBonusEffect`.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::contracts::request::Message;

use super::aura_helpers::{
    CallbackMask, HitOutcome, ProcTrigger, StackingStatAura, StatBuffAura,
    TemporaryStatBuffWithStacksConfig,
};
use super::env::Environment;
use super::items::{self, effect_stats};
use super::procs::DynamicProcManager;
use super::shared_items::{damage_defense_type, dpm_for_mask, EffectSource};
use super::sim::{AuraConfig, AuraId, Sim, UnitId, MILLISECOND};
use super::spell::{DefenseType, ProcMask, SpellConfig, SpellFlag};

/// Go `ProcDamageEffect.TriggerDPM`: the proc manager of the trigger, built per character.
pub(crate) type TriggerDpmFn = Rc<dyn Fn(&Sim, UnitId) -> Rc<DynamicProcManager>>;

/// Go `ProcDamageEffect`: a proc whose handler casts a damage spell of a Go literal range.
#[derive(Clone, Default)]
pub(crate) struct ProcDamageEffect {
    pub item_id: i32,
    pub spell_id: i32,
    pub enchant_id: i32,
    pub trigger: ProcTrigger,
    pub trigger_dpm: Option<TriggerDpmFn>,
    pub school: u8,
    /// From SpellCategories. Left unset, it is inferred from the school and `is_melee`.
    pub defense_type: DefenseType,
    pub min_dmg: f64,
    pub max_dmg: f64,
    pub bonus_coefficient: f64,
    pub is_melee: bool,
    pub flags: SpellFlag,
}

/// Go `NewProcDamageEffect`'s closure: registers the damage spell and the trigger aura that casts
/// it at once on the unit the proc answers.
pub(crate) fn apply_proc_damage_effect(env: &mut Environment, config: &ProcDamageEffect) {
    let unit = env.player;
    let trigger_action_id = if config.enchant_id != 0 {
        ActionId::spell(config.spell_id)
    } else {
        ActionId::item(config.item_id)
    };
    let defense_type = damage_defense_type(config.defense_type, config.school, config.is_melee);

    // Per-character copy of the trigger: the manager is bound to this character.
    let mut trigger = config.trigger.clone();
    if super::aura_helpers::is_empty_action(&trigger.action_id) {
        trigger.action_id = trigger_action_id;
    }
    if let Some(trigger_dpm) = &config.trigger_dpm {
        trigger.dpm = Some(trigger_dpm(&env.sim, unit));
    }

    env.sim.register_spell(
        unit,
        SpellConfig {
            action_id: ActionId::spell(config.spell_id),
            spell_school: config.school,
            defense_type,
            proc_mask: ProcMask::EMPTY,
            flags: config.flags,
            damage_multiplier: 1.0,
            threat_multiplier: 1.0,
            bonus_coefficient: config.bonus_coefficient,
            ..SpellConfig::default()
        },
    );
    trigger.trigger_immediately = true;
    env.sim.make_proc_trigger_aura(unit, &trigger);
}

/// Go `ProcStatBonusEffect`: a proc that buffs stats, from the item database's effect entry.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ProcStatBonusEffect {
    pub name: &'static str,
    pub item_id: i32,
    pub enchant_id: i32,
    pub callback: CallbackMask,
    pub proc_mask: ProcMask,
    pub outcome: HitOutcome,
    pub require_damage_dealt: bool,
    pub class_spells_only: bool,
    pub can_proc_from_procs: bool,
    pub is_weapon_proc: bool,
    pub spell_flags_exclude: SpellFlag,
    /// What adds a stack while a stacking trinket's window is open.
    pub stack_callback: CallbackMask,
    pub stack_proc_mask: ProcMask,
    pub stack_outcome: HitOutcome,
}

impl ProcStatBonusEffect {
    fn source(&self) -> EffectSource {
        EffectSource {
            id: if self.enchant_id != 0 {
                self.enchant_id
            } else {
                self.item_id
            },
            is_enchant: self.enchant_id != 0,
        }
    }
}

/// Go `buildProcAura`: the three shapes a proc buff comes in. Only the first returns a second
/// aura: there the trigger opens a window and the stat aura inside it accumulates.
fn build_proc_aura(
    env: &mut Environment,
    config: &ProcStatBonusEffect,
    effect: &Message,
) -> (StatBuffAura, Option<AuraId>) {
    let unit = env.player;
    let label = format!("{} Proc", config.name);
    let action = ActionId::spell(effect.i32("buff_id"));
    let duration = i64::from(effect.i32("effect_duration_ms")) * MILLISECOND;

    if let Some(stacking) = effect.message("stacking_aura") {
        return env.sim.new_temporary_stat_buff_with_stacks(
            unit,
            &TemporaryStatBuffWithStacksConfig {
                aura_label: label,
                action_id: action,
                duration,
                max_stacks: stacking.i32("max_cumulative_stacks"),
                bonus_per_stack: effect_stats(stacking),
                stacking_aura_action_id: ActionId::spell(stacking.i32("buff_id")),
                stacking_aura_label: format!("{} Stacks", config.name),
                time_per_stack: i64::from(effect.i32("stack_period_ms")) * MILLISECOND,
                tick_immediately: true,
                stacks_from_event: effect.has("stack_proc"),
            },
        );
    }
    if effect.i32("max_cumulative_stacks") > 0 {
        let aura = env.sim.make_stacking_aura(
            unit,
            StackingStatAura {
                aura: AuraConfig {
                    label,
                    action_id: Some(action),
                    duration,
                    max_stacks: effect.i32("max_cumulative_stacks"),
                    ..AuraConfig::default()
                },
                bonus_per_stack: effect_stats(effect),
            },
        );
        return (aura, None);
    }
    let aura =
        env.sim
            .new_temporary_stats_aura(unit, &label, &action, effect_stats(effect), duration);
    (aura, None)
}

/// Go `attachStackTrigger`: event-driven stacks come from their own trigger: the container's proc
/// flags decide what counts, and it only does anything while the window is open.
fn attach_stack_trigger(
    env: &mut Environment,
    config: &ProcStatBonusEffect,
    effect: &Message,
    window_aura: Option<AuraId>,
) {
    let unit = env.player;
    let (Some(stack_proc), Some(window_aura)) = (effect.message("stack_proc"), window_aura) else {
        return;
    };
    if config.stack_callback == CallbackMask::EMPTY {
        return;
    }
    let dpm = dpm_for_mask(
        &env.sim,
        unit,
        config.source(),
        stack_proc.f64("ppm"),
        config.stack_proc_mask,
    );
    env.sim.attach_proc_trigger_callback(
        window_aura,
        unit,
        &ProcTrigger {
            name: format!("{} Stack Trigger", config.name),
            callback: config.stack_callback,
            proc_mask: config.stack_proc_mask,
            outcome: config.stack_outcome,
            spell_flags_exclude: config.spell_flags_exclude,
            can_proc_from_procs: config.can_proc_from_procs,
            is_weapon_proc: config.is_weapon_proc,
            proc_chance: stack_proc.f64("proc_chance"),
            dpm,
            icd: i64::from(stack_proc.i32("icd_ms")) * MILLISECOND,
            ..ProcTrigger::default()
        },
    );
}

/// Go `factory_StatBonusEffect`'s closure, without the extra spell: the trigger, its buff and the
/// window a stacking trinket opens.
pub(crate) fn apply_proc_stat_bonus_effect(env: &mut Environment, config: &ProcStatBonusEffect) {
    let unit = env.player;
    let source = config.source();
    let trigger_action_id = source.action_id();
    let declared: Vec<Message> = if source.is_enchant {
        items::enchant(source.id)
            .map(|enchant| enchant.enchant_effects)
            .unwrap_or_default()
    } else {
        items::database_item(source.id)
            .map(|item| item.item_effects)
            .unwrap_or_default()
    };
    let effects: Vec<Message> = declared
        .into_iter()
        .filter(|effect| effect.has("proc"))
        .collect();
    if effects.is_empty() {
        panic!("Error getting proc effects for item/enchant {}", source.id);
    }
    for effect in effects {
        let proc = effect.message("proc");
        let (proc_aura, window_aura) = build_proc_aura(env, config, &effect);
        let ppm = proc.map_or(0.0, |proc| proc.f64("ppm"));
        let dpm = dpm_for_mask(&env.sim, unit, source, ppm, config.proc_mask);
        let icd_ms = proc.map_or(0, |proc| proc.i32("icd_ms"));

        let trigger_aura = env.sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                action_id: trigger_action_id.clone(),
                name: config.name.to_string(),
                callback: config.callback,
                proc_mask: config.proc_mask,
                spell_flags_exclude: config.spell_flags_exclude,
                outcome: config.outcome,
                require_damage_dealt: config.require_damage_dealt,
                class_spells_only: config.class_spells_only,
                can_proc_from_procs: config.can_proc_from_procs,
                is_weapon_proc: config.is_weapon_proc,
                proc_chance: proc.map_or(0.0, |proc| proc.f64("proc_chance")),
                dpm,
                icd: i64::from(icd_ms) * MILLISECOND,
                ..ProcTrigger::default()
            },
        );

        attach_stack_trigger(env, config, &effect, window_aura);

        // Carried on the stacking path too: what it feeds is the ICD-aware APL values.
        if icd_ms != 0 {
            let icd = env.sim.aura(trigger_aura).icd;
            env.sim.aura_mut(proc_aura.aura).icd = icd;
        }
    }
}
