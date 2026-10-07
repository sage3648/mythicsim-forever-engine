//! Go sim/core/buffs.go and buffs_gen_support.go: the helpers the buff drivers build on.
//!
//! What a fight does with them (the pending actions that recast a shout, the cooldown manager
//! casting an external cooldown, the damage a shield deals back) is not part of preparation. What
//! preparation sees is the auras, spells, timers, exclusive effects and callbacks they register.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;

use super::super::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use super::super::character::MajorCooldown;
use super::super::sim::{
    AuraConfig, AuraId, BuildPhase, Cooldown, Duration, Sim, SpellId, UnitId, MILLISECOND,
    NEVER_EXPIRES, SECOND,
};
use super::super::spell::{CastConfig, DefenseType, ProcMask, SpellConfig, SpellFlag};
use super::super::stats::Stat;

/// Go `StatBuffCategory`: flat stat buffs that do not stack with each other.
#[allow(dead_code)]
pub(crate) const STAT_BUFF_CATEGORY: &str = "StatBuff";

/// Go `CooldownPriorityDefault`.
const COOLDOWN_PRIORITY_DEFAULT: i32 = 0;

/// Go `ApplyFixedShoutAura`: a shout the party keeps up. Behind the player's own shout it chains
/// instead (each time the player's shout gains, this one comes back a reaction time after it
/// runs out, which is a pending action and so leaves no trace here), and it is otherwise rolled
/// by `ApplyFixedUptimeAura`.
pub(crate) fn apply_fixed_shout_aura(sim: &mut Sim, unit: UnitId, aura: AuraId, category: &str) {
    let category = category.to_string();
    sim.apply_on_init(
        aura,
        Rc::new(move |sim: &mut Sim, _| {
            let own = sim.auras_with_tag(unit, &category).into_iter().find(|id| {
                sim.aura(*id)
                    .action_id
                    .as_ref()
                    .map_or(0, |action| action.tag)
                    == 0
            });
            let Some(own) = own else {
                return;
            };
            sim.apply_on_gain(own, Rc::new(|_: &mut Sim, _| {}));
        }),
    );
    let duration = sim.aura(aura).duration;
    sim.apply_fixed_uptime_aura(aura, 1.0, duration + 1, -1);
}

/// Go `InspirationAura`. Client 1.60.1.70009: the talent 14892's rank curve reads 8, 17 and 25%
/// armor, the buff is 14893 for 15 seconds.
#[allow(dead_code)]
pub(crate) fn inspiration_aura(sim: &mut Sim, unit: UnitId, points: usize) -> AuraId {
    let multiplier = 1.0 + [0.0, 0.08, 0.17, 0.25][points];
    let armor = sim.new_dynamic_multiply_stat(unit, Stat::Armor, multiplier);
    let aura = sim.get_or_register_aura(
        unit,
        AuraConfig {
            label: "Inspiration".to_string(),
            action_id: Some(ActionId::spell(14893)),
            duration: 15 * SECOND,
            ..AuraConfig::default()
        },
    );
    sim.attach_stat_dependency(aura, armor)
}

/// Go `ApplyInspiration`.
#[allow(dead_code)]
pub(crate) fn apply_inspiration(sim: &mut Sim, unit: UnitId, uptime: f64) {
    if uptime <= 0.0 {
        return;
    }
    let uptime = uptime.min(1.0);
    let aura = inspiration_aura(sim, unit, 3);
    sim.apply_fixed_uptime_aura(aura, uptime, 2500 * MILLISECOND, 1);
}

/// Go `externalConsecutiveCDApproximation`, without the closures that only run in a fight.
pub(crate) struct ExternalConsecutiveCd {
    pub action_id: ActionId,
    pub cooldown_type: u32,
    pub aura_duration: Duration,
    pub related_self_buff: Option<AuraId>,
}

/// Go `registerExternalConsecutiveCDApproximation`: approximates a cooldown other players apply to
/// this character, such as Innervate and Power Infusion, with `num_sources` of them taking turns.
pub(crate) fn register_external_consecutive_cd_approximation(
    sim: &mut Sim,
    unit: UnitId,
    config: ExternalConsecutiveCd,
    num_sources: i32,
) -> SpellId {
    assert!(num_sources != 0, "Need at least 1 source!");
    for _ in 0..num_sources {
        sim.new_timer(unit);
    }
    let shared_timer = sim.new_timer(unit);
    let spell = sim.register_spell(
        unit,
        SpellConfig {
            action_id: config.action_id,
            flags: SpellFlag::NO_ON_CAST_COMPLETE | SpellFlag::NO_METRICS | SpellFlag::NO_LOGS,
            cast: CastConfig {
                cd: Cooldown {
                    timer: Some(shared_timer),
                    // Assumes that multiple buffs are different sources.
                    duration: config.aura_duration,
                },
                ..CastConfig::default()
            },
            has_extra_cast_condition: true,
            related_self_buff: config.related_self_buff,
            ..SpellConfig::default()
        },
    );
    sim.add_major_cooldown(
        unit,
        MajorCooldown {
            spell,
            priority: COOLDOWN_PRIORITY_DEFAULT,
            cooldown_type: config.cooldown_type,
            allow_spell_queueing: false,
            timings: Vec::new(),
        },
    );
    spell
}

/// Go `GeneratedExternalCD`, without the activation condition a fight evaluates.
pub(crate) struct GeneratedExternalCd {
    pub num_sources: i32,
    pub cooldown_type: u32,
}

