//! The class side of preparation: Go's `Agent` interface and the oracle exporter's per-class
//! `classExport`, in one trait each class implements under `src/classes/<class>/prepare`.

use crate::contracts::prepared_v2::ActionId;
use crate::contracts::request::Message;

use super::sim::{Sim, UnitId};

/// Go `ProtoToActionID`.
pub(crate) fn proto_to_action_id(id: &Message) -> ActionId {
    let mut out = ActionId {
        tag: id.i32("tag"),
        ..ActionId::default()
    };
    if let Some((field, _)) = id.oneof("raw_id") {
        match field {
            "spell_id" => out.spell_id = id.i32("spell_id"),
            "item_id" => out.item_id = id.i32("item_id"),
            "other_id" => {
                let number = id.enum_number("other_id");
                if number != 0 {
                    out.other_id = id.enum_name("other_id");
                }
            }
            _ => {}
        }
    }
    out
}

/// A class mask bit's stable name, as the exporter's `classSpellName`.
pub(crate) struct ClassSpellName {
    pub mask: i64,
    pub name: &'static str,
}

/// One class: construction and initialization as Go's agent runs them, and the exporter's
/// description of what Go keeps in closures.
pub(crate) trait PrepAgent {
    /// Go `Agent.AddRaidBuffs`: the buffs the class brings to the raid.
    fn add_raid_buffs(&self, _raid_buffs: &mut Message) {}
    /// Go `Agent.AddPartyBuffs`.
    fn add_party_buffs(&self, _party_buffs: &mut Message) {}
    /// Go `Agent.ApplyTalents`.
    fn apply_talents(&mut self, _sim: &mut Sim, _unit: UnitId) {}
    /// Go `Agent.Initialize`.
    fn initialize(&mut self, _sim: &mut Sim, _unit: UnitId) {}
    /// Go `Agent.Reset`.
    fn reset(&mut self, _sim: &mut Sim, _unit: UnitId) {}
    /// A `core.NewItemEffect` the class package registers for an item: applies it and answers
    /// true, or answers false for an item the class registers no effect for. Called after the
    /// shared effects (`items_registry`), in Go's equipment order.
    fn apply_item_effect(&mut self, _sim: &mut Sim, _unit: UnitId, _item: i32) -> bool {
        false
    }
    /// A `core.NewEnchantEffect` the class package registers for an enchant.
    fn apply_enchant_effect(&mut self, _sim: &mut Sim, _unit: UnitId, _enchant: i32) -> bool {
        false
    }

    /// The talents proto the class filled from the talent string.
    fn talents(&self) -> &Message;
    /// Stable names for the class's spell mask bits.
    fn class_spells(&self) -> &'static [ClassSpellName];
    /// The exporter's class effects, in its order.
    fn effects(&self, _sim: &Sim, _unit: UnitId) -> Vec<serde_json::Value> {
        Vec::new()
    }
    /// Stable names for class spells Go registers without a class mask, by action.
    fn unmasked_spell(&self, _id: &ActionId) -> Option<&'static str> {
        None
    }
    /// Whether the class's main hand swing replacement always keeps the swing it is given.
    fn swing_replacement_keeps_swing(&self) -> bool {
        false
    }
    /// Go `EurekaAgent.EurekaSpells`: the spell masks the Gnome racial Eureka! names for the
    /// class. A class that does not implement it leaves the racial on every class spell.
    fn eureka_spells(&self) -> Option<super::racials::EurekaSpells> {
        None
    }
    /// Go `Agent.NewAPLAction`, asked first for every rotation action: `Some(true)` when the
    /// class builds the action itself, `Some(false)` when it builds it as nil, `None` to leave
    /// it to the core actions.
    fn custom_apl_action(&self, _sim: &Sim, _unit: UnitId, _action: &Message) -> Option<bool> {
        None
    }
    /// Whether a spell is one of the class's mana gems, which the class describes itself:
    /// the item loop skips `spell.Matches(mage.MageSpellManaGem)`.
    fn is_mana_gem(&self, _sim: &Sim, _spell: super::sim::SpellId) -> bool {
        false
    }
    /// The client damage roll of a spell, `{average, variance}`, for the spells the class names.
    fn damage_effect(&self, _sim: &Sim, _spell: super::sim::SpellId) -> Option<serde_json::Value> {
        None
    }
}

/// Go `FillTalentsProto`: each digit sets the field numbered by its position, counting each
/// tree from the sum of the earlier trees' sizes.
pub(crate) fn fill_talents(
    type_name: &str,
    talents: &str,
    tree_sizes: [usize; 3],
) -> Result<Message, String> {
    use serde_json::{Map, Value};
    let mut object = Map::new();
    let mut offset = 0;
    for (tree, digits) in talents.split('-').enumerate() {
        for (i, digit) in digits.chars().enumerate() {
            // Go: strconv.Atoi, whose error is ignored, reads a non-digit as 0.
            let points = digit.to_digit(10).map_or(0, i64::from);
            let number = (offset + i + 1) as u32;
            let (name, is_bool) = crate::contracts::request::field_by_number(type_name, number)
                .ok_or_else(|| {
                    format!(
                        "Couldn't find proto field for talent #{number}, full string: {talents}"
                    )
                })?;
            let value = if is_bool {
                Value::Bool(points == 1)
            } else {
                Value::from(points)
            };
            object.insert(name, value);
        }
        offset += tree_sizes.get(tree).copied().unwrap_or(0);
    }
    Message::from_json_text(type_name, &Value::Object(object).to_string())
}
