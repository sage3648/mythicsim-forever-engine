//! The class-independent effects the exporter describes: tools/oracle-v2/main.go
//! `commonEffects`, in its order. Each section is a function of its own, named for the Go it
//! ports, so the parts can be ported and reviewed separately.

use serde_json::{json, Value};

use super::env::Environment;
use super::sim::SpellId;
use super::spell::SpellFlag;

/// Go `commonEffects`.
pub(crate) fn common_effects(env: &mut Environment, unrepresented: &mut Vec<String>) -> Vec<Value> {
    let mut effects = Vec::new();
    effects.extend(aura_refresh_effects(env, unrepresented));
    effects.extend(player_movement_effect(env));
    effects.extend(judgement_of_wisdom_effects(env));
    effects.extend(racial_effects(env, unrepresented));
    effects.extend(battle_shout_effect(env));
    effects.extend(sunder_armor_effect(env, unrepresented));
    effects.extend(racial_defensive_effects(env));
    effects.extend(item_use_effects(env, unrepresented));
    effects
}

/// tools/oracle-v2/aura_refresh.go `auraShouldRefreshEffects`.
fn aura_refresh_effects(_env: &mut Environment, _unrepresented: &mut Vec<String>) -> Vec<Value> {
    Vec::new()
}

/// tools/oracle-v2/movement.go `playerMovementEffect`: the movement speed a prepull move runs at.
fn player_movement_effect(env: &Environment) -> Option<Value> {
    let player = env.player;
    let rotation = env
        .sim
        .character(player)
        .player
        .message("rotation")?
        .clone();
    let moves = rotation.messages("prepull_actions").iter().any(|prepull| {
        prepull
            .message("action")
            .is_some_and(|action| action.message("move").is_some())
    });
    if !moves {
        return None;
    }
    let mut speed_auras = Vec::new();
    for aura in &env.sim.unit(player).auras {
        let a = env.sim.aura(*aura);
        let changes = a.label == "Elemental Blessing"
            || a.exclusive_effects.iter().any(|effect| {
                let name = &env.sim.categories[env.sim.effects[effect.0].category.0].name;
                name == "PassiveMovementSpeed" || name == "ActiveMovementSpeed"
            });
        if changes {
            speed_auras.push(a.label.clone());
        }
    }
    Some(json!({
        "kind": "player_movement",
        "speed_multiplier": env.sim.unit(player).pseudo_stats.movement_speed_multiplier,
        "speed_auras": speed_auras,
    }))
}

/// The `Judgement of Wisdom (External)` loop over the target's auras.
fn judgement_of_wisdom_effects(_env: &Environment) -> Vec<Value> {
    Vec::new()
}

/// The player stats an aura changes while it is active, read from a separate reset simulation
/// so the exported one is untouched: tools/oracle-v2 `activeStats`.
pub(crate) fn active_stats(env: &Environment, label: &str) -> Value {
    let mut fresh = env.fresh();
    let player = fresh.player;
    let before = fresh.sim.stats(player);
    let aura = fresh
        .sim
        .get_aura(player, label)
        .expect("the aura exists in every reset");
    fresh.sim.activate(aura);
    let after = fresh.sim.stats(player);
    let mut changed = serde_json::Map::new();
    for stat in super::stats::Stat::ALL {
        if after[stat] != before[stat] {
            changed.insert(stat.name().to_string(), json!(after[stat]));
        }
    }
    Value::Object(changed)
}

/// Go `Stats.FlatString`: every nonzero stat as `"Name": 0.000,` in stat order.
pub(crate) fn flat_string(stats: &super::stats::Stats) -> String {
    let mut out = String::from("{");
    for stat in super::stats::Stat::ALL {
        let value = stats[stat];
        if value != 0.0 {
            out.push_str(&format!("\"{}\": {:.3},", stat.name(), value));
        }
    }
    out.push('}');
    out
}

