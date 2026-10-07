//! The item and enchant effects Go registers with `core.NewItemEffect` and
//! `core.NewEnchantEffect`, ported one by one. Each returns whether it knows the ID.
//!
//! Go keeps one global registry. Here the shared effects (sim/common) come first and the class
//! packages' own effects answer through the agent (`PrepAgent::apply_item_effect`).

use super::env::Environment;
use super::Refusal;

pub(crate) fn apply_item_effect(env: &mut Environment, item: i32) -> Result<bool, Refusal> {
    if super::forever_items::apply_item_effect(env, item)? {
        return Ok(true);
    }
    if super::classic_items::apply_item_effect(env, item)? {
        return Ok(true);
    }
    if super::classic_weapons::apply_item_effect(env, item)? {
        return Ok(true);
    }
    let unit = env.player;
    Ok(env.agent.apply_item_effect(&mut env.sim, unit, item))
}

pub(crate) fn apply_enchant_effect(env: &mut Environment, enchant: i32) -> Result<bool, Refusal> {
    if super::forever_items::apply_enchant_effect(env, enchant)? {
        return Ok(true);
    }
    if super::classic_enchants::apply_enchant_effect(env, enchant) {
        return Ok(true);
    }
    let unit = env.player;
    Ok(env.agent.apply_enchant_effect(&mut env.sim, unit, enchant))
}
