//! The exporter's descriptions of item and enchant effects: tools/oracle-v2 `gear_procs.go`,
//! `item_procs.go`, `energy_procs.go`, `damage_on_use.go`, `melee_procs.go` and the item parts of
//! `main.go` (`meleeItemListeners`, `hitTakenItemListeners`, `survivalOnUse`, `speedOnUse`,
//! `simpleStatActive`, `onlyOnUseEffect`).
//!
//! Each function reads the registered auras and spells the way the Go exporter reads them, and
//! restates the formulas the runtime executes. A shape it cannot describe is noted in
//! `unrepresented`, which refuses the request.

use serde_json::{json, Value};

use crate::contracts::prepared_v2::ActionId;
use crate::contracts::request::Message;
use crate::data::spells::Spell;

use super::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use super::character::constants::CHARACTER_LEVEL;
use super::common_effects::{action_id_string, active_stats, flat_string, SPELL_BATCH_WINDOW};
use super::consumable_effects::proc_trigger_spells;
use super::dbcenums;
use super::env::Environment;
use super::items::{self, effect_stats};
use super::procs::DynamicProcManager;
use super::resolve_proc::{item_proc_chance, proc_trigger};
use super::shared_auras::in_area;
use super::sim::{AuraId, SpellId, UnitId, SECOND};
use super::spell::{school, DefenseType, ProcMask, SpellFlag};
use super::spelldata::{effect::AURA_ON_ENEMY, effect::AURA_ON_PET, find, must_find};
use super::stats::{Stat, Stats};

// ---------------------------------------------------------------------------------------------
// Helpers shared by the descriptions.
// ---------------------------------------------------------------------------------------------

/// druid.go `callbackNames`: the names of the callbacks a trigger listens to, in Go's order.
pub(crate) fn callback_names(callback: CallbackMask) -> Vec<&'static str> {
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

/// Go `HitOutcome.String` of one bit.
fn outcome_name(flag: HitOutcome) -> &'static str {
    if flag.matches(HitOutcome::MISS) {
        "Miss"
    } else if flag.matches(HitOutcome::DODGE) {
        "Dodge"
    } else if flag.matches(HitOutcome::PARRY) {
        "Parry"
    } else if flag.matches(HitOutcome::BLOCK) && flag.matches(HitOutcome::CRIT) {
        "BlockedCrit"
    } else if flag.matches(HitOutcome::BLOCK) {
        "Block"
    } else if flag.matches(HitOutcome::GLANCE) {
        "Glance"
    } else if flag.matches(HitOutcome::CRIT) {
        "Crit"
    } else if flag.matches(HitOutcome::HIT) {
        "Hit"
    } else if flag.matches(HitOutcome::CRUSH) {
        "Crush"
    } else {
        "Empty"
    }
}

/// druid.go `outcomeNames`: the name of each bit of the outcome.
pub(crate) fn outcome_names(outcome: HitOutcome) -> Vec<&'static str> {
    (0..32)
        .map(|bit| HitOutcome(1u16.checked_shl(bit).unwrap_or(0)))
        .filter(|flag| flag.0 != 0 && outcome.0 & flag.0 != 0)
        .map(outcome_name)
        .collect()
}

/// melee_procs.go `dpmChances`: the chance a dynamic proc manager rolls for each spell it hears,
/// taking the first mask entry the spell's proc mask matches.
fn dpm_chances(
    env: &Environment,
    dpm: &DynamicProcManager,
    eligible: impl Fn(&super::spell::Spell) -> bool,
) -> Vec<Value> {
    let mut chances = Vec::new();
    for (position, spell) in env.sim.unit(env.player).spellbook.iter().enumerate() {
        let spell = env.sim.spell(*spell);
        if !eligible(spell) {
            continue;
        }
        for (mask, chance) in dpm.proc_masks.iter().zip(&dpm.proc_chances) {
            if mask.matches(spell.proc_mask) {
                chances.push(json!({"spell": position, "chance": chance}));
                break;
            }
        }
    }
    chances
}

/// The position of the last spellbook entry with this action: the exporter's loops keep
/// overwriting the position, and `-1` where there is none.
fn spell_position(env: &Environment, action: &ActionId) -> Option<usize> {
    let mut found = None;
    for (position, spell) in env.sim.unit(env.player).spellbook.iter().enumerate() {
        if &env.sim.spell(*spell).action_id == action {
            found = Some(position);
        }
    }
    found
}

/// The spell at a spellbook position.
fn spell_at(env: &Environment, position: usize) -> SpellId {
    env.sim.unit(env.player).spellbook[position]
}

fn trigger_for(env: &Environment, row: &'static Spell, with_item_chance: bool) -> ProcTrigger {
    let opts = if with_item_chance {
        vec![item_proc_chance(row)]
    } else {
        Vec::new()
    };
    proc_trigger(&env.sim, Some(env.player), row, &opts)
}

/// An unset chance reads as certain: `AttachProcTriggerCallback`.
fn chance_or_certain(listener: &ProcTrigger) -> f64 {
    if listener.proc_chance == 0.0 {
        1.0
    } else {
        listener.proc_chance
    }
}

fn aura_named(env: &Environment, label: &str) -> Option<AuraId> {
    env.sim.get_aura(env.player, label)
}

/// A proc mask is a melee hit taken or dealt.
const STRUCK_PROC_MASK: ProcMask = ProcMask::MELEE_OR_RANGED;

/// Go `Encounter.ActiveTargetCount` with every target active.
fn active_target_count(env: &Environment) -> usize {
    env.encounter.targets.len()
}

/// Go `Encounter.AOECapMultiplier` with every target active.
fn aoe_cap_multiplier(env: &Environment) -> f64 {
    (20.0 / env.encounter.targets.len() as f64).min(1.0)
}

/// Go `spell.Dot(target)`: the dot of the spell or of its related dot spell on the unit.
fn spell_dot(env: &Environment, spell: SpellId, target: UnitId) -> Option<super::sim::DotId> {
    let s = env.sim.spell(spell);
    if s.dots.is_empty() {
        return s
            .related_dot_spell
            .and_then(|related| spell_dot(env, related, target));
    }
    let index = env.sim.unit(target).unit_index as usize;
    s.dots.get(index).copied().flatten()
}

fn has_mana_bar(env: &Environment) -> bool {
    env.sim.unit(env.player).mana_bar.enabled
}

// ---------------------------------------------------------------------------------------------
// main.go: the item uses.
// ---------------------------------------------------------------------------------------------

/// Items whose use registers temporary stats with `RegisterTemporaryStatsOnUseCD`, by the Go
/// literal stats each passes (common/classic/items_weapons.go): Headmaster's Charge.
fn temporary_stat_items(item: i32) -> Option<Stats> {
    match item {
        13937 => Some(Stats::from_pairs(&[(Stat::Intellect, 20.0)])),
        _ => None,
    }
}

/// main.go `onlyOnUseEffect`: the item's one use effect, or none.
pub(crate) fn only_on_use_effect(item: i32) -> Option<Message> {
    if item == 0 {
        return None;
    }
    let db_item = items::database_item(item)?;
    let mut on_use: Option<Message> = None;
    for effect in db_item.item_effects {
        if !effect.has("on_use") {
            continue;
        }
        if on_use.is_some() {
            return None;
        }
        on_use = Some(effect);
    }
    on_use
}

fn related_self_buff(env: &Environment, spell: SpellId) -> Option<AuraId> {
    env.sim.spell(spell).related_self_buff
}