/// Go `ActionID.String`.
pub(crate) fn action_id_string(id: &crate::contracts::prepared_v2::ActionId) -> String {
    let mut out = String::from("{");
    if id.spell_id != 0 {
        out.push_str(&format!("SpellID: {}", id.spell_id));
    } else if id.item_id != 0 {
        out.push_str(&format!("ItemID: {}", id.item_id));
    } else if !id.other_id.is_empty() {
        let number =
            crate::contracts::request::enum_number("proto.OtherAction", &id.other_id).unwrap_or(0);
        out.push_str(&format!("OtherID: {number}"));
    }
    if id.tag != 0 {
        out.push_str(&format!(", Tag: {}", id.tag));
    }
    out.push('}');
    out
}

/// Touch of the Grave, Berserking, Blood Fury and Elune's Light.
fn racial_effects(env: &mut Environment, _unrepresented: &mut Vec<String>) -> Vec<Value> {
    use super::spell::ProcMask;
    let player = env.player;
    let mut effects = Vec::new();
    if env.sim.get_aura(player, "Touch of the Grave").is_some() {
        let chance = match env.sim.character(player).class.as_str() {
            "ClassWarrior" | "ClassPaladin" | "ClassRogue" => 0.05,
            _ => 0.1,
        };
        effects.push(json!({
            "kind": "touch_of_the_grave", "trigger_aura": "Touch of the Grave", "drain_spell_id": 1260198,
            "proc_chance": chance,
            "proc_mask": (ProcMask::MELEE | ProcMask::RANGED | ProcMask::SPELL_DAMAGE).names(),
            "health_fraction": 0.05, "delay_ns": SPELL_BATCH_WINDOW,
        }));
    }
    if let Some(aura) = env.sim.get_aura(player, "Berserking") {
        let a = env.sim.aura(aura);
        effects.push(json!({
            "kind": "berserking", "spell_id": a.action_id.as_ref().map_or(0, |id| id.spell_id),
            "aura": a.label, "cast_speed_multiplier": 1.1, "attack_speed_multiplier": 1.1,
        }));
    }
    if let Some(aura) = env.sim.get_aura(player, "Blood Fury") {
        let (label, spell_id) = {
            let a = env.sim.aura(aura);
            (
                a.label.clone(),
                a.action_id.as_ref().map_or(0, |id| id.spell_id),
            )
        };
        effects.push(json!({
            "kind": "blood_fury", "spell_id": spell_id, "aura": label,
            "active_stats": active_stats(env, &label),
        }));
    }
    if let Some(aura) = env.sim.get_aura(player, "Elune's Light") {
        let (label, id) = {
            let a = env.sim.aura(aura);
            (a.label.clone(), a.action_id.clone().unwrap_or_default())
        };
        let mut buffs = super::stats::Stats::default();
        buffs[super::stats::Stat::PhysicalCritPercent] = 10.0;
        buffs[super::stats::Stat::SpellCritPercent] = 10.0;
        effects.push(json!({
            "kind": "temporary_stats", "spell_id": id.spell_id, "aura": label,
            "active_stats": active_stats(env, &label),
            "gain_log": format!("Gained {} from {}.", flat_string(&buffs), action_id_string(&id)),
            "expire_log": format!("Lost {} from fading {}.", flat_string(&buffs), action_id_string(&id)),
        }));
    }
    effects
}

/// Go `core.SpellBatchWindow`.
pub(crate) const SPELL_BATCH_WINDOW: i64 = 10 * super::sim::MILLISECOND;

/// The party's Battle Shout: `fixed_uptime_aura`.
fn battle_shout_effect(_env: &Environment) -> Option<Value> {
    None
}

/// The raid's Sunder Armor: `sunder_armor_ramp`.
fn sunder_armor_effect(_env: &mut Environment, _unrepresented: &mut Vec<String>) -> Option<Value> {
    None
}