/// Go `NewGeneratedExternalCD`: the external cooldown around a generated aura, approximated by
/// `num_sources` casters taking turns.
pub(crate) fn new_generated_external_cd(
    sim: &mut Sim,
    unit: UnitId,
    aura: AuraId,
    config: GeneratedExternalCd,
) {
    if config.num_sources == 0 {
        return;
    }
    // The external caster's copy of a permanent buff is up while the character sheet is
    // measured; a cooldown is not, so it keeps its effects out of the stats the build phase
    // collects.
    sim.aura_mut(aura).build_phase = BuildPhase::NONE;
    let action_id = sim
        .aura(aura)
        .action_id
        .clone()
        .unwrap_or_default();
    let aura_duration = sim.aura(aura).duration;
    register_external_consecutive_cd_approximation(
        sim,
        unit,
        ExternalConsecutiveCd {
            action_id,
            cooldown_type: config.cooldown_type,
            aura_duration,
            related_self_buff: Some(aura),
        },
        config.num_sources,
    );
}

/// What `NewDamageShield` is given.
pub(crate) struct DamageShield<'a> {
    pub label: String,
    pub action_id: ActionId,
    pub duration: Duration,
    pub category: &'a str,
    pub single_aura: bool,
    pub school: u8,
    pub damage: f64,
    pub bonus_coefficient: f64,
}

/// Go `NewDamageShield`: the damage a damage shield deals back to whoever lands a melee hit,
/// which also scales with the wearer's spell power by the bonus coefficient. The category is the
/// aura's own, which decides whether a second copy of the shield can sit next to it.
pub(crate) fn new_damage_shield(sim: &mut Sim, unit: UnitId, shield: DamageShield) -> AuraId {
    sim.register_spell(
        unit,
        SpellConfig {
            action_id: ActionId {
                tag: shield.action_id.tag + 2,
                ..shield.action_id.clone()
            },
            spell_school: shield.school,
            defense_type: DefenseType::default(),
            proc_mask: ProcMask::EMPTY,
            flags: SpellFlag::BINARY | SpellFlag::PASSIVE_SPELL,
            damage_multiplier: 1.0,
            threat_multiplier: 1.0,
            bonus_coefficient: shield.bonus_coefficient,
            ..SpellConfig::default()
        },
    );
    let build_phase = if shield.action_id.tag == -1 {
        BuildPhase::BUFFS
    } else {
        BuildPhase::NONE
    };
    let aura = sim.get_or_register_aura(
        unit,
        AuraConfig {
            label: shield.label.clone(),
            action_id: Some(shield.action_id),
            duration: if shield.duration > 0 {
                shield.duration
            } else {
                NEVER_EXPIRES
            },
            build_phase,
            ..AuraConfig::default()
        },
    );
    sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: format!("{} Damage", shield.label),
            callback: CallbackMask::ON_SPELL_HIT_TAKEN,
            outcome: HitOutcome::LANDED,
            ..ProcTrigger::default()
        },
    );
    // The shield has no stat to apply or remove: what the category decides is which aura keeps
    // its proc trigger, so the damage is the whole bid.
    if !shield.category.is_empty() {
        sim.new_exclusive_effect(
            aura,
            shield.category,
            shield.single_aura,
            shield.damage,
            None,
            None,
        );
    }
    aura
}

/// Go `JoinSharedCategory`: the second category the aura joins without an effect of its own,
/// which is how the paladin auras exclude each other across schools. Only the player's own copy
/// joins it: the external copy has to be able to sit next to the one the player casts.
pub(crate) fn join_shared_category(sim: &mut Sim, aura: AuraId, shared: &str, is_player: bool) {
    if shared.is_empty() || !is_player {
        return;
    }
    sim.new_exclusive_effect(aura, shared, true, 0.0, None, None);
}

/// Go `AddGeneratedFlatBonus`: raises a generated buff the client states as worth `base` to
/// `base + bonus`, in both of the numbers a buff that does not stack keeps apart: what it applies
/// and what it bids for its category. Three pieces of the warrior's tier 2 set are worth 30 more
/// attack power on Battle Shout, and a shout worth 169 has to outbid one worth 139.
///
/// A call against an aura that already carries the bonus does nothing. It writes the priority
/// directly, which is only safe before the fight: every caller runs while the character is being
/// built.
pub(crate) fn add_generated_flat_bonus(
    sim: &mut Sim,
    aura: AuraId,
    stat: Stat,
    base: f64,
    bonus: f64,
) {
    let (label, tag, max_stacks) = {
        let aura = sim.aura(aura);
        (aura.label.clone(), aura.tag.clone(), aura.max_stacks)
    };
    assert!(
        max_stacks <= 0,
        "a stacking aura re-prices its category effect on every stack, which would drop the bonus: {label}"
    );
    for effect in sim.aura(aura).exclusive_effects.clone() {
        let category = sim.effects[effect.0].category;
        if sim.categories[category.0].name != tag {
            continue;
        }
        assert!(
            sim.categories[category.0].single_aura,
            "a category that holds more than one aura cannot tell a flat bonus when to apply: {label}"
        );
        let priority = sim.effects[effect.0].priority;
        if priority == base + bonus {
            return;
        }
        assert!(
            priority == base,
            "{label} bids {priority}, which is neither the {base} it is worth nor the {} the bonus makes it",
            base + bonus
        );
        sim.effects[effect.0].priority = base + bonus;
        sim.attach_stat_buff(aura, stat, bonus);
        return;
    }
    panic!("a flat bonus needs a buff that bids for its whole category: {label}");
}