/// main.go `speedOnUse`: shared.NewSpellDataSpeedOnUse: the use activates the buff row's aura,
/// its self buff, which multiplies melee, ranged and cast speed by the row's haste percents while
/// up. Nil for any other item use.
fn speed_on_use(env: &Environment, spell: SpellId) -> Option<Value> {
    let item = env.sim.spell(spell).action_id.item_id;
    let on_use = only_on_use_effect(item)?;
    let aura = related_self_buff(env, spell)?;
    let row = find(on_use.i32("buff_id"));
    if row.speed_effects().is_empty()
        || env.sim.aura(aura).label != super::resolve_aura::aura_config(row, &[]).label
    {
        return None;
    }
    let pseudo = row.speed_pseudo_stats();
    let multiplier = |stat: usize| 1.0 + pseudo.get(stat).copied().unwrap_or(0.0) / 100.0;
    use super::spelldata::speed::pseudo_stat;
    Some(json!({
        "kind": "speed_on_use", "item_id": item, "aura": env.sim.aura(aura).label,
        "melee_multiplier": multiplier(pseudo_stat::MELEE_HASTE_PERCENT),
        "ranged_multiplier": multiplier(pseudo_stat::RANGED_HASTE_PERCENT),
        "cast_multiplier": multiplier(pseudo_stat::SPELL_HASTE_PERCENT),
    }))
}

/// main.go `simpleStatActive`: shared.NewSimpleStatActive: an on-use item whose one use effect's
/// buff, its self buff, carries the effect's scaling stats times the buff row's area bonus. Nil
/// for any other item use.
pub(crate) fn simple_stat_active(env: &Environment, spell: SpellId) -> Option<Stats> {
    let item = env.sim.spell(spell).action_id.item_id;
    let aura = related_self_buff(env, spell)?;
    if temporary_stat_items(item).is_some() || speed_on_use(env, spell).is_some() {
        return None;
    }
    let on_use = only_on_use_effect(item)?;
    if on_use.str("buff_name") != env.sim.aura(aura).label {
        return None;
    }
    items::scaling_option(&on_use)?;
    let (amount, _) = find(on_use.i32("buff_id")).area_bonus(|area| in_area(env, area));
    Some(effect_stats(&on_use).multiply(amount))
}

/// main.go `survivalOnUse`: shared.NewSpellDataAbsorbOnUse and NewSpellDataHealOnUse: an item
/// use whose row shields the wearer against the schools its absorb effect masks, for the amount
/// the effect rolls, or heals it directly, a share of maximum health or a rolled amount.
fn survival_on_use(env: &Environment, spell: SpellId, item: i32) -> Option<Value> {
    let effect = only_on_use_effect(item)?;
    let row = find(effect.i32("buff_id"));
    if row.is_nil() {
        return None;
    }
    let player = env.player;
    let absorb = row.absorb_effect();
    if !absorb.is_nil() {
        if let Some(aura) = related_self_buff(env, spell) {
            return Some(json!({
                "kind": "absorb_on_use", "item_id": item, "aura": env.sim.aura(aura).label,
                "schools": absorb.misc, "average": absorb.average(env.sim.unit(player).level),
                "variance": absorb.variance,
            }));
        }
    }
    let heal = row.proc_heal_effect();
    if heal.is_nil()
        || heal.aura == dbcenums::A_PERIODIC_HEAL
        || related_self_buff(env, spell).is_some()
        || env.sim.spell(spell).bonus_coefficient != 0.0
    {
        return None;
    }
    let pseudo = &env.sim.unit(player).pseudo_stats;
    let mut exported = json!({
        "kind": "heal_on_use", "item_id": item, "can_crit": !row.cannot_crit(),
        "healing_dealt_multiplier": pseudo.healing_dealt_multiplier,
        "healing_taken_multiplier": pseudo.healing_taken_multiplier,
        "table_healing_dealt_multiplier": env.attack_table(player, player).healing_dealt_multiplier,
        "bonus_healing_taken": pseudo.bonus_healing_taken,
    });
    if heal.effect_type == dbcenums::E_HEAL_PCT {
        exported["max_health_share"] = json!(heal.percent());
    } else {
        exported["average"] = json!(heal.average(env.sim.unit(player).level));
        exported["variance"] = json!(heal.variance);
    }
    Some(exported)
}

/// The item cases of the item loop: temporary stats, speed, damage, survival and energize on
/// use, Burst of Knowledge and Second Wind, in Go's order. Class item uses follow through the
/// agent. `None` for an item none claims; an empty list for one claimed that exports nothing.
pub(crate) fn use_item_effect(
    env: &mut Environment,
    spell: SpellId,
    item: i32,
    unrepresented: &mut Vec<String>,
) -> Option<Vec<Value>> {
    // RegisterTemporaryStatsOnUseCD on an item: Go literal stats, or shared.NewSimpleStatActive's
    // on-use buff from the item's database effect.
    if temporary_stat_items(item).is_some() || simple_stat_active(env, spell).is_some() {
        let Some(aura) = related_self_buff(env, spell) else {
            unrepresented.push(format!("item {item} has no temporary stats aura"));
            return Some(Vec::new());
        };
        let bonus = temporary_stat_items(item).or_else(|| simple_stat_active(env, spell))?;
        let label = env.sim.aura(aura).label.clone();
        let aura_id = env.sim.aura(aura).action_id.clone().unwrap_or_default();
        return Some(vec![json!({
            "kind": "temporary_stats", "spell_id": 0, "item_id": item, "aura": label,
            "active_stats": active_stats(env, &label),
            "gain_log": format!("Gained {} from {}.", flat_string(&bonus), action_id_string(&aura_id)),
            "expire_log": format!("Lost {} from fading {}.", flat_string(&bonus), action_id_string(&aura_id)),
        })]);
    }
    if let Some(effect) = speed_on_use(env, spell) {
        return Some(vec![effect]);
    }
    if DAMAGE_ON_USE_ITEMS.contains(&item) {
        let effect = damage_on_use_effect(env, spell, unrepresented);
        return Some(if effect.is_null() {
            Vec::new()
        } else {
            vec![effect]
        });
    }
    // common/classic/items_trinkets.go Burst of Knowledge.
    if item == 11832 {
        return Some(
            match spell_cost_aura_on_use_effect(env, spell, "Burst of Knowledge") {
                Some(effect) => vec![effect],
                None => {
                    unrepresented.push("Burst of Knowledge has no aura".to_string());
                    Vec::new()
                }
            },
        );
    }
    if let Some(effect) = survival_on_use(env, spell, item) {
        return Some(vec![effect]);
    }
    // common/classic/items_trinkets.go Second Wind: Go literals, 63 mana a second for 10
    // seconds on its own metrics, and the automatic use waits for 630 mana missing.
    if item == 11819 && has_mana_bar(env) {
        return Some(vec![json!({
            "kind": "second_wind", "item_id": item, "mana": 63.0, "ticks": 10,
            "period_ns": SECOND, "metrics_spell_id": 15604, "min_deficit": 630.0,
        })]);
    }
    // A use effect a class file registers itself.
    if let Some(effect) = env.agent.class_item_use_effect(env, spell, item) {
        return Some(vec![effect]);
    }
    Some(energize_on_use_effects(env, spell, item, unrepresented))
}