/// Shatter Curse, Stoneform and Read Ley Line.
fn racial_defensive_effects(env: &Environment) -> Vec<Value> {
    let player = env.player;
    let mut effects = Vec::new();
    let spell_id = |aura: super::sim::AuraId| {
        env.sim
            .aura(aura)
            .action_id
            .as_ref()
            .map_or(0, |id| id.spell_id)
    };
    if let Some(aura) = env.sim.get_aura(player, "Shatter Curse") {
        effects.push(json!({"kind": "shatter_curse", "spell_id": spell_id(aura), "aura": env.sim.aura(aura).label,
            "school_damage_taken_multiplier": 0.85,
            "schools": ["arcane", "fire", "frost", "holy", "nature", "shadow"]}));
    }
    if let Some(aura) = env.sim.get_aura(player, "Stoneform") {
        effects.push(json!({"kind": "stoneform", "spell_id": spell_id(aura), "aura": env.sim.aura(aura).label,
            "school_damage_taken_multiplier": 0.9, "schools": ["physical"]}));
    }
    if let Some(aura) = env.sim.get_aura(player, "Energized") {
        for spell in &env.sim.unit(player).spellbook {
            if env.sim.spell(*spell).related_self_buff == Some(aura) {
                effects.push(json!({"kind": "read_ley_line", "spell_id": env.sim.spell(*spell).action_id.spell_id,
                    "aura": env.sim.aura(aura).label, "regen_multiplier": 2.0}));
            }
        }
    }
    effects
}

/// The major cooldown items, then the potions, conjured items and Diamond Flasks a rotation
/// casts itself: the item loop at the end of `commonEffects`. Each item is described by the
/// first case that claims it, in Go's order.
fn item_use_effects(env: &mut Environment, unrepresented: &mut Vec<String>) -> Vec<Value> {
    let player = env.player;
    let mut items: Vec<SpellId> = env
        .sim
        .character(player)
        .initial_major_cooldowns
        .iter()
        .map(|mcd| mcd.spell)
        .collect();
    let cooldowns = items.clone();
    for spell in env.sim.unit(player).spellbook.clone() {
        let s = env.sim.spell(spell);
        let item = s.action_id.item_id;
        let sapper = s.action_id.item_id == GOBLIN_SAPPER_ITEM && s.action_id.tag == 0;
        if item != 0
            && !cooldowns.contains(&spell)
            && (s.flags.matches(SpellFlag::POTION | SpellFlag::CONJURED)
                || sapper
                || item == DIAMOND_FLASK_ITEM)
        {
            items.push(spell);
        }
    }
    let mut effects = Vec::new();
    for spell in items {
        let s = env.sim.spell(spell);
        let item = s.action_id.item_id;
        // Mage gems are described by the class's mana_gems effect.
        if item == 0 || env.agent.is_mana_gem(&env.sim, spell) {
            continue;
        }
        if let Some(effect) = consumable_item_effect(env, spell, item, unrepresented) {
            effects.push(effect);
            continue;
        }
        if let Some(effect) = use_item_effect(env, spell, item, unrepresented) {
            effects.push(effect);
            continue;
        }
        unrepresented.push(format!("major cooldown item {item} has no exported effect"));
    }
    effects
}

/// Go's `GoblinSapperActionID` item.
pub(crate) const GOBLIN_SAPPER_ITEM: i32 = 10646;
/// The Diamond Flask, a Warrior item the loop lists beside the consumables.
pub(crate) const DIAMOND_FLASK_ITEM: i32 = 20130;

/// The consumable cases of the item loop: potions, conjured items, the Goblin Sapper Charge and
/// the basic explosives. `None` for an item no consumable case claims.
fn consumable_item_effect(
    _env: &mut Environment,
    _spell: SpellId,
    _item: i32,
    _unrepresented: &mut Vec<String>,
) -> Option<Value> {
    None
}

/// The item cases of the item loop: temporary stats, speed, damage, survival and energize on
/// use, Burst of Knowledge, Second Wind and class item uses. `None` for an item none claims.
fn use_item_effect(
    env: &mut Environment,
    spell: SpellId,
    item: i32,
    _unrepresented: &mut Vec<String>,
) -> Option<Value> {
    // The last case of Go's switch before the energize default: `classItemUseEffects[item]`.
    env.agent.class_item_use_effect(env, spell, item)
}
