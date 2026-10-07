//! Go sim/common/shared/shared_utils.go, spell_data_energize.go and spell_data_aura.go: the
//! constructors that register an on-use item from the item database's effect entries and the
//! client's rows.
//!
//! As in `shared_items`, each constructor is the registration decision plus what the closure does
//! to the character, and the closures that only run in a fight are not carried.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::contracts::request::Message;
use crate::data::spells::Spell;

use super::aura_helpers::{ProcTrigger, StackingStatAura, StatBuffAura};
use super::character::{cooldown_type, MajorCooldown};
use super::dbcenums;
use super::env::Environment;
use super::items::{self, effect_stats};
use super::resolve_aura::{aura_config, dot_config};
use super::resolve_spell::{self, spell_config};
use super::shared_auras::{in_area, spell_data_on_use_aura_spell};
use super::shared_items::{
    dpm_for_mask, spell_data_absorb_spell, spell_data_proc_damage_spell,
    spell_data_proc_heal_spell, EffectSource,
};
use super::sim::{AuraConfig, AuraId, Cooldown, Duration, Sim, UnitId, MILLISECOND, NEVER_EXPIRES};
use super::spell::{CastConfig, ProcMask, SpellConfig, SpellFlag};
use super::spelldata::{find, must_find};
use super::stats::Stats;

/// The on-use constructors, in Go's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OnUseKind {
    /// `NewSimpleStatActive`.
    SimpleStatActive,
    /// `NewSpellDataDamageOnUse`.
    Damage,
    /// `NewSpellDataHealOnUse`.
    Heal,
    /// `NewSpellDataAbsorbOnUse`.
    Absorb,
    /// `NewSpellDataSpeedOnUse`.
    Speed,
    /// `NewSpellDataAuraOnUse`.
    Aura,
    /// `NewSpellDataEnergizeOnUse`.
    Energize,
}

/// Go `itemEffectsFor`: the effects an on-use helper works from.
pub(crate) fn item_effects_for(item_id: i32) -> Vec<Message> {
    let item =
        items::database_item(item_id).unwrap_or_else(|| panic!("No item with ID: {item_id}"));
    if item.item_effects.is_empty() {
        panic!("No effects data for item with ID: {item_id}");
    }
    item.item_effects
}

/// Go `onUseEffectsFor`.
pub(crate) fn on_use_effects_for(item_id: i32) -> Vec<Message> {
    let effects: Vec<Message> = item_effects_for(item_id)
        .into_iter()
        .filter(|effect| effect.has("on_use"))
        .collect();
    if effects.is_empty() {
        panic!("No active effects found for item with ID: {item_id}!");
    }
    effects
}

/// Go `sharedCooldown`: share a cooldown only when the effect says it belongs to a category.
/// One with no category shares nothing, and putting it on a generic trinket timer would gate it
/// against unrelated items.
pub(crate) fn shared_cooldown(sim: &mut Sim, unit: UnitId, effect: &Message) -> Cooldown {
    let Some(on_use) = effect.message("on_use") else {
        return Cooldown::default();
    };
    if on_use.i32("category_id") <= 0 {
        return Cooldown::default();
    }
    let mut duration = i64::from(on_use.i32("category_cooldown_ms")) * MILLISECOND;
    if duration <= 0 {
        duration = i64::from(effect.i32("effect_duration_ms")) * MILLISECOND;
    }
    Cooldown {
        timer: Some(sim.category_timer(unit, on_use.i32("category_id"))),
        duration,
    }
}

/// Go `onUseCast`: the item effect's own cooldown and the category cooldown it shares, whatever
/// the spell's row states.
pub(crate) fn on_use_cast(sim: &mut Sim, unit: UnitId, effect: &Message) -> CastConfig {
    let cooldown_ms = effect
        .message("on_use")
        .map_or(0, |on_use| on_use.i32("cooldown_ms"));
    let timer = sim.new_timer(unit);
    let cd = Cooldown {
        timer: Some(timer),
        duration: i64::from(cooldown_ms) * MILLISECOND,
    };
    let shared_cd = shared_cooldown(sim, unit, effect);
    CastConfig {
        cd,
        shared_cd,
        ..CastConfig::default()
    }
}

