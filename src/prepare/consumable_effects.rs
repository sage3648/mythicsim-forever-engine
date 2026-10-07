//! The effects the exporter describes for consumables: tools/oracle-v2 `commonEffects` for
//! potions, conjured items, the Goblin Sapper Charge and the basic explosives (explosives.go,
//! damage_taken.go `goblinSapperEffect`), and melee_procs.go's Dragonbreath Chili.
//!
//! Each effect is a JSON object with exactly the exporter's keys and values.

use serde_json::{json, Value};

use crate::contracts::prepared_v2::ActionId;

use super::aura_helpers::ProcTrigger;
use super::common_effects::{GOBLIN_SAPPER_ITEM, SPELL_BATCH_WINDOW};
use super::consumes::{
    basic_explosive, consumable_by_id, spell_effect_by_id, Consumable, EFFECT_TYPE_HEAL,
    EFFECT_TYPE_RESOURCE_GAIN, RESOURCE_ENERGY, RESOURCE_MANA, RESOURCE_RAGE,
};
use super::env::Environment;
use super::sim::SpellId;
use super::spell::{ProcMask, SpellFlag};
use super::stats::{Stat, Stats};

/// Go `Stats.FlatString`: `{"Name": 1.000,...}` over the non-zero stats, each followed by a
/// comma.
pub(crate) fn flat_string(stats: &Stats) -> String {
    let mut out = String::from("{");
    for stat in Stat::ALL {
        let value = stats[stat];
        if value == 0.0 {
            continue;
        }
        out.push_str(&format!("\"{}\": {:.3},", stat.name(), value));
    }
    out.push('}');
    out
}

/// Go `ActionID.String`, for an item or spell action.
pub(crate) fn action_id_string(id: &ActionId) -> String {
    let mut out = String::from("{");
    if id.spell_id != 0 {
        out.push_str(&format!("SpellID: {}", id.spell_id));
    } else if id.item_id != 0 {
        out.push_str(&format!("ItemID: {}", id.item_id));
    }
    if id.tag != 0 {
        out.push_str(&format!(", Tag: {}", id.tag));
    }
    out.push('}');
    out
}

/// The consumable cases of the item loop: potions, conjured items, the Goblin Sapper Charge and
/// the basic explosives. `None` for an item no consumable case claims.
pub(crate) fn consumable_item_effect(
    env: &mut Environment,
    spell: SpellId,
    item: i32,
    unrepresented: &mut Vec<String>,
) -> Option<Value> {
    let flags = env.sim.spell(spell).flags;
    let consumable = consumable_by_id(item);
    if consumable.id != 0 && flags.matches(SpellFlag::POTION) {
        if potion_needs_resources(&consumable) {
            return Some(potion_resource_effect(env, &consumable, unrepresented));
        }
        return Some(potion_mana_effect(env, &consumable, unrepresented));
    }
    if consumable.id != 0 && flags.matches(SpellFlag::CONJURED) {
        return Some(conjured_effect(env, &consumable, unrepresented));
    }
    if env.sim.spell(spell).action_id == ActionId::item(GOBLIN_SAPPER_ITEM) {
        return Some(goblin_sapper_effect(env, unrepresented));
    }
    if flags.matches(SpellFlag::EXPLOSIVE) && basic_explosive(item).is_some() {
        return Some(basic_explosive_effect(env, spell, unrepresented));
    }
    None
}

fn stone_multiplier(env: &Environment) -> f64 {
    if env.sim.has_alch_stone(env.player) {
        1.4
    } else {
        1.0
    }
}

/// Go `potionNeedsResources`: whether a potion restores something other than mana, or carries
/// a stat buff. The general potion_resource effect describes it, and potion_mana keeps the
/// instant mana potions.
fn potion_needs_resources(consumable: &Consumable) -> bool {
    if consumable.buff_duration > 0 {
        return true;
    }
    consumable.effect_ids.iter().any(|id| {
        spell_effect_by_id(*id)
            .is_some_and(|e| e.resource_type != 0 && e.resource_type != RESOURCE_MANA)
    })
}