/// The default case of the item loop: shared.NewSpellDataEnergizeOnUse: an item use spell that
/// restores mana, at once or as a self hot of the row's ticks; the manager waits until the whole
/// gain fits. It has no self buff; an item whose use activates one registered another effect.
fn energize_on_use_effects(
    env: &Environment,
    spell: SpellId,
    item: i32,
    unrepresented: &mut Vec<String>,
) -> Vec<Value> {
    let mut effects = Vec::new();
    let mut found = false;
    if let Some(db_item) = items::database_item(item) {
        if related_self_buff(env, spell).is_none() {
            let level = env.sim.unit(env.player).level;
            for item_effect in &db_item.item_effects {
                if !item_effect.has("on_use") {
                    continue;
                }
                let row = find(item_effect.i32("buff_id"));
                let effect = row.proc_energize_effect();
                if effect.is_nil() || effect.misc != dbcenums::POWER_MANA {
                    continue;
                }
                let periodic = effect.aura == dbcenums::A_PERIODIC_ENERGIZE;
                let mut ticks = 1.0;
                if periodic {
                    if env.sim.spell(spell).aoe_dot.is_none() {
                        continue;
                    }
                    ticks = f64::from(
                        super::resolve_aura::dot_config(row, effect, &[]).number_of_ticks,
                    );
                }
                found = true;
                let mut energize = json!({
                    "kind": "energize_on_use", "item_id": item, "spell_id": row.id,
                    "average": effect.average(level), "variance": effect.variance,
                    "whole": effect.max(level) * ticks,
                });
                if periodic {
                    energize["periodic"] = json!(true);
                }
                effects.push(energize);
            }
        }
    }
    if !found {
        unrepresented.push(format!("major cooldown item {item} has no exported effect"));
    }
    effects
}

/// common/classic/items_trinkets.go Burst of Knowledge (11832): its use activates an aura whose
/// spell mod lowers the mana cost of spells by a flat amount. The changes are read from a
/// separate reset simulation with the aura active, by spellbook position.
fn spell_cost_aura_on_use_effect(env: &Environment, spell: SpellId, label: &str) -> Option<Value> {
    aura_named(env, label)?;
    let mut fresh = env.fresh();
    let player = fresh.player;
    let book = fresh.sim.unit(player).spellbook.clone();
    let flat =
        |sim: &super::sim::Sim, id: SpellId| sim.spell(id).cost.as_ref().map(|c| c.flat_modifier);
    let before: Vec<Option<i32>> = book.iter().map(|id| flat(&fresh.sim, *id)).collect();
    let aura = fresh.sim.get_aura(player, label)?;
    fresh.sim.activate(aura);
    let mut changes = Vec::new();
    for (i, id) in book.iter().enumerate() {
        if let Some(now) = flat(&fresh.sim, *id) {
            let was = before[i].unwrap_or(0);
            if now != was {
                changes.push(json!({"spell": i, "flat": now - was}));
            }
        }
    }
    Some(json!({
        "kind": "spell_cost_aura_on_use", "item_id": env.sim.spell(spell).action_id.item_id,
        "aura": label, "cost_changes": changes,
    }))
}

// ---------------------------------------------------------------------------------------------
// damage_on_use.go.
// ---------------------------------------------------------------------------------------------

/// common/forever/stat_bonus_cds_auto_gen.go: the on-use items shared.NewSpellDataDamageOnUse
/// registers.
const DAMAGE_ON_USE_ITEMS: [i32; 6] = [8348, 11905, 13171, 21891, 219345, 274759];

/// The outcome shared_utils.go `damageOutcome` picks for the defense type, by the runtime's name.
fn on_use_outcome(defense: DefenseType, cannot_crit: bool) -> &'static str {
    match (defense, cannot_crit) {
        (DefenseType::Melee, true) => "melee_special_hit",
        (DefenseType::Melee, false) => "melee_special_hit_and_crit",
        (DefenseType::Magic, true) => "magic_hit",
        (DefenseType::Magic, false) => "magic_hit_and_crit",
        _ => "",
    }
}

/// damage_on_use.go `damageOnUseEffect`: shared_utils.go `spellDataOnUseDamageSpell` on the one
/// target: the row's direct hit, rolled once and scaled as `calcMultiTargetDamage` scales it for
/// one target, then the damage over time it carries. A row it cannot describe is unrepresented
/// and returns null.
fn damage_on_use_effect(
    env: &Environment,
    spell: SpellId,
    unrepresented: &mut Vec<String>,
) -> Value {
    let s = env.sim.spell(spell);
    let item = s.action_id.item_id;
    if !DAMAGE_ON_USE_ITEMS.contains(&item) || s.action_id.spell_id != 0 {
        return Value::Null;
    }
    let position = env
        .sim
        .unit(env.player)
        .spellbook
        .iter()
        .rposition(|registered| *registered == spell);
    let on_use = only_on_use_effect(item);
    let (Some(on_use), Some(position)) = (on_use, position) else {
        unrepresented.push(format!(
            "damage on-use item {item} has no single on-use spell"
        ));
        return Value::Null;
    };
    let row = must_find(on_use.i32("buff_id"));
    let direct = row.damage_effect();
    let periodic = row.periodic_damage_effect();
    let defense = s.defense_type;
    let physical = s.spell_school & school::PHYSICAL != 0;
    let outcome = on_use_outcome(defense, row.cannot_crit());
    let target = env.encounter.targets[0];
    let dot = spell_dot(env, spell, target);
    let dot_coefficient = dot.map_or(0.0, |dot| env.sim.dots[dot.0].bonus_coefficient);
    let level = env.sim.unit(env.player).level;

    if outcome.is_empty()
        || !row.debuff_effects().is_empty()
        || (direct.is_nil() && periodic.is_nil())
    {
        unrepresented.push(format!(
            "damage on-use item {item}'s row is not a hit or a damage over time"
        ));
        return Value::Null;
    }
    if physical && (s.bonus_coefficient != 0.0 || (dot.is_some() && dot_coefficient != 0.0)) {
        unrepresented.push(format!(
            "damage on-use item {item} scales a physical hit with spell power"
        ));
        return Value::Null;
    }
    if !periodic.is_nil() && row.periodic_can_crit() && defense != DefenseType::Magic {
        unrepresented.push(format!(
            "damage on-use item {item}'s ticks roll a physical crit"
        ));
        return Value::Null;
    }
    if !periodic.is_nil() && dot.is_none() {
        unrepresented.push(format!("damage on-use item {item} has no dot"));
        return Value::Null;
    }
    if !direct.is_nil()
        && (direct.hits_an_area() || direct.chain_targets > 1)
        && active_target_count(env) > 1
    {
        unrepresented.push(format!("damage on-use item {item} hits several targets"));
        return Value::Null;
    }
    let mut effect = json!({"kind": "damage_on_use", "item_id": item, "spell": position});
    if !direct.is_nil() {
        let mut scale = 1.0;
        if direct.hits_an_area() && row.max_targets == 0 && !row.splits_damage {
            scale = aoe_cap_multiplier(env);
        }
        effect["direct"] = json!({
            "average": direct.average(level), "variance": direct.variance,
            "scale": scale, "outcome": outcome,
        });
    }
    if !periodic.is_nil() {
        let mut ticks = json!({
            "tick_base": periodic.average(level), "tick_can_crit": row.periodic_can_crit(),
        });
        if direct.is_nil() {
            ticks["application_outcome"] = json!(on_use_outcome(defense, true));
        }
        effect["periodic"] = ticks;
    }
    effect
}

// ---------------------------------------------------------------------------------------------
// item_procs.go.
// ---------------------------------------------------------------------------------------------

