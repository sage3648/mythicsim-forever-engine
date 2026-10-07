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
fn aura_refresh_effects(env: &mut Environment, _unrepresented: &mut Vec<String>) -> Vec<Value> {
    super::buffs::aura_should_refresh_effects(env)
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
fn judgement_of_wisdom_effects(env: &Environment) -> Vec<Value> {
    super::buffs::judgement_of_wisdom_effects(env)
}

/// Touch of the Grave, Berserking, Blood Fury and Elune's Light.
fn racial_effects(_env: &mut Environment, _unrepresented: &mut Vec<String>) -> Vec<Value> {
    Vec::new()
}

/// The party's Battle Shout: `fixed_uptime_aura`.
fn battle_shout_effect(env: &Environment) -> Option<Value> {
    super::buffs::battle_shout_effect(env)
}

/// The raid's Sunder Armor: `sunder_armor_ramp`.
fn sunder_armor_effect(env: &mut Environment, unrepresented: &mut Vec<String>) -> Option<Value> {
    super::buffs::sunder_armor_effect(env, unrepresented)
}

/// Shatter Curse, Stoneform and Read Ley Line.
fn racial_defensive_effects(_env: &Environment) -> Vec<Value> {
    Vec::new()
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
    _env: &mut Environment,
    _spell: SpellId,
    _item: i32,
    _unrepresented: &mut Vec<String>,
) -> Option<Value> {
    None
}