/// The `potion_mana` effect: the instant mana potions of consumes.go `registerPotionCD`.
fn potion_mana_effect(
    env: &Environment,
    consumable: &Consumable,
    unrepresented: &mut Vec<String>,
) -> Value {
    let item = consumable.id;
    let mut gains = Vec::new();
    for effect_id in &consumable.effect_ids {
        let Some(e) = spell_effect_by_id(*effect_id) else {
            unrepresented.push(format!(
                "potion {item} effect {effect_id} is not an instant mana gain"
            ));
            continue;
        };
        if e.resource_type == RESOURCE_MANA
            && e.aura_period_ms == 0
            && e.kind == EFFECT_TYPE_RESOURCE_GAIN
        {
            gains.push(json!({"min": e.min_effect_size, "spread": e.effect_spread}));
        } else {
            unrepresented.push(format!(
                "potion {item} effect {effect_id} is not an instant mana gain"
            ));
        }
    }
    if consumable.buff_duration > 0 {
        unrepresented.push(format!("potion {item} has a stat buff"));
    }
    json!({
        "kind": "potion_mana", "item_id": item, "rng_label": consumable.name, "gains": gains,
        "stone_multiplier": stone_multiplier(env), "regen_window_seconds": 5.0,
    })
}

/// Go `potionResourceEffect`: the buff aura activates first, then each instant gain rolls under
/// the potion's name, times the alchemist stone multiplier.
fn potion_resource_effect(
    env: &Environment,
    consumable: &Consumable,
    unrepresented: &mut Vec<String>,
) -> Value {
    let item = consumable.id;
    let mut gains = Vec::new();
    for effect_id in &consumable.effect_ids {
        let Some(e) = spell_effect_by_id(*effect_id) else {
            continue;
        };
        let resource = e.resource_type;
        let periodic = e.aura_period_ms > 0;
        if resource != 0
            && !periodic
            && e.kind == EFFECT_TYPE_RESOURCE_GAIN
            && (resource == RESOURCE_MANA || resource == RESOURCE_RAGE)
        {
            gains.push(
                json!({"resource": e.resource_name, "min": e.min_effect_size,
                              "spread": e.effect_spread}),
            );
        } else if resource != 0 && (periodic || e.kind == EFFECT_TYPE_RESOURCE_GAIN) {
            unrepresented.push(format!(
                "potion {item} effect {effect_id} restores {} over time or a resource Rust lacks",
                e.resource_name
            ));
        } else if !e.stats.is_zero() {
            unrepresented.push(format!(
                "potion {item} effect {effect_id} applies a triggered stat aura"
            ));
        }
    }
    let mut effect = json!({
        "kind": "potion_resource", "item_id": item, "rng_label": consumable.name,
        "gains": gains, "stone_multiplier": stone_multiplier(env),
    });
    if consumable.buff_duration > 0 {
        let buffs = flat_string(&consumable.stats);
        let id = action_id_string(&ActionId::item(item));
        effect["aura"] = json!(consumable.name);
        effect["gain_log"] = json!(format!("Gained {buffs} from {id}."));
        effect["expire_log"] = json!(format!("Lost {buffs} from fading {id}."));
    }
    effect
}

/// The conjured cases of the item loop: `conjured_mana` or `conjured_energy`.
fn conjured_effect(
    env: &Environment,
    consumable: &Consumable,
    unrepresented: &mut Vec<String>,
) -> Value {
    let item = consumable.id;
    let selected = env.sim.character(env.player).consumables.i32("conjured_id") == item;
    let mut gains = Vec::new();
    let mut energy_gains = Vec::new();
    for effect_id in &consumable.effect_ids {
        let Some(e) = spell_effect_by_id(*effect_id) else {
            continue;
        };
        let resource = e.resource_type;
        if (e.kind == EFFECT_TYPE_RESOURCE_GAIN || e.kind == EFFECT_TYPE_HEAL) && resource != 0 {
            let gain = json!({"min": e.min_effect_size, "spread": e.effect_spread});
            if resource == RESOURCE_MANA {
                gains.push(gain);
            } else if resource == RESOURCE_ENERGY {
                // A class without an energy bar keeps Go's empty bar: the cast never activates.
                energy_gains.push(gain);
            } else {
                unrepresented.push(format!("conjured {item} restores {}", e.resource_name));
            }
        }
    }
    if consumable.buff_duration > 0 {
        unrepresented.push(format!("conjured {item} has a stat buff"));
    }
    if energy_gains.is_empty() {
        return json!({
            "kind": "conjured_mana", "item_id": item, "rng_label": consumable.name,
            "gains": gains, "selected": selected, "regen_window_seconds": 5.0,
        });
    }
    if !gains.is_empty() {
        unrepresented.push(format!("conjured {item} restores mana and energy"));
    }
    // Thistle Tea (9512) restores a flat 100; the cast activates once all but 10 of it fits, a
    // Go literal (consumes.go makeConjuredActivationSpellInternal).
    json!({
        "kind": "conjured_energy", "item_id": item, "rng_label": consumable.name,
        "gains": energy_gains, "selected": selected, "spill": 10.0,
    })
}