/// Procs common/forever/stat_bonus_procs_auto_gen.go and enchants_auto_gen.go register with
/// shared.NewSpellDataProc: the trigger's name, its trigger and buff rows, and the item or
/// enchant whose effect entry states the buff's stats; or with shared.NewSpellDataAuraProc, whose
/// buff is the row's own parsed aura on the wearer.
struct SpellDataStatProc {
    label: &'static str,
    trigger: i32,
    buff: i32,
    item: i32,
    enchant: i32,
    parsed: bool,
}

const SPELL_DATA_STAT_PROCS: [SpellDataStatProc; 4] = [
    SpellDataStatProc {
        label: "Draconic Infused Emblem",
        trigger: 1318931,
        buff: 1318930,
        item: 22268,
        enchant: 0,
        parsed: false,
    },
    SpellDataStatProc {
        label: "Enchant Weapon - Grand Sorcerer",
        trigger: 1231163,
        buff: 1231162,
        item: 0,
        enchant: 7942,
        parsed: false,
    },
    SpellDataStatProc {
        label: "Enchant 2H Weapon - Grand Arcanist",
        trigger: 1231152,
        buff: 1231138,
        item: 0,
        enchant: 7941,
        parsed: false,
    },
    SpellDataStatProc {
        label: "Enchant Weapon - Insight",
        trigger: 1248758,
        buff: 1299796,
        item: 0,
        enchant: 8216,
        parsed: true,
    },
];

/// shared_utils.go applySpellDataProc names the buff it builds after the trigger.
fn spell_data_stat_proc_aura(label: &str) -> String {
    format!("{label} Proc")
}

/// The stat auras of the spell data procs the character wears, which `statAurasEffect` combines.
pub(crate) fn spell_data_stat_proc_auras(env: &Environment) -> Vec<String> {
    SPELL_DATA_STAT_PROCS
        .iter()
        .filter(|proc| aura_named(env, proc.label).is_some())
        .map(|proc| spell_data_stat_proc_aura(proc.label))
        .collect()
}

/// item_procs.go `spellDataStatProcEffects`: the listener its trigger row decodes to rolls the
/// stated chance under the trigger's name, behind the trigger aura's cooldown, and a spell batch
/// window later activates a temporary stats aura of the effect entry's stats. Only a proc without
/// a rate, stacks or charges, hearing landed or any hits, casts and heals, is described.
pub(crate) fn spell_data_stat_proc_effects(
    env: &Environment,
    unrepresented: &mut Vec<String>,
) -> Vec<Value> {
    let mut effects = Vec::new();
    for proc in &SPELL_DATA_STAT_PROCS {
        let Some(trigger_aura) = aura_named(env, proc.label) else {
            continue;
        };
        let proc_aura = aura_named(env, &spell_data_stat_proc_aura(proc.label));
        let trigger = must_find(proc.trigger);
        let buff = must_find(proc.buff);
        let entries: Vec<Message> = if proc.item != 0 {
            items::database_item(proc.item)
                .map(|item| item.item_effects)
                .unwrap_or_default()
        } else {
            items::enchant(proc.enchant)
                .map(|enchant| enchant.enchant_effects)
                .unwrap_or_default()
        };
        let mut entry: Option<Message> = entries
            .into_iter()
            .rfind(|candidate| candidate.i32("buff_id") == proc.buff);
        if proc.parsed {
            // The parsed aura needs no effect entry; one on a pet or an enemy is not described.
            entry = Some(Message::empty("proto.ItemEffect"));
            if !super::spelldata::effect::effects_on(buff, AURA_ON_PET).is_empty()
                || !super::spelldata::effect::effects_on(buff, AURA_ON_ENEMY).is_empty()
            {
                entry = None;
            }
        }
        let listener = trigger_for(env, trigger, true);
        let names = callback_names(listener.callback);
        let heard = !names.is_empty()
            && names.iter().all(|name| {
                matches!(
                    *name,
                    "on_spell_hit_dealt" | "on_heal_dealt" | "on_cast_complete"
                )
            });
        let unsupported = match (&entry, proc_aura) {
            (Some(entry), Some(_)) => {
                entry.has("stacking_aura")
                    || env.sim.aura(trigger_aura).dpm.is_some()
                    || trigger.rppm != 0.0
                    || entry.message("proc").is_some_and(|p| p.f64("ppm") != 0.0)
                    || buff.max_stack.max(trigger.max_stack) > 0
                    || buff.proc_charges > 0
                    || !heard
                    || listener.is_weapon_proc
                    || (listener.outcome != HitOutcome::EMPTY
                        && listener.outcome != HitOutcome::LANDED)
            }
            _ => true,
        };
        if unsupported {
            unrepresented.push(format!(
                "{} is not a chance stat proc on hits, heals or casts",
                proc.label
            ));
            continue;
        }
        let (Some(entry), Some(proc_aura)) = (entry, proc_aura) else {
            continue;
        };
        let mut effect = json!({
            "kind": "spell_data_stat_proc", "trigger_aura": env.sim.aura(trigger_aura).label,
            "aura": env.sim.aura(proc_aura).label,
            "trigger_spells": proc_trigger_spells(env, &listener), "callbacks": names,
            "landed_only": listener.outcome == HitOutcome::LANDED,
            "require_damage": listener.require_damage_dealt,
            "proc_chance": chance_or_certain(&listener),
        });
        if !proc.parsed {
            let bonus = effect_stats(&entry);
            let id = env
                .sim
                .aura(proc_aura)
                .action_id
                .clone()
                .unwrap_or_default();
            effect["gain_log"] = json!(format!(
                "Gained {} from {}.",
                flat_string(&bonus),
                action_id_string(&id)
            ));
            effect["expire_log"] = json!(format!(
                "Lost {} from fading {}.",
                flat_string(&bonus),
                action_id_string(&id)
            ));
        }
        effects.push(effect);
    }
    effects
}

// ---------------------------------------------------------------------------------------------
// main.go: the lion horn and the damage shield, classic items through the shared constructors.
// ---------------------------------------------------------------------------------------------

/// classic items_store_gaps.go The Lion Horn of Stormwind.
pub(crate) const LION_HORN: &str = "The Lion Horn of Stormwind";

/// main.go `lionHornProcAura`: a tank's proc aura joins the stat auras, whose combinations carry
/// the target's rolls.
pub(crate) fn lion_horn_proc_aura(env: &Environment) -> Vec<String> {
    let target = env.encounter.targets[0];
    if aura_named(env, LION_HORN).is_none()
        || env.sim.unit(target).current_target != Some(env.player)
    {
        return Vec::new();
    }
    vec![format!("{LION_HORN} Proc")]
}

