//! The item and enchant effects Go registers with `core.NewItemEffect` and
//! `core.NewEnchantEffect`, ported one by one. Each returns whether it knows the ID.

use super::env::Environment;
use super::Refusal;

pub(crate) fn apply_item_effect(_env: &mut Environment, _item: i32) -> Result<bool, Refusal> {
    Ok(false)
}

pub(crate) fn apply_enchant_effect(_env: &mut Environment, _enchant: i32) -> Result<bool, Refusal> {
    Ok(false)
}