/// Go `Encounter.AOECapMultiplier` with every target active.
fn aoe_cap_multiplier(env: &Environment) -> f64 {
    (20.0 / env.encounter.targets.len() as f64).min(1.0)
}

/// explosives.go `basicExplosiveEffect`: a rolled hit on every target scaled by the AoE cap,
/// dealt after travel when the explosive flies. The spell's registered school and missile
/// speed must agree with the restated literals.
fn basic_explosive_effect(
    env: &Environment,
    spell: SpellId,
    unrepresented: &mut Vec<String>,
) -> Value {
    let s = env.sim.spell(spell);
    let explosive = basic_explosive(s.action_id.item_id).expect("a basic explosive");
    if s.action_id.tag != 0
        || s.spell_school != explosive.school
        || s.missile_speed != explosive.speed
    {
        unrepresented.push(format!(
            "explosive {} differs from its restated literals",
            action_id_string(&s.action_id)
        ));
    }
    json!({
        "kind": "basic_explosive", "item_id": s.action_id.item_id,
        "min_damage": explosive.min, "max_damage": explosive.max,
        "aoe_cap_multiplier": aoe_cap_multiplier(env),
    })
}

/// damage_taken.go `goblinSapperEffect`: a rolled Fire hit on every target, scaled by the AoE
/// cap, dealt at once, then a second roll that hits the player.
fn goblin_sapper_effect(env: &Environment, unrepresented: &mut Vec<String>) -> Value {
    let player = env.player;
    let self_tag = ActionId {
        tag: 1,
        ..ActionId::item(GOBLIN_SAPPER_ITEM)
    };
    if env.sim.get_spell(player, &self_tag).is_none() {
        unrepresented.push(format!(
            "Goblin Sapper Charge {} has no self damage spell",
            action_id_string(&ActionId::item(GOBLIN_SAPPER_ITEM))
        ));
    }
    json!({
        "kind": "goblin_sapper", "item_id": GOBLIN_SAPPER_ITEM, "self_tag": 1,
        "min_damage": 450.0, "max_damage": 750.0,
        "aoe_cap_multiplier": aoe_cap_multiplier(env),
        "self_attack_table": crate::prepare::damage_taken::self_attack_table(env, unrepresented),
    })
}

/// The spellbook positions of the spells a proc trigger listens to: tools/oracle-v2
/// `procTriggerSpells`, which mirrors core `ProcTrigger.matchesSpell`.
pub(crate) fn proc_trigger_spells(env: &Environment, trigger: &ProcTrigger) -> Vec<usize> {
    env.sim
        .unit(env.player)
        .spellbook
        .iter()
        .enumerate()
        .filter(|(_, spell)| trigger.matches_spell(env.sim.spell(**spell)))
        .map(|(position, _)| position)
        .collect()
}

/// melee_procs.go: core/consumes.go `registerDragonbreathChili`, a 5% proc on landed melee
/// hits, Go literals, whose handler waits a spell batch window and casts a rolled Fire hit.
/// The caller places it in `meleeItemListeners`' order.
pub(crate) fn dragonbreath_chili_effect(env: &Environment) -> Option<Value> {
    let aura = env.sim.get_aura(env.player, "Dragonbreath Chili")?;
    let trigger = ProcTrigger {
        proc_mask: ProcMask::MELEE,
        ..ProcTrigger::default()
    };
    Some(json!({
        "kind": "dragonbreath_chili", "trigger_aura": env.sim.aura(aura).label,
        "spell_id": 15851, "proc_chance": 0.05,
        "trigger_spells": proc_trigger_spells(env, &trigger),
        "roll_min": 57.0, "roll_max": 73.0, "delay_ns": SPELL_BATCH_WINDOW,
    }))
}