/// main.go `lionHornProc`: The Lion Horn of Stormwind, through shared.NewProcStatBonusEffect: a
/// listener on landed melee hits the wearer takes that dealt damage, at the item effect's chance
/// and cooldown, that a spell batch window later activates the effect's temporary stats aura.
fn lion_horn_proc(env: &Environment, trigger: AuraId) -> Option<Value> {
    let proc_aura = aura_named(env, &format!("{LION_HORN} Proc"))?;
    let entry = items::database_item(14557)?
        .item_effects
        .into_iter()
        .rfind(|candidate| candidate.has("proc"))?;
    let proc = entry.message("proc")?;
    let trigger_aura = env.sim.aura(trigger);
    let icd_ms = proc.i32("icd_ms");
    if entry.has("stacking_aura")
        || entry.i32("max_cumulative_stacks") > 0
        || trigger_aura.dpm.is_some()
        || proc.f64("ppm") != 0.0
        || (icd_ms != 0) != trigger_aura.icd.is_some()
        || trigger_aura
            .icd
            .is_some_and(|icd| icd.duration != i64::from(icd_ms) * super::sim::MILLISECOND)
    {
        return None;
    }
    let chance = if proc.f64("proc_chance") == 0.0 {
        1.0
    } else {
        proc.f64("proc_chance")
    };
    let bonus = effect_stats(&entry);
    let id = env
        .sim
        .aura(proc_aura)
        .action_id
        .clone()
        .unwrap_or_default();
    Some(json!({
        "kind": "spell_data_stat_proc", "trigger_aura": trigger_aura.label,
        "aura": env.sim.aura(proc_aura).label, "trigger_spells": Vec::<usize>::new(),
        "callbacks": ["on_spell_hit_taken"], "struck": true, "landed_only": true,
        "require_damage": true, "proc_chance": chance,
        "gain_log": format!("Gained {} from {}.", flat_string(&bonus), action_id_string(&id)),
        "expire_log": format!("Lost {} from fading {}.", flat_string(&bonus), action_id_string(&id)),
    }))
}

/// main.go `damageShieldProc`: classic items_armor.go newDamageShieldEffect, through
/// shared.NewProcDamageEffect: a listener on landed melee hits the wearer takes, at no chance or
/// cooldown, that casts a fixed magic hit of the school on the attacker at once, which cannot
/// crit. Only that shape is described.
fn damage_shield_proc(
    env: &Environment,
    aura: AuraId,
    spell_id: i32,
    damage: f64,
) -> Option<Value> {
    let position = spell_position(env, &ActionId::spell(spell_id));
    let a = env.sim.aura(aura);
    let position = position?;
    if a.dpm.is_some()
        || a.icd.is_some()
        || env.sim.spell(spell_at(env, position)).defense_type != DefenseType::Magic
        || a.events.on_spell_hit_dealt
        || !a.events.on_spell_hit_taken
        || a.events.on_periodic_damage_dealt
    {
        return None;
    }
    Some(json!({
        "kind": "spell_data_damage_proc", "trigger_aura": a.label, "trigger_spells": Vec::<usize>::new(),
        "struck": true, "landed_only": true, "require_damage": false, "proc_chance": 1.0,
        "spell": position, "average": 0.0, "variance": 0.0, "roll": [damage, damage],
        "can_crit": false,
    }))
}

/// main.go `spellDataAbsorbProc`: shared.NewSpellDataAbsorbProc: a listener resolved from the
/// trigger row that casts the absorb row's spell on the wearer at once, whose aura shields it
/// against the schools the absorb effect masks for the amount the effect rolls. Only a listener
/// on the melee hits the player takes, at a static chance, is described.
fn spell_data_absorb_proc(
    env: &Environment,
    aura: AuraId,
    trigger_id: i32,
    absorb_id: i32,
) -> Option<Value> {
    let trigger = must_find(trigger_id);
    let row = must_find(absorb_id);
    let listener = trigger_for(env, trigger, true);
    let absorb = row.absorb_effect();
    let position = spell_position(env, &ActionId::spell(row.id))?;
    let spell = env.sim.spell(spell_at(env, position));
    let a = env.sim.aura(aura);
    if absorb.is_nil()
        || spell.related_self_buff.is_none()
        || listener.dpm.is_some()
        || trigger.rppm != 0.0
        || listener.can_proc_from_procs
        || listener.is_weapon_proc
        || listener.class_spell_mask != 0
        || listener.spell_flags != SpellFlag::NONE
        || listener.proc_mask_exclude != ProcMask::UNKNOWN
        || (listener.icd != 0) != a.icd.is_some()
        || a.icd.is_some_and(|icd| icd.duration != listener.icd)
    {
        return None;
    }
    let level = env.sim.unit(env.player).level;
    let shield = env.sim.aura(spell.related_self_buff?).label.clone();
    Some(json!({
        "kind": "spell_data_absorb_proc", "trigger_aura": a.label,
        "outcome": outcome_names(listener.outcome),
        "require_damage": listener.require_damage_dealt, "proc_chance": chance_or_certain(&listener),
        "spell": position, "aura": shield, "schools": absorb.misc,
        "average": absorb.average(level), "variance": absorb.variance,
    }))
}

/// main.go `meleeItemProcs`: item procs whose listener, decoded by spelldata's `ProcTrigger` from
/// the trigger row, hears only melee hits. Melee autos need auto attacks, which are
/// unrepresented, so the listener never acts unless a spell with a melee special mask exists.
const MELEE_ITEM_PROCS: [(&str, i32); 2] = [("Storm Gauntlets", 16615), ("Orb of Fire", 16982)];

/// main.go `meleeItemListeners`.
pub(crate) fn melee_item_listeners(env: &Environment) -> Vec<Value> {
    let book = &env.sim.unit(env.player).spellbook;
    if book.iter().any(|spell| {
        env.sim
            .spell(*spell)
            .proc_mask
            .matches(ProcMask::MELEE_SPECIAL)
    }) {
        return Vec::new();
    }
    let mut effects = Vec::new();
    for (label, trigger) in MELEE_ITEM_PROCS {
        if aura_named(env, label).is_none() {
            continue;
        }
        let listener = trigger_for(env, find(trigger), false);
        if listener.proc_mask == ProcMask::UNKNOWN || listener.proc_mask.0 & !ProcMask::MELEE.0 != 0
        {
            continue;
        }
        effects.push(json!({
            "kind": "inert_listener", "unit": "player", "aura": label,
            "reason": "acts only on melee hits",
        }));
    }
    effects
}

/// main.go `hitTakenItemProcs`: item procs whose listener hears only the melee hits the player
/// takes: Go literal triggers in classic items (Essence of the Pure Flame's damage shield, The
/// Lion Horn of Stormwind), and Uther's Strength and the chest absorption enchants, whose trigger
/// rows spelldata decodes.
const HIT_TAKEN_ITEM_PROCS: [(&str, i32, i32); 6] = [
    ("Essence of the Pure Flame", 0, 0),
    (LION_HORN, 0, 0),
    ("Uther's Strength", 8397, 10368),
    ("Enchant Chest - Minor Absorption", 7445, 7423),
    ("Enchant Chest - Lesser Absorption", 7446, 7447),
    ("Enchant Chest - Absorption", 1249072, 1249073),
];

/// main.go `hitTakenItemListeners`.
pub(crate) fn hit_taken_item_listeners(
    env: &Environment,
    tanking: bool,
    unrepresented: &mut Vec<String>,
) -> Vec<Value> {
    const LIFECYCLE: [&str; 7] = [
        "on_init",
        "on_reset",
        "on_done_iteration",
        "on_gain",
        "on_expire",
        "on_stacks_change",
        "on_encounter_start",
    ];
    let mut effects = Vec::new();
    for (label, trigger_id, absorb_id) in HIT_TAKEN_ITEM_PROCS {
        let Some(aura) = aura_named(env, label) else {
            continue;
        };
        let mut heard = env.sim.aura(aura).callback_names().iter().all(|callback| {
            LIFECYCLE.contains(&callback.as_str()) || callback == "on_spell_hit_taken"
        });
        if trigger_id != 0 {
            let listener = trigger_for(env, find(trigger_id), false);
            let names = callback_names(listener.callback);
            heard = heard
                && names.len() == 1
                && names[0] == "on_spell_hit_taken"
                && listener.proc_mask != ProcMask::UNKNOWN
                && listener.proc_mask.0 & !ProcMask::MELEE.0 == 0;
        }
        if heard && tanking && label == "Essence of the Pure Flame" {
            match damage_shield_proc(env, aura, 23266, 13.0) {
                Some(effect) => effects.push(effect),
                None => unrepresented.push(format!("{label}'s proc is not a damage shield")),
            }
            continue;
        }
        if heard && tanking && label == LION_HORN {
            match lion_horn_proc(env, aura) {
                Some(effect) => effects.push(effect),
                None => unrepresented.push(format!("{label}'s proc is not a chance stat proc")),
            }
            continue;
        }
        if heard && tanking && absorb_id != 0 {
            match spell_data_absorb_proc(env, aura, trigger_id, absorb_id) {
                Some(effect) => effects.push(effect),
                None => unrepresented.push(format!(
                    "{label}'s proc is not an absorb shield on the wearer"
                )),
            }
            continue;
        }
        if heard {
            effects.push(json!({
                "kind": "inert_listener", "unit": "player", "aura": label,
                "reason": "hears only melee hits the player takes",
            }));
        }
    }
    effects
}

