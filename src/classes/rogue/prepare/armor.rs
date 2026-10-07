//! Go sim/rogue/expose_armor.go: the Rogue's own Expose Armor debuff on each enemy, and the
//! exporter's description of how it bids in the major armor category
//! (tools/oracle-v2/rogue_specs.go `rogueExposeArmorEffects`).

use std::cell::Cell;
use std::rc::Rc;

use serde_json::{json, Value};

use crate::prepare::buffs::{Meta, MetaKind};
use crate::prepare::env::Environment;
use crate::prepare::sim::{AuraConfig, AuraId, EffectId, Sim, UnitId, NEVER_EXPIRES};
use crate::prepare::stats::Stat;

use super::spell_data::spell_data;
use super::util::spell_action;
use super::Rogue;

/// Go `buffs.ExposeArmorCategory`.
const EXPOSE_ARMOR_CATEGORY: &str = "MajorArmorReduction";

/// Go's generated `exposeArmorMeta`, restated: the raid's Expose Armor debuff.
const EXPOSE_ARMOR_META: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Expose Armor",
    spell: 11198,
    category: EXPOSE_ARMOR_CATEGORY,
    single_aura: true,
    full_combo_points: true,
    ..Meta::DEFAULT
};

/// The combo points a reset leaves the bar with.
const RESET_COMBO_POINTS: f64 = 0.0;

/// Go `exposeArmorPerComboPoint`: client 11198 is 450 armor a combo point, and the generated
/// raid debuff is priced at five of them.
pub(super) fn expose_armor_per_combo_point() -> f64 {
    EXPOSE_ARMOR_META.value(0).abs() / 5.0
}

/// Go `Rogue.exposeArmorAura`: the same label, id, duration and single aura category as the
/// generated player copy, bidding the armor the cast's combo points are worth.
pub(super) fn expose_armor_aura(sim: &mut Sim, target: UnitId) -> AuraId {
    let rank = spell_data().expose_armor.highest();
    let effect: Rc<Cell<Option<EffectId>>> = Rc::new(Cell::new(None));
    let gain_effect = Rc::clone(&effect);
    let aura = sim.get_or_register_aura(
        target,
        AuraConfig {
            label: "Expose Armor (Player)".to_string(),
            tag: EXPOSE_ARMOR_CATEGORY.to_string(),
            action_id: Some(spell_action(rank.id)),
            duration: EXPOSE_ARMOR_META.duration(0),
            on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                if let Some(effect) = gain_effect.get() {
                    sim.set_effect_priority(
                        effect,
                        expose_armor_per_combo_point() * RESET_COMBO_POINTS,
                    );
                }
            })),
            ..AuraConfig::default()
        },
    );
    let armor_of = |sim: &Sim, effect: EffectId| {
        let aura = sim.effects[effect.0].aura;
        (sim.aura(aura).unit, sim.effects[effect.0].priority)
    };
    let id = sim.new_exclusive_effect(
        aura,
        EXPOSE_ARMOR_CATEGORY,
        true,
        0.0,
        Some(Rc::new(move |sim: &mut Sim, effect| {
            let (unit, priority) = armor_of(sim, effect);
            sim.add_stat_dynamic(unit, Stat::Armor, -priority);
        })),
        Some(Rc::new(move |sim: &mut Sim, effect| {
            let (unit, priority) = armor_of(sim, effect);
            sim.add_stat_dynamic(unit, Stat::Armor, priority);
        })),
    );
    effect.set(Some(id));
    aura
}

/// Go `Unit.Armor`.
fn armor(sim: &Sim, unit: UnitId) -> f64 {
    let unit = sim.unit(unit);
    unit.pseudo_stats.armor_multiplier * unit.stats[Stat::Armor]
}

/// tools/oracle-v2/main.go `targetArmorWithStacks`: the target's armor with the named aura active
/// at the given stacks, from a separate reset simulation. NaN when a stronger member of the
/// armor category keeps the aura out.
fn target_armor_with_stacks(env: &Environment, label: &str, stacks: i32) -> f64 {
    let mut fresh = env.fresh();
    let target = fresh.encounter.targets[0];
    if stacks > 0 {
        let Some(aura) = fresh.sim.get_aura(target, label) else {
            return f64::NAN;
        };
        fresh.sim.activate(aura);
        if !fresh.sim.aura(aura).active {
            return f64::NAN;
        }
        fresh.sim.set_stacks(aura, stacks);
    }
    armor(&fresh.sim, target)
}

/// tools/oracle-v2/warrior.go `stackBid`: what one stack of a target aura bids in its exclusive
/// category, from a separate reset simulation. Zero when the aura cannot activate alone.
fn stack_bid(env: &Environment, label: &str, category: &str) -> f64 {
    let mut fresh = env.fresh();
    let target = fresh.encounter.targets[0];
    let Some(aura) = fresh.sim.get_aura(target, label) else {
        return 0.0;
    };
    fresh.sim.activate(aura);
    if !fresh.sim.aura(aura).active {
        return 0.0;
    }
    fresh.sim.set_stacks(aura, 1);
    for effect in &fresh.sim.aura(aura).exclusive_effects {
        let effect = &fresh.sim.effects[effect.0];
        if fresh.sim.categories[effect.category.0].name == category {
            return effect.priority;
        }
    }
    0.0
}