/// Go `onUseStatBuff`: the on-use buff's stats and duration, scaled by its area bonus.
pub(crate) fn on_use_stat_buff(env: &Environment, effect: &Message) -> (Stats, Duration) {
    let (amount, duration) = find(effect.i32("buff_id")).area_bonus(|area| in_area(env, area));
    let stats = effect_stats(effect).multiply(amount);
    let millis = (f64::from(effect.i32("effect_duration_ms")) * duration) as i64;
    (stats, millis * MILLISECOND)
}

/// Go `registerSpellDataOnUseCooldown`'s decision: an on-use constructor registers an effect for
/// every item it is called for.
pub(crate) fn registers(_kind: OnUseKind, _item_id: i32) -> bool {
    true
}

/// Applies the on-use effect a constructor registers for the item to the player.
pub(crate) fn apply(env: &mut Environment, kind: OnUseKind, item_id: i32) {
    match kind {
        OnUseKind::SimpleStatActive => apply_simple_stat_active(env, item_id),
        OnUseKind::Damage => register_spell_data_on_use(env, item_id, |env, row| {
            Some((
                spell_data_proc_damage_spell(env, row, false),
                cooldown_type::DPS,
            ))
        }),
        OnUseKind::Heal => register_spell_data_on_use(env, item_id, |env, row| {
            Some((
                spell_data_proc_heal_spell(env, row, false),
                cooldown_type::SURVIVAL,
            ))
        }),
        OnUseKind::Absorb => register_spell_data_on_use(env, item_id, |env, row| {
            Some((
                spell_data_absorb_spell(env, row, item_id, false),
                cooldown_type::SURVIVAL,
            ))
        }),
        OnUseKind::Speed => register_spell_data_on_use(env, item_id, |env, row| {
            Some((spell_data_on_use_speed_spell(env, row), cooldown_type::DPS))
        }),
        OnUseKind::Aura => register_spell_data_on_use(env, item_id, |env, row| {
            Some((spell_data_on_use_aura_spell(env, row), cooldown_type::DPS))
        }),
        OnUseKind::Energize => register_spell_data_on_use(env, item_id, energize_on_use),
    }
}

/// Go `NewSimpleStatActive`: an on-use item whose buff carries the effect's scaling stats.
fn apply_simple_stat_active(env: &mut Environment, item_id: i32) {
    let unit = env.player;
    for effect in on_use_effects_for(item_id) {
        let cast = on_use_cast(&mut env.sim, unit, &effect);
        let config = SpellConfig {
            action_id: ActionId::item(item_id),
            cast,
            ..SpellConfig::default()
        };
        let (stats, duration) = on_use_stat_buff(env, &effect);
        env.sim.register_temporary_stats_on_use_cd(
            unit,
            effect.str("buff_name"),
            stats,
            duration,
            config,
        );
    }
}