// ---------------------------------------------------------------------------------------------
// melee_procs.go.
// ---------------------------------------------------------------------------------------------

/// A shape `meleeProcEffects` cannot describe, in Go's words.
fn not_single_target_magic_hit(label: &str) -> String {
    format!("{label}'s proc is not a single target magic hit")
}

/// melee_procs.go `spellDataDamageProcs`: item procs common/shared/shared_utils.go
/// applySpellDataDamageProc builds from client rows: the item's trigger aura, its trigger row and
/// the damage row the proc casts.
const SPELL_DATA_DAMAGE_PROCS: [(&str, i32, i32); 4] = [
    ("Storm Gauntlets", 16615, 16614),
    ("Orb of Fire", 16982, 13441),
    ("Premier High Warlord's Shield Wall", 13959, 16782),
    ("Premier Grand Marshal's Aegis", 13959, 16782),
];

/// melee_procs.go `spellDataDamageProcEffects`: a listener resolved from the trigger row that
/// casts the damage spell at once on the unit hit, or, for a hit taken, on the attacker. Only a
/// single target magic hit is described: dealt where it lands, or struck by a melee or ranged
/// hit.
fn spell_data_damage_proc_effects(
    env: &Environment,
    unrepresented: &mut Vec<String>,
) -> Vec<Value> {
    let mut effects = Vec::new();
    let melee = env.sim.unit(env.player).spellbook.iter().any(|spell| {
        env.sim
            .spell(*spell)
            .proc_mask
            .matches(ProcMask::MELEE_SPECIAL)
    });
    for (label, trigger_id, damage_id) in SPELL_DATA_DAMAGE_PROCS {
        if aura_named(env, label).is_none() {
            continue;
        }
        let trigger = must_find(trigger_id);
        let damage = must_find(damage_id);
        let listener = trigger_for(env, trigger, true);
        let struck = listener.callback == CallbackMask::ON_SPELL_HIT_TAKEN;
        // Without a melee special, meleeItemListeners describes a dealt listener as inert.
        if !struck && !melee {
            continue;
        }
        let position = spell_position(env, &ActionId::spell(damage.id));
        let effect = damage.damage_effect();
        // shared_utils.go damageDefenseType: a stated defense type, else the school's.
        let mut defense = damage.defense_type_core();
        if defense == DefenseType::None {
            defense = if damage.spell_school() & school::PHYSICAL != 0 {
                DefenseType::Melee
            } else {
                DefenseType::Magic
            };
        }
        let names = callback_names(listener.callback);
        let outcome_ok =
            listener.outcome == HitOutcome::EMPTY || listener.outcome == HitOutcome::LANDED;
        let struck_ok = !struck
            || (listener.proc_mask == STRUCK_PROC_MASK
                && !listener.can_proc_from_procs
                && !listener.is_weapon_proc
                && listener.class_spell_mask == 0
                && listener.spell_flags == SpellFlag::NONE
                && listener.proc_mask_exclude == ProcMask::UNKNOWN);
        let Some(position) = position else {
            unrepresented.push(not_single_target_magic_hit(label));
            continue;
        };
        if effect.is_nil()
            || !damage.periodic_damage_effect().is_nil()
            || effect.hits_an_area()
            || effect.chain_targets > 1
            || damage.applies_an_aura_to_an_enemy()
            || damage.speed != 0.0
            || defense != DefenseType::Magic
            || trigger.rppm != 0.0
            || listener.dpm.is_some()
            || names.len() != 1
            || (names[0] != "on_spell_hit_dealt" && !struck)
            || !outcome_ok
            || !struck_ok
        {
            unrepresented.push(not_single_target_magic_hit(label));
            continue;
        }
        // A struck proc hears the attacker's hits, which the Rust runtime tells by their masks.
        let trigger_spells = if struck {
            Vec::new()
        } else {
            proc_trigger_spells(env, &listener)
        };
        let mut exported = json!({
            "kind": "spell_data_damage_proc", "trigger_aura": label, "trigger_spells": trigger_spells,
            "landed_only": listener.outcome == HitOutcome::LANDED,
            "require_damage": listener.require_damage_dealt,
            "proc_chance": chance_or_certain(&listener), "spell": position,
            "average": effect.average(CHARACTER_LEVEL), "variance": effect.variance,
            "can_crit": !damage.cannot_crit(),
        });
        if struck {
            exported["struck"] = json!(true);
        }
        effects.push(exported);
    }
    effects
}

/// melee_procs.go `spellDataHealProcs`: enchants common/shared/shared_utils.go
/// NewSpellDataHealProc builds from client rows: Recovery.
const SPELL_DATA_HEAL_PROCS: [(&str, i32, i32); 1] =
    [("Enchant Weapon - Recovery", 1248761, 1248759)];

/// melee_procs.go `spellDataHealProcEffects`: a listener resolved from the trigger row that casts
/// the heal row on the wearer at once. Only a direct heal, a share of maximum health or a rolled
/// amount, is described.
fn spell_data_heal_proc_effects(env: &Environment, unrepresented: &mut Vec<String>) -> Vec<Value> {
    let mut effects = Vec::new();
    for (label, trigger_id, heal_id) in SPELL_DATA_HEAL_PROCS {
        let Some(aura) = aura_named(env, label) else {
            continue;
        };
        let trigger = must_find(trigger_id);
        let heal = must_find(heal_id);
        let listener = trigger_for(env, trigger, true);
        let position = spell_position(env, &ActionId::spell(heal.id));
        let effect = heal.proc_heal_effect();
        let names = callback_names(listener.callback);
        let a = env.sim.aura(aura);
        let icd = a.icd.is_some_and(|icd| icd.duration != listener.icd);
        let bonus_coefficient =
            position.map_or(0.0, |p| env.sim.spell(spell_at(env, p)).bonus_coefficient);
        let Some(position) = position else {
            unrepresented.push(format!("{label}'s proc is not a direct heal on the wearer"));
            continue;
        };
        if effect.is_nil()
            || effect.aura == dbcenums::A_PERIODIC_HEAL
            || (effect.effect_type != dbcenums::E_HEAL_PCT
                && effect.effect_type != dbcenums::E_HEAL)
            || listener.dpm.is_some()
            || trigger.rppm != 0.0
            || names.len() != 1
            || names[0] != "on_spell_hit_dealt"
            || icd
            || (listener.icd != 0) != a.icd.is_some()
            || bonus_coefficient != 0.0
        {
            unrepresented.push(format!("{label}'s proc is not a direct heal on the wearer"));
            continue;
        }
        let player = env.player;
        let pseudo = &env.sim.unit(player).pseudo_stats;
        let level = env.sim.unit(player).level;
        let mut exported = json!({
            "kind": "spell_data_heal_proc", "trigger_aura": label,
            "trigger_spells": proc_trigger_spells(env, &listener),
            "outcome": outcome_names(listener.outcome),
            "require_damage": listener.require_damage_dealt,
            "proc_chance": chance_or_certain(&listener), "spell": position,
            "can_crit": !heal.cannot_crit(),
            "healing_dealt_multiplier": pseudo.healing_dealt_multiplier,
            "healing_taken_multiplier": pseudo.healing_taken_multiplier,
            "table_healing_dealt_multiplier": env.attack_table(player, player).healing_dealt_multiplier,
            "bonus_healing_taken": pseudo.bonus_healing_taken,
        });
        if effect.effect_type == dbcenums::E_HEAL_PCT {
            exported["max_health_share"] = json!(effect.percent());
        } else {
            exported["average"] = json!(effect.average(level));
            exported["variance"] = json!(effect.variance);
        }
        effects.push(exported);
    }
    effects
}

