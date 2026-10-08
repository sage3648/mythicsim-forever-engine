//! The buffs other players cast on the player on cooldown: tools/oracle-v2/external_cooldowns.go,
//! from core/buffs.go `registerExternalConsecutiveCDApproximation` through buffs/drivers.go,
//! which wraps a generated aura in `NewGeneratedExternalCD`. Priests in the raid cast Power
//! Infusion (`drivePowerInfusions`), druids cast Innervate (`driveInnervates`) and shamans drop
//! Mana Tide Totem (`driveManaTideTotems`), each with a number of sources taking turns. What
//! differs between the buffs is when the major cooldown manager may cast (`ShouldActivate`) and
//! what the aura does: Power Infusion's multipliers (`power_infusion.rs`), Innervate's spirit
//! regeneration (`AttachInnervateRegen`) and Mana Tide Totem's mana per 5 seconds, a stat the
//! stat auras carry.

use serde_json::{json, Value};

use crate::contracts::prepared_v2::{ActionId, ExternalActivation};

use super::buffs::drivers::INNERVATE_SPIRIT_REGEN_MULTIPLIER;
use super::buffs::generated::{INNERVATES, MANA_TIDE_TOTEMS, POWER_INFUSIONS};
use super::env::Environment;
use super::sim::{AuraId, SECOND};
use super::stats::Stat;

/// buffs/drivers.go `innervateRegenTag`: a Go literal.
const INNERVATE_REGEN_TAG: i32 = -2;

/// buffs/drivers.go `driveManaTideTotems`: the totem waits until the party has mana to refill,
/// which is 40 seconds in, or halfway through a fight shorter than that.
const MANA_TIDE_INITIAL_DELAY_CAP: i64 = 40 * SECOND;

/// buffs/drivers.go `innervateManaThreshold`: a mage burns mana fast enough that waiting for a
/// flat thousand left would waste most of the innervate.
pub(crate) fn innervate_mana_threshold(env: &Environment) -> f64 {
    if env.sim.character(env.player).class == "ClassMage" {
        env.sim.max_mana(env.player) * 0.4
    } else {
        1000.0
    }
}

/// buffs/drivers.go `driveManaTideTotems`: no sooner than halfway through the fight, or 40
/// seconds in.
pub(crate) fn mana_tide_initial_delay(env: &Environment) -> i64 {
    (env.encounter.duration / 2).min(MANA_TIDE_INITIAL_DELAY_CAP)
}

/// One generated external cooldown: the aura's label, how many sources the request assigns, the
/// buff's cooldown and what its aura adds.
struct External {
    label: &'static str,
    sources: i32,
    cooldown: i64,
    aura_effects: fn(&Environment, AuraId, &mut Vec<String>) -> Vec<Value>,
}

/// `externalCooldowns`: the cooldowns in the order `applyGeneratedBuffs` drives them, Mana Tide
/// Totem from the party buffs, then Innervate and Power Infusion from the individual buffs.
fn externals(env: &Environment) -> [External; 3] {
    let individual = env.sim.character(env.player).player.message("buffs");
    let count = |name: &str| individual.map_or(0, |buffs| buffs.i32(name));
    [
        External {
            label: "Mana Tide Totem (External)",
            sources: env.party_buffs.i32("mana_tide_totems"),
            cooldown: MANA_TIDE_TOTEMS.cooldown(),
            aura_effects: mana_tide_aura_effects,
        },
        External {
            label: "Innervates (External)",
            sources: count("innervates"),
            cooldown: INNERVATES.cooldown(),
            aura_effects: innervate_aura_effects,
        },
        External {
            label: "Power Infusions (External)",
            sources: count("power_infusions"),
            cooldown: POWER_INFUSIONS.cooldown(),
            aura_effects: power_infusion_aura_effects,
        },
    ]
}

/// `externalCooldownEffects`: the external cooldown of each buff the request assigns, and the
/// effects of its aura.
pub(crate) fn external_cooldown_effects(
    env: &Environment,
    unrepresented: &mut Vec<String>,
) -> Vec<Value> {
    let player = env.player;
    let mut effects = Vec::new();
    for external in externals(env) {
        let Some(aura) = env.sim.get_aura(player, external.label) else {
            continue;
        };
        let mut note = |condition: bool, message: String| {
            if condition {
                unrepresented.push(message);
            }
        };
        let state = env.sim.aura(aura);
        let action = state.action_id.clone().unwrap_or_default();
        let cast = env
            .sim
            .unit(player)
            .spellbook
            .iter()
            .copied()
            .find(|spell| {
                let spell = env.sim.spell(*spell);
                spell.related_self_buff == Some(aura) && spell.action_id == action
            })
            .map(|spell| env.sim.spell(spell));
        let Some(cast) = cast.filter(|cast| cast.cd.timer.is_some() && external.sources > 0) else {
            note(true, format!("{} has no cooldown spell", external.label));
            continue;
        };
        note(
            cast.cd.duration != state.duration,
            format!("{}'s cooldown is not its aura's duration", external.label),
        );
        let mut effect = json!({
            "kind": "external_cooldown", "spell_id": cast.action_id.spell_id,
            "spell_tag": cast.action_id.tag, "aura": state.label, "aura_tag": state.tag,
            "sources": external.sources, "cooldown_ns": external.cooldown,
            "duration_ns": state.duration,
        });
        if let Some(activation) = env
            .external_activations
            .iter()
            .find_map(|(id, activation)| (*id == aura).then_some(*activation))
        {
            effect["activation"] =
                serde_json::to_value(activation).expect("an activation serializes");
        }
        effects.push(effect);
        effects.extend((external.aura_effects)(env, aura, unrepresented));
    }
    effects
}