/// tools/oracle-v2/warrior.go `blockedForGood`: whether an aura can never activate, each of its
/// exclusive categories being a single aura category held by another aura that never expires.
fn blocked_for_good(sim: &Sim, aura: AuraId) -> bool {
    let effects = &sim.aura(aura).exclusive_effects;
    for effect in effects {
        let category = &sim.categories[sim.effects[effect.0].category.0];
        match category.active_effect {
            Some(active)
                if active != *effect
                    && sim.aura(sim.effects[active.0].aura).duration == NEVER_EXPIRES
                    && category.single_aura => {}
            _ => return false,
        }
    }
    !effects.is_empty()
}

/// tools/oracle-v2/warrior.go `exclusiveCategoryEffect`: a single aura category of the unit, with
/// each member's aura, bid and spell in registration order.
fn exclusive_category_effect(
    sim: &Sim,
    unit: UnitId,
    side: &str,
    name: &str,
    unrepresented: &mut Vec<String>,
) -> Option<Value> {
    for category in &sim.unit(unit).categories {
        let category = &sim.categories[category.0];
        if category.name != name {
            continue;
        }
        if !category.single_aura {
            unrepresented.push(format!("exclusive category {name} holds several auras"));
            return None;
        }
        let members: Vec<Value> = category
            .effects
            .iter()
            .map(|effect| {
                let effect = &sim.effects[effect.0];
                let aura = sim.aura(effect.aura);
                json!({
                    "aura": aura.label,
                    "priority": effect.priority,
                    "spell_id": aura.action_id.as_ref().map_or(0, |action| action.spell_id),
                })
            })
            .collect();
        return Some(json!({
            "kind": "exclusive_category", "unit": side, "category": name, "members": members,
        }));
    }
    None
}

impl Rogue {
    /// tools/oracle-v2/rogue_specs.go `rogueExposeArmorEffects`: a finisher whose debuff bids
    /// 450 armor a combo point and takes that much armor off the target while it holds the
    /// target's major armor category. Improved Expose Armor hands combo points back on a five
    /// point spend. A permanent member that holds the category from the reset blocks the debuff
    /// for good, and the export carries its bid instead.
    pub(super) fn expose_armor_effects(
        &self,
        env: &Environment,
        unrepresented: &mut Vec<String>,
    ) -> Vec<Value> {
        let sim = &env.sim;
        if self.spells.expose_armor.is_none() {
            return Vec::new();
        }
        let target = env.encounter.targets[0];
        let index = sim.unit(target).unit_index as usize;
        let Some(aura) = self.auras.expose_armor.get(index).copied().flatten() else {
            return Vec::new();
        };
        if sim.aura(aura).exclusive_effects.len() != 1 {
            unrepresented.push("Expose Armor holds several exclusive effects".to_string());
            return Vec::new();
        }
        let category = sim.effects[sim.aura(aura).exclusive_effects[0].0].category;
        let category_name = sim.categories[category.0].name.clone();
        let label = sim.aura(aura).label.clone();
        let points_back = spell_data()
            .improved_expose_armor
            .effect_at(2)
            .value_at(self.talents.i32("improved_expose_armor"));
        let effect = json!({
            "kind": "expose_armor",
            "spell_id": sim.spell(self.spells.expose_armor.expect("checked")).action_id.spell_id,
            "aura": label,
            "armor_per_combo_point": expose_armor_per_combo_point(),
            "points_back": points_back as i32,
            "points_back_action": {"spell_id": 14169},
        });
        let mut effect = effect;
        let active = sim.categories[category.0].active_effect;
        if blocked_for_good(sim, aura) {
            let holder = active.map_or(0.0, |active| sim.effects[active.0].priority);
            effect["blocking_priority"] = json!(holder);
            return vec![effect];
        }
        if let Some(active) = active {
            let holder = &sim.aura(sim.effects[active.0].aura).label;
            unrepresented.push(format!(
                "the major armor category holds {holder} from the reset"
            ));
            return Vec::new();
        }
        let Some(mut exclusive) =
            exclusive_category_effect(sim, target, "target", &category_name, unrepresented)
        else {
            return Vec::new();
        };
        let mut armor_by_stacks = vec![target_armor_with_stacks(env, &label, 0)];
        let members = exclusive["members"].as_array().cloned().unwrap_or_default();
        let mut with_bids = Vec::new();
        for mut member in members {
            let member_label = member["aura"].as_str().unwrap_or_default().to_string();
            let stacking = sim
                .get_aura(target, &member_label)
                .map(|other| sim.aura(other).max_stacks)
                .filter(|max| *max > 0);
            if let Some(max_stacks) = stacking {
                if armor_by_stacks.len() > 1 {
                    unrepresented.push(
                        "the major armor category holds several stacking debuffs".to_string(),
                    );
                    return Vec::new();
                }
                for stacks in 1..=max_stacks {
                    armor_by_stacks.push(target_armor_with_stacks(env, &member_label, stacks));
                }
                member["per_stack"] = json!(stack_bid(env, &member_label, &category_name));
            }
            with_bids.push(member);
        }
        if armor_by_stacks.iter().any(|armor| armor.is_nan()) {
            unrepresented
                .push("a stacking armor debuff cannot activate beside Expose Armor".to_string());
            return Vec::new();
        }
        exclusive["members"] = Value::Array(with_bids);
        exclusive["armor_by_stacks"] = json!(armor_by_stacks);
        vec![effect, exclusive]
    }
}
