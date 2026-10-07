//! Go sim/core/item_effects.go and item_sets.go: effects Go registers in code for items,
//! enchants and set bonuses.

use super::env::Environment;
use super::Refusal;

/// Go `Equipment.applyItemEffects` and `registerEquipSpeedAuras`.
pub(crate) fn apply_item_effects(env: &mut Environment) -> Result<(), Refusal> {
    let unit = env.player;
    let tables = crate::data::tables::tables();
    let class = env.sim.character(unit).class.clone();
    let mut seen_items = Vec::new();
    let mut seen_enchants = Vec::new();
    for item in env.sim.character(unit).equipment.clone().iter() {
        let usable = item.class_allowlist.is_empty() || item.class_allowlist.contains(&class);
        if tables.item_effect_ids.contains(&item.id) && !seen_items.contains(&item.id) && usable {
            seen_items.push(item.id);
            if !super::items_registry::apply_item_effect(env, item.id)? {
                return Err(Refusal::new(
                    "item_effect",
                    format!("item {} has an effect not prepared yet", item.id),
                ));
            }
        }
        let enchant = item.enchant.effect_id;
        if tables.enchant_effect_ids.contains(&enchant) && !seen_enchants.contains(&enchant) {
            seen_enchants.push(enchant);
            if !super::items_registry::apply_enchant_effect(env, enchant)? {
                return Err(Refusal::new(
                    "enchant_effect",
                    format!("enchant {enchant} has an effect not prepared yet"),
                ));
            }
        }
    }
    env.sim.register_equip_speed_auras(unit);
    Ok(())
}

/// Go `applyItemSetBonusEffects`: in item_sets.rs.
pub(crate) fn apply_item_set_bonus_effects(env: &mut Environment) -> Result<(), Refusal> {
    super::item_sets::apply_item_set_bonus_effects(env)
}