/// melee_procs.go `weaponEnchantDamageProcs`: common/forever/enchants.go Fiery Weapon (803) and
/// Lifestealing (1898), and Fiery Blaze (36) from enchants_auto_gen.go: a weapon proc on landed
/// hits, at a rate of the enchanted hand, that casts the row's damage spell at once on the unit
/// hit. Only a magic hit on one target is described.
const WEAPON_ENCHANT_DAMAGE_PROCS: [(&str, i32); 3] = [
    ("Enchant Weapon - Fiery Weapon", 13897),
    ("Enchant Weapon - Lifestealing", 20004),
    ("Enchant: Fiery Blaze", 6297),
];

/// melee_procs.go `singleTargetMultiHit`: a chain or a capped or split area hit casts one roll on
/// it, and an uncapped area hit one roll times the encounter's AoE cap multiplier, which is
/// exactly one for a single target.
fn single_target_multi_hit(env: &Environment, effect: &crate::data::spells::Effect) -> bool {
    if !effect.hits_an_area() && effect.chain_targets <= 1 {
        return true;
    }
    active_target_count(env) == 1 && aoe_cap_multiplier(env) == 1.0
}

fn weapon_enchant_damage_proc_effects(
    env: &Environment,
    unrepresented: &mut Vec<String>,
) -> Vec<Value> {
    let mut effects = Vec::new();
    for (label, damage_id) in WEAPON_ENCHANT_DAMAGE_PROCS {
        let Some(aura) = aura_named(env, label) else {
            continue;
        };
        let damage = must_find(damage_id);
        let position = spell_position(env, &ActionId::spell(damage.id));
        let effect = damage.damage_effect();
        let a = env.sim.aura(aura);
        let (Some(position), Some(dpm)) = (position, a.dpm.as_ref()) else {
            unrepresented.push(not_single_target_magic_hit(label));
            continue;
        };
        if a.icd.is_some()
            || effect.is_nil()
            || !damage.periodic_damage_effect().is_nil()
            || !single_target_multi_hit(env, effect)
            || damage.applies_an_aura_to_an_enemy()
            || damage.speed != 0.0
            || env.sim.spell(spell_at(env, position)).defense_type != DefenseType::Magic
        {
            unrepresented.push(not_single_target_magic_hit(label));
            continue;
        }
        let chances = dpm_chances(env, dpm, |spell| {
            !spell.flags.matches(SpellFlag::SUPPRESS_WEAPON_PROCS)
        });
        let triggers: Vec<Value> = chances
            .iter()
            .map(|chance| chance["spell"].clone())
            .collect();
        effects.push(json!({
            "kind": "spell_data_damage_proc", "trigger_aura": a.label, "trigger_spells": triggers,
            "landed_only": true, "require_damage": false, "proc_chance": 1.0, "chances": chances,
            "spell": position, "average": effect.average(env.sim.unit(env.player).level),
            "variance": effect.variance, "can_crit": !damage.cannot_crit(),
        }));
    }
    effects
}

/// melee_procs.go `procDamageItems`: items common/shared/shared_utils.go NewProcDamageEffect
/// builds by hand: a listener on landed melee and ranged hits at a legacy proc manager's rate
/// that casts a magic hit of a Go literal range at once on the unit hit. Heart of Wyrmthalak.
const PROC_DAMAGE_ITEMS: [(&str, i32, f64, f64, ProcMask); 1] = [(
    "Heart of Wyrmthalak",
    27655,
    112.0,
    168.0,
    ProcMask::MELEE_OR_RANGED,
)];

fn proc_damage_item_effects(env: &Environment, unrepresented: &mut Vec<String>) -> Vec<Value> {
    let mut effects = Vec::new();
    for (label, spell_id, min, max, mask) in PROC_DAMAGE_ITEMS {
        let Some(aura) = aura_named(env, label) else {
            continue;
        };
        let position = spell_position(env, &ActionId::spell(spell_id));
        let a = env.sim.aura(aura);
        let (Some(position), Some(dpm)) = (position, a.dpm.as_ref()) else {
            unrepresented.push(not_single_target_magic_hit(label));
            continue;
        };
        if a.icd.is_some()
            || env.sim.spell(spell_at(env, position)).defense_type != DefenseType::Magic
            || !a.events.on_spell_hit_dealt
            || a.events.on_spell_hit_taken
            || a.events.on_periodic_damage_dealt
        {
            unrepresented.push(not_single_target_magic_hit(label));
            continue;
        }
        let listener = ProcTrigger {
            proc_mask: mask,
            outcome: HitOutcome::LANDED,
            ..ProcTrigger::default()
        };
        effects.push(json!({
            "kind": "spell_data_damage_proc", "trigger_aura": label,
            "trigger_spells": proc_trigger_spells(env, &listener),
            "landed_only": true, "require_damage": false, "proc_chance": 1.0, "spell": position,
            "average": 0.0, "variance": 0.0, "roll": [min, max], "can_crit": true,
            "chances": dpm_chances(env, dpm, |spell| {
                spell.proc_mask.matches(mask) && !spell.flags.matches(SpellFlag::PROC)
            }),
        }));
    }
    effects
}

/// melee_procs.go `setStatProcs`: set bonuses common/forever/item_sets_classic.go `setStatProc`
/// builds: a proc trigger on the set bonus aura, rolling its proc manager on the hits it hears,
/// that activates a temporary stats aura a batch window later.
fn set_stat_proc_effects(env: &Environment, unrepresented: &mut Vec<String>) -> Vec<Value> {
    let mut effects = Vec::new();
    // (set aura, trigger name, aura, aura spell, stats, proc mask)
    let procs: [(&str, &str, &str, i32, Stats, ProcMask); 1] = [(
        "Lightforge Armor 5P",
        "Item - Crusader's Wrath Proc - Lightforge Armor",
        "Crusader's Wrath",
        27499,
        Stats::from_pairs(&[(Stat::SpellDamage, 65.0), (Stat::HealingPower, 65.0)]),
        ProcMask::MELEE_WHITE_HIT,
    )];
    for (set_aura, name, aura_label, aura_id, stats, mask) in procs {
        let Some(set) = aura_named(env, set_aura) else {
            continue;
        };
        let dpm = env.sim.aura(set).dpm.as_ref();
        let (Some(dpm), true) = (dpm, aura_named(env, aura_label).is_some()) else {
            unrepresented.push(format!("{set_aura} has no proc manager"));
            continue;
        };
        let id = ActionId::spell(aura_id);
        effects.push(json!({
            "kind": "stat_proc", "trigger_aura": set_aura, "rng_label": name, "aura": aura_label,
            "chances": dpm_chances(env, dpm, |spell| {
                spell.proc_mask.matches(mask) && !spell.flags.matches(SpellFlag::PROC)
            }),
            "gain_log": format!("Gained {} from {}.", flat_string(&stats), action_id_string(&id)),
            "expire_log": format!("Lost {} from fading {}.", flat_string(&stats), action_id_string(&id)),
        }));
    }
    effects
}