/// `innervateAuraEffects`: `AttachInnervateRegen` forces full spirit regeneration at five times
/// the rate while the aura is up, and credits its mana to regeneration metrics of their own. A
/// separate reset simulation checks the aura changes nothing else.
fn innervate_aura_effects(env: &Environment, aura: AuraId, notes: &mut Vec<String>) -> Vec<Value> {
    let state = env.sim.aura(aura);
    let label = state.label.clone();
    let mut regen = state.action_id.clone().unwrap_or_default();
    regen.tag = INNERVATE_REGEN_TAG;
    let mut fresh = env.fresh();
    let player = fresh.player;
    let before = fresh.sim.unit(player).pseudo_stats.clone();
    let before_stats = fresh.sim.unit(player).stats;
    let fresh_aura = fresh
        .sim
        .get_aura(player, &label)
        .expect("the fresh simulation has the same auras");
    fresh.sim.activate(fresh_aura);
    let mut after = fresh.sim.unit(player).pseudo_stats.clone();
    let mut note = |condition: bool, message: &str| {
        if condition {
            notes.push(message.to_string());
        }
    };
    note(
        !after.force_full_spirit_regen
            || after.spirit_regen_multiplier
                != before.spirit_regen_multiplier * INNERVATE_SPIRIT_REGEN_MULTIPLIER,
        "Innervate does not force full spirit regeneration at five times the rate",
    );
    after.force_full_spirit_regen = before.force_full_spirit_regen;
    after.spirit_regen_multiplier = before.spirit_regen_multiplier;
    note(
        after != before || fresh.sim.unit(player).stats != before_stats,
        "Innervate changes more than spirit regeneration",
    );
    vec![json!({
        "kind": "innervate_regen", "aura": label,
        "spirit_regen_multiplier": INNERVATE_SPIRIT_REGEN_MULTIPLIER,
        "regen_metrics_action_id": regen_id(&regen),
    })]
}

fn regen_id(id: &ActionId) -> Value {
    serde_json::to_value(id).expect("an action id serializes")
}

/// `manaTideAuraEffects`: the aura is the mana per 5 seconds the totem's row states, which the
/// stat auras carry. A separate reset simulation checks it changes that stat alone, in a
/// category of its own.
fn mana_tide_aura_effects(env: &Environment, aura: AuraId, notes: &mut Vec<String>) -> Vec<Value> {
    let mut note = |condition: bool, message: &str| {
        if condition {
            notes.push(message.to_string());
        }
    };
    for effect in &env.sim.aura(aura).exclusive_effects {
        let category = env.sim.effects[effect.0].category;
        note(
            env.sim.categories[category.0].effects.len() != 1,
            "Mana Tide Totem shares its category",
        );
    }
    let label = env.sim.aura(aura).label.clone();
    let mut fresh = env.fresh();
    let player = fresh.player;
    let before = fresh.sim.unit(player).pseudo_stats.clone();
    let before_stats = fresh.sim.unit(player).stats;
    let fresh_aura = fresh
        .sim
        .get_aura(player, &label)
        .expect("the fresh simulation has the same auras");
    fresh.sim.activate(fresh_aura);
    let mut after = fresh.sim.unit(player).stats;
    note(
        after[Stat::MP5] == before_stats[Stat::MP5],
        "Mana Tide Totem does not add mana per 5 seconds",
    );
    after[Stat::MP5] = before_stats[Stat::MP5];
    note(
        after != before_stats || fresh.sim.unit(player).pseudo_stats != before,
        "Mana Tide Totem changes more than mana per 5 seconds",
    );
    Vec::new()
}

/// `powerInfusionAuraEffects`: the aura is the generated external copy, whose multipliers
/// `power_infusion.rs` describes; the runtime rolls the healing multiplier back by division,
/// which is exact only when it is 1.
fn power_infusion_aura_effects(
    env: &Environment,
    aura: AuraId,
    notes: &mut Vec<String>,
) -> Vec<Value> {
    let healing = env
        .sim
        .unit(env.player)
        .pseudo_stats
        .healing_dealt_multiplier;
    if healing != 1.0 {
        notes.push(format!(
            "the external Power Infusion on a healing dealt multiplier of {healing}"
        ));
    }
    let action = env.sim.aura(aura).action_id.clone().unwrap_or_default();
    super::power_infusion::power_infusion_effect(env, &action, notes)
        .into_iter()
        .collect()
}

/// The activation `driveInnervates` computes once the environment finalizes.
pub(crate) fn innervate_activation(env: &Environment) -> ExternalActivation {
    ExternalActivation::ManaAtMost {
        threshold: innervate_mana_threshold(env),
    }
}

/// The activation `driveManaTideTotems` computes once the environment finalizes.
pub(crate) fn mana_tide_activation(env: &Environment) -> ExternalActivation {
    ExternalActivation::NotBefore {
        time_ns: mana_tide_initial_delay(env),
    }
}
