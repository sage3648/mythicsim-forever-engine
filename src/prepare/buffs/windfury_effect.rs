//! The exporter's Windfury Totem effect: tools/oracle-v2/melee_procs.go `meleeProcEffects` appends
//! it last, after the proc effects of items and enchants.

use serde_json::{json, Value};

use super::super::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use super::super::env::Environment;
use super::super::resolve_proc::{chance, proc_trigger};
use super::super::sim::SECOND;
use super::super::spell::ProcMask;
use super::super::spelldata::must_find;

/// Go `outcomeNames`: the names of the outcome bits a listener hears. A single bit's name is the
/// one `HitOutcome.String()` answers; a partial resist bit alone names nothing.
fn outcome_names(outcome: HitOutcome) -> Vec<&'static str> {
    (0..16)
        .map(|bit| 1u16 << bit)
        .filter(|flag| outcome.0 & flag != 0)
        .map(|flag| match flag {
            f if f == HitOutcome::MISS.0 => "Miss",
            f if f == HitOutcome::DODGE.0 => "Dodge",
            f if f == HitOutcome::PARRY.0 => "Parry",
            f if f == HitOutcome::BLOCK.0 => "Block",
            f if f == HitOutcome::GLANCE.0 => "Glance",
            f if f == HitOutcome::CRIT.0 => "Crit",
            f if f == HitOutcome::HIT.0 => "Hit",
            f if f == HitOutcome::CRUSH.0 => "Crush",
            _ => "Empty",
        })
        .collect()
}

/// Go `callbackNames`.
fn callback_names(callback: CallbackMask) -> Vec<&'static str> {
    [
        (CallbackMask::ON_SPELL_HIT_DEALT, "on_spell_hit_dealt"),
        (CallbackMask::ON_SPELL_HIT_TAKEN, "on_spell_hit_taken"),
        (
            CallbackMask::ON_PERIODIC_DAMAGE_DEALT,
            "on_periodic_damage_dealt",
        ),
        (CallbackMask::ON_HEAL_DEALT, "on_heal_dealt"),
        (
            CallbackMask::ON_PERIODIC_HEAL_DEALT,
            "on_periodic_heal_dealt",
        ),
        (CallbackMask::ON_CAST_COMPLETE, "on_cast_complete"),
        (CallbackMask::ON_APPLY_EFFECTS, "on_apply_effects"),
        (
            CallbackMask::ON_PERIODIC_DAMAGE_TAKEN,
            "on_periodic_damage_taken",
        ),
    ]
    .into_iter()
    .filter(|(mask, _)| callback.matches(*mask))
    .map(|(_, name)| name)
    .collect()
}

/// Go `procTriggerSpells`: the positions in the spellbook of the spells a trigger hears.
fn proc_trigger_spells(env: &Environment, trigger: &ProcTrigger) -> Vec<usize> {
    env.sim
        .unit(env.player)
        .spellbook
        .iter()
        .enumerate()
        .filter(|(_, spell)| trigger.matches_spell(env.sim.spell(**spell)))
        .map(|(position, _)| position)
        .collect()
}

/// tools/oracle-v2/melee_procs.go `windfuryTriggerSpells`: the spells a Windfury Totem trigger
/// hears once `buffs.NarrowWindfuryProc` has narrowed it to the main hand, as the totem's driver
/// does (`WindfuryTotemMainHandOnly`). The narrowing keeps main hand attacks and off hand
/// specials, and its extra condition refuses an off hand special when the character wields an
/// off hand weapon: a shield strike is a main hand attack to the server.
pub(crate) fn windfury_trigger_spells(env: &Environment, trigger: &ProcTrigger) -> Vec<usize> {
    let mut trigger = trigger.clone();
    trigger.proc_mask =
        ProcMask(trigger.proc_mask.0 & (ProcMask::MELEE_MH.0 | ProcMask::MELEE_OH_SPECIAL.0));
    let has_oh_weapon = env.sim.oh_weapon(env.player).is_some();
    env.sim
        .unit(env.player)
        .spellbook
        .iter()
        .enumerate()
        .filter(|(_, spell)| {
            let spell = env.sim.spell(**spell);
            trigger.matches_spell(spell)
                && !(spell.proc_mask.matches(ProcMask::MELEE_OH_SPECIAL) && has_oh_weapon)
        })
        .map(|(position, _)| position)
        .collect()
}

/// buffs/drivers.go `driveWindfuryTotem`: the totem aura, refreshed every 5 seconds, holds a
/// trigger that can grant charges of attack power and cast an extra main hand attack; the
/// charges are spent by landed autos. Triggers resolve from client rows as Go resolves them.
///
/// `meleeProcEffects` has to call this where Go does, as its last effect.
#[allow(dead_code)]
pub(crate) fn windfury_totem_effect(
    env: &Environment,
    unrepresented: &mut Vec<String>,
) -> Option<Value> {
    let player = env.player;
    let sim = &env.sim;
    let totem = sim.get_aura(player, "Windfury Totem")?;
    let proc_aura = sim.get_aura(player, "Windfury Totem (External)");
    let trigger_aura = sim.get_aura(player, "Windfury Totem Trigger");
    // The extra attack a caster registers from an empty main hand configuration has no
    // OtherActionAttack, so it is not found here.
    let extra = sim.unit(player).spellbook.iter().rposition(|spell| {
        let id = &sim.spell(*spell).action_id;
        id.other_id == "OtherActionAttack" && id.tag == 25584 && id.spell_id == 0 && id.item_id == 0
    });
    // Since the 16 October 2026 beta notes the party's Grace of Air stands beside Windfury Totem:
    // no air totem slot joins them.
    let grant = proc_trigger(sim, Some(player), must_find(10612), &[]);
    let spend = proc_trigger(sim, Some(player), must_find(10610), &[chance(1.0)]);
    let grant_spells = windfury_trigger_spells(env, &grant);
    // Without melee autos Go registers no extra attack, and nothing the character casts can
    // trigger the totem then, as for a caster.
    let (Some(proc_aura), Some(trigger_aura)) = (proc_aura, trigger_aura) else {
        unrepresented.push("Windfury Totem is incomplete".to_string());
        return None;
    };
    if extra.is_none() && !grant_spells.is_empty() {
        unrepresented.push("Windfury Totem is incomplete".to_string());
        return None;
    }
    let grant_callbacks = callback_names(grant.callback);
    let spend_callbacks = callback_names(spend.callback);
    if grant.dpm.is_some()
        || grant_callbacks != ["on_spell_hit_dealt"]
        || spend_callbacks != ["on_spell_hit_dealt"]
    {
        unrepresented.push("Windfury Totem's triggers listen to other callbacks".to_string());
    }
    let mut effect = json!({
        "kind": "windfury_totem", "totem_aura": sim.aura(totem).label, "period_ns": 5 * SECOND,
        "trigger_aura": sim.aura(trigger_aura).label, "trigger_spells": grant_spells,
        "trigger_outcome": outcome_names(grant.outcome), "trigger_proc_chance": grant.proc_chance,
        "proc_aura": sim.aura(proc_aura).label, "spend_spells": proc_trigger_spells(env, &spend),
        "spend_outcome": outcome_names(spend.outcome),
        "trigger_require_damage": grant.require_damage_dealt,
        "spend_require_damage": spend.require_damage_dealt,
    });
    if let Some(extra) = extra {
        effect["extra_attack_spell"] = json!(extra);
    }
    Some(effect)
}