/// Go `registerSpellDataOnUseCooldown`: the spell a proc of the same row would cast, used from
/// the item instead: it is the item's action, counts its casts, and runs on the item's cooldowns
/// rather than the row's, with the cast time and global cooldown the row states.
fn register_spell_data_on_use(
    env: &mut Environment,
    item_id: i32,
    on_use: impl Fn(&mut Environment, &'static Spell) -> Option<(SpellConfig, u32)>,
) {
    let unit = env.player;
    for effect in on_use_effects_for(item_id) {
        let row = must_find(effect.i32("buff_id"));
        let Some((mut config, cd_type)) = on_use(env, row) else {
            continue;
        };
        config.action_id = ActionId::item(item_id);

        let item_cast = on_use_cast(&mut env.sim, unit, &effect);
        config.cast = resolve_spell::cast(row);
        config.cast.cd = item_cast.cd;
        config.cast.shared_cd = item_cast.shared_cd;

        let spell = env.sim.register_spell(unit, config);
        env.sim.add_major_cooldown(
            unit,
            MajorCooldown {
                spell,
                priority: 0,
                cooldown_type: cd_type,
                allow_spell_queueing: false,
                timings: Vec::new(),
            },
        );
    }
}

/// Go `spellDataOnUseSpeedSpell`: every speed the row states is on one aura, up for the row's
/// duration.
fn spell_data_on_use_speed_spell(env: &mut Environment, row: &'static Spell) -> SpellConfig {
    let unit = env.player;
    let aura = env.sim.register_aura(unit, aura_config(row, &[]));
    env.sim
        .attach_haste_pseudo_stats(aura, &row.speed_pseudo_stats());

    let mut config = spell_config(&mut env.sim, unit, row, &[]);
    config.proc_mask = ProcMask::EMPTY;
    config.related_self_buff = Some(aura);
    config
}

/// Go `NewSpellDataEnergizeOnUse`: an on-use item whose spell restores the wearer's mana, rage or
/// energy, at once or over time, on the item's cooldowns with the cast time and global cooldown
/// the row states. A character without that bar does not register it.
fn energize_on_use(env: &mut Environment, row: &'static Spell) -> Option<(SpellConfig, u32)> {
    let unit = env.player;
    let effect = row.proc_energize_effect();
    let power = effect.misc;
    let bar = &env.sim.unit(unit);
    let cd_type = if power == dbcenums::POWER_MANA && bar.mana_bar.enabled {
        cooldown_type::MANA
    } else if power == dbcenums::POWER_RAGE && bar.rage_bar.enabled
        || power == dbcenums::POWER_ENERGY && bar.energy_bar.enabled
    {
        cooldown_type::DPS
    } else {
        return None;
    };

    let mut config = spell_config(&mut env.sim, unit, row, &[]);
    if effect.aura == dbcenums::A_PERIODIC_ENERGIZE {
        config.hot = dot_config(row, effect, &[]);
        config.hot.self_only = true;
    }
    Some((config, cd_type))
}

// ---------------------------------------------------------------------------------------------
// Stacking stat on-use items.
// ---------------------------------------------------------------------------------------------

/// Go `StackingStatBonusCD`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StackingStatBonusCd {
    pub name: &'static str,
    pub id: i32,
    pub cd: Duration,
    pub callback: super::aura_helpers::CallbackMask,
    pub proc_mask: ProcMask,
    pub spell_flags: SpellFlag,
    pub outcome: super::aura_helpers::HitOutcome,
    pub require_damage_dealt: bool,
    pub can_proc_from_procs: bool,
    pub spell_flags_exclude: SpellFlag,
    /// The stacks will only be granted as long as the trinket is active.
    pub trinket_limits_duration: bool,
}

impl StackingStatBonusCd {
    /// The zero config the generated registrations fill in.
    pub(crate) const EMPTY: StackingStatBonusCd = StackingStatBonusCd {
        name: "",
        id: 0,
        cd: 0,
        callback: super::aura_helpers::CallbackMask::EMPTY,
        proc_mask: ProcMask::UNKNOWN,
        spell_flags: SpellFlag::NONE,
        outcome: super::aura_helpers::HitOutcome::EMPTY,
        require_damage_dealt: false,
        can_proc_from_procs: false,
        spell_flags_exclude: SpellFlag::NONE,
        trinket_limits_duration: false,
    };
}

/// Go `stackingStats`: where the stacks actually live. A database-resolved stacking trinket
/// keeps the window and the stacks in two auras, so the count, the per-stack stats and the stat
/// aura's identity all come from the nested one rather than from the effect itself.
struct StackingStats {
    action_id: ActionId,
    max_stacks: i32,
    per_stack: Stats,
    window_bounded: bool,
}

fn resolve_stacking_stats(
    effect: &Message,
    effect_action_id: ActionId,
    trinket_limits_duration: bool,
) -> StackingStats {
    if let Some(stacking) = effect.message("stacking_aura") {
        return StackingStats {
            action_id: ActionId::spell(stacking.i32("buff_id")),
            max_stacks: stacking.i32("max_cumulative_stacks"),
            per_stack: effect_stats(stacking),
            window_bounded: true,
        };
    }
    StackingStats {
        action_id: effect_action_id,
        max_stacks: effect.i32("max_cumulative_stacks"),
        per_stack: effect_stats(effect),
        window_bounded: trinket_limits_duration,
    }
}

/// Go `stackingAuraID`: the aura the on-use itself applies. Effects that name no buff spell fall
/// back to the item.
fn stacking_aura_id(effect: &Message, item_id: i32) -> ActionId {
    let aura_id = ActionId::spell(effect.i32("buff_id"));
    if super::aura_helpers::is_empty_action(&aura_id) {
        return ActionId::item(item_id);
    }
    aura_id
}