/// melee_procs.go `meleeProcEffects`: the weapon, item and set procs that act on melee hits, in
/// Go's order. The classic items' effects (Crusader, Dragon's Call, Sulfuras) and the raid
/// buffs' (the Windfury totem) belong to the ports of those packages and sit between the extra
/// attack procs and the chili.
pub(crate) fn melee_proc_effects(env: &Environment, unrepresented: &mut Vec<String>) -> Vec<Value> {
    let mut effects = spell_data_damage_proc_effects(env, unrepresented);
    effects.extend(spell_data_heal_proc_effects(env, unrepresented));
    effects.extend(weapon_enchant_damage_proc_effects(env, unrepresented));
    effects.extend(proc_damage_item_effects(env, unrepresented));
    effects.extend(set_stat_proc_effects(env, unrepresented));
    // common/classic/items_weapons.go Ironfoe (11684) and common/forever/items_trinkets.go Hand
    // of Justice (11815): proc triggers on landed melee hits, Go literal chances, with the aura's
    // cooldown, whose handlers grant two and one extra main hand attacks at once.
    for (label, chance, attacks) in [
        ("Fury of Forgewright", 0.06, 2),
        ("Hand of Justice", 0.01, 1),
    ] {
        if let Some(aura) = aura_named(env, label) {
            effects.push(json!({
                "kind": "extra_attack_proc", "trigger_aura": env.sim.aura(aura).label,
                "proc_chance": chance, "attacks": attacks,
            }));
        }
    }
    if let Some(chili) = super::consumable_effects::dragonbreath_chili_effect(env) {
        effects.push(chili);
    }
    effects.extend(super::buffs::windfury_totem_effect(env, unrepresented));
    effects
}

/// The effects `prepare` appends after the common effects and the inert pets: the melee, gear,
/// spell data and energy procs, in Go's order. The stat auras follow in `export`.
pub(crate) fn item_proc_effects(env: &Environment, unrepresented: &mut Vec<String>) -> Vec<Value> {
    let mut effects = melee_proc_effects(env, unrepresented);
    effects.extend(gear_proc_effects(env, unrepresented));
    effects.extend(spell_data_stat_proc_effects(env, unrepresented));
    effects.extend(energy_proc_effects(env, unrepresented));
    effects
}

// ---------------------------------------------------------------------------------------------
// gear_procs.go and energy_procs.go.
// ---------------------------------------------------------------------------------------------

/// gear_procs.go `gearProcEffects`: shared gear procs of common/forever: Battlegear of Valor's
/// Warrior's Resolve and the Puncture Armor weapon procs.
pub(crate) fn gear_proc_effects(env: &Environment, unrepresented: &mut Vec<String>) -> Vec<Value> {
    let mut effects = Vec::new();
    // item_sets_classic.go Battlegear of Valor 5 piece, Warrior's Resolve: landed melee hits roll
    // a legacy one proc a minute manager under the trigger's name; a batch window later the
    // handler heals Roll(88, 133) and, with a rage bar, gives 10 rage, both Go literals.
    if let Some(set) = aura_named(env, "Battlegear of Valor 5P") {
        match env.sim.aura(set).dpm.as_ref() {
            None => unrepresented.push("Battlegear of Valor 5P has no proc manager".to_string()),
            Some(dpm) => {
                let rage = if env.sim.unit(env.player).rage_bar.enabled {
                    10.0
                } else {
                    0.0
                };
                effects.push(json!({
                    "kind": "health_rage_proc", "trigger_aura": env.sim.aura(set).label,
                    "rng_label": "Warrior's Resolve",
                    "chances": dpm_chances(env, dpm, |spell| {
                        spell.proc_mask.matches(ProcMask::MELEE) && !spell.flags.matches(SpellFlag::PROC)
                    }),
                    "heal_min": 88.0, "heal_max": 133.0, "rage": rage,
                    "metrics_action_id": json!({"spell_id": 450589}),
                }));
            }
        }
    }
    // items_weapons.go Bashguuder and Rivenspike: a weapon proc at two procs a minute of the
    // weapon's speed on landed hits; a batch window later the handler activates the target's
    // Puncture Armor and adds a stack, and each stack change moves the target's armor through
    // AddStatDynamic. The armor at each stack count is read from a separate reset simulation.
    for name in ["Bashguuder", "Rivenspike"] {
        let label = format!("{name} Proc");
        let Some(trigger) = aura_named(env, &label) else {
            continue;
        };
        let Some(dpm) = env.sim.aura(trigger).dpm.clone() else {
            unrepresented.push(format!(
                "{} has no proc manager",
                env.sim.aura(trigger).label
            ));
            continue;
        };
        let mut scratch = env.fresh();
        let target = scratch.encounter.targets[0];
        let Some(debuff) = scratch.sim.get_aura(target, "Puncture Armor") else {
            unrepresented.push(format!("{label} has no Puncture Armor aura"));
            continue;
        };
        let mut armor = vec![0.0];
        let base = scratch.sim.stat(target, Stat::Armor);
        scratch.sim.activate(debuff);
        for stacks in 1..=scratch.sim.aura(debuff).max_stacks {
            scratch.sim.set_stacks(debuff, stacks);
            armor.push(scratch.sim.stat(target, Stat::Armor) - base);
        }
        effects.push(json!({
            "kind": "armor_debuff_proc", "trigger_aura": label, "rng_label": label,
            "chances": dpm_chances(env, &dpm, |spell| {
                !spell.flags.matches(SpellFlag::SUPPRESS_WEAPON_PROCS)
            }),
            "aura": scratch.sim.aura(debuff).label, "armor_by_stacks": armor,
        }));
    }
    effects
}

/// energy_procs.go `energyProcEffects`: common/forever/item_sets_classic.go Shadowcraft Armor
/// (5): a set bonus proc trigger on landed white hits at one proc a minute of each hand's speed,
/// whose handler waits a spell batch window and restores 20 energy to a character with an energy
/// bar.
pub(crate) fn energy_proc_effects(
    env: &Environment,
    unrepresented: &mut Vec<String>,
) -> Vec<Value> {
    let mut effects = Vec::new();
    if let Some(aura) = aura_named(env, "Shadowcraft Armor 5P") {
        let Some(dpm) = env.sim.aura(aura).dpm.as_ref() else {
            unrepresented.push("Shadowcraft Armor's energize has no proc manager".to_string());
            return effects;
        };
        effects.push(json!({
            "kind": "energize_proc", "trigger_aura": env.sim.aura(aura).label,
            "rng_label": "Rogue Armor Energize",
            "chances": dpm_chances(env, dpm, |spell| {
                !spell.flags.matches(SpellFlag::PROC) && spell.proc_mask.matches(ProcMask::MELEE_WHITE_HIT)
            }),
            "energy": 20.0, "metrics_action_id": json!({"spell_id": 27787}),
            "delay_ns": SPELL_BATCH_WINDOW,
        }));
    }
    effects
}
