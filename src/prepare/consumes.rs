//! Go sim/core/consumes.go.

use crate::contracts::request::Message;

use super::env::Environment;
use super::Refusal;

/// Go `applyConsumeEffects`.
pub(crate) fn apply_consume_effects(
    env: &mut Environment,
    _party_buffs: &Message,
) -> Result<(), Refusal> {
    let consumables = env.sim.character(env.player).consumables.clone();
    if let Some(field) = consumables.set_fields().first() {
        return Err(Refusal::new(
            "consumable",
            format!("consumable {field} is not prepared yet"),
        ));
    }
    Ok(())
}