/// Go `NewStackingStatBonusCD`: a stacking on-use trinket.
pub(crate) fn apply_stacking_stat_bonus_cd(env: &mut Environment, config: &StackingStatBonusCd) {
    let unit = env.player;
    for effect in item_effects_for(config.id) {
        let stacks = resolve_stacking_stats(
            &effect,
            stacking_aura_id(&effect, config.id),
            config.trinket_limits_duration,
        );
        let (stat_aura, proc_aura) = build_stacking_cd_auras(env, config, &effect, &stacks);

        attach_stacking_cd_trigger(env, config, &effect, proc_aura);

        let timer = env.sim.new_timer(unit);
        let shared_cd = shared_cooldown(&mut env.sim, unit, &effect);
        let spell = env.sim.register_spell(
            unit,
            SpellConfig {
                action_id: ActionId::item(config.id),
                flags: SpellFlag::NO_ON_CAST_COMPLETE,
                cast: CastConfig {
                    cd: Cooldown {
                        timer: Some(timer),
                        duration: config.cd,
                    },
                    shared_cd,
                    ..CastConfig::default()
                },
                related_self_buff: Some(stat_aura.aura),
                ..SpellConfig::default()
            },
        );
        env.sim.add_major_cooldown(
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

/// Go `buildStackingCDAuras`: the aura pair a stacking on-use drives. Where the window bounds the
/// stacks the stat aura is given no duration of its own and a second aura ends it on expiry;
/// otherwise the stat aura is its own window and both returns are the same object.
fn build_stacking_cd_auras(
    env: &mut Environment,
    config: &StackingStatBonusCd,
    effect: &Message,
    stacks: &StackingStats,
) -> (StatBuffAura, AuraId) {
    let unit = env.player;
    let aura_duration = i64::from(effect.i32("effect_duration_ms")) * MILLISECOND;
    let stat_aura = env.sim.make_stacking_aura(
        unit,
        StackingStatAura {
            aura: AuraConfig {
                label: format!("{} Proc", config.name),
                action_id: Some(stacks.action_id.clone()),
                duration: if stacks.window_bounded {
                    NEVER_EXPIRES
                } else {
                    aura_duration
                },
                max_stacks: stacks.max_stacks,
                ..AuraConfig::default()
            },
            bonus_per_stack: stacks.per_stack,
        },
    );
    if !stacks.window_bounded {
        let aura = stat_aura.aura;
        return (stat_aura, aura);
    }
    let limit = env.sim.register_aura(
        unit,
        AuraConfig {
            label: format!("{} Limit Aura {}", config.name, effect.str("buff_name")),
            action_id: Some(stacking_aura_id(effect, config.id)),
            duration: aura_duration,
            on_expire: Some({
                let stat = stat_aura.aura;
                Rc::new(move |sim: &mut Sim, _| sim.deactivate(stat))
            }),
            ..AuraConfig::default()
        },
    );
    (stat_aura, limit)
}

/// Go `attachStackingCDTrigger`: what moves the stack count while the window is open. Attached to
/// the window so it is live only then, and a decaying trinket spends a stack per event where the
/// rest gain one.
fn attach_stacking_cd_trigger(
    env: &mut Environment,
    config: &StackingStatBonusCd,
    effect: &Message,
    window_aura: AuraId,
) {
    let unit = env.player;
    // Rate and lockout come off the stack proc. The getters are nil-safe: an item effect that
    // carries no stack proc keeps a plain always-on trigger.
    let stack_proc = effect.message("stack_proc");
    let stack_dpm = stack_proc.and_then(|proc| {
        dpm_for_mask(
            &env.sim,
            unit,
            EffectSource {
                id: config.id,
                is_enchant: false,
            },
            proc.f64("ppm"),
            config.proc_mask,
        )
    });
    let proc_chance = if stack_dpm.is_none() {
        stack_proc.map_or(0.0, |proc| proc.f64("proc_chance"))
    } else {
        0.0
    };
    let trigger = ProcTrigger {
        name: config.name.to_string(),
        callback: config.callback,
        proc_mask: config.proc_mask,
        spell_flags: config.spell_flags,
        spell_flags_exclude: config.spell_flags_exclude,
        can_proc_from_procs: config.can_proc_from_procs,
        outcome: config.outcome,
        require_damage_dealt: config.require_damage_dealt,
        proc_chance,
        icd: i64::from(stack_proc.map_or(0, |proc| proc.i32("icd_ms"))) * MILLISECOND,
        dpm: stack_dpm,
        ..ProcTrigger::default()
    };
    env.sim
        .attach_proc_trigger_callback(window_aura, unit, &trigger);
}
