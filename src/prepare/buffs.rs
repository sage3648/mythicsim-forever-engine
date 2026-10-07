//! Go sim/core/buffs.go: raid, party and individual buffs.

use crate::contracts::request::Message;

use super::env::Environment;
use super::Refusal;

/// Go `applyBuffEffects`.
pub(crate) fn apply_buff_effects(
    _env: &mut Environment,
    raid_buffs: &Message,
    party_buffs: &Message,
    individual: &Message,
) -> Result<(), Refusal> {
    for (name, buffs) in [
        ("raid", raid_buffs),
        ("party", party_buffs),
        ("individual", individual),
    ] {
        if let Some(field) = buffs.set_fields().first() {
            return Err(Refusal::new(
                "buff",
                format!("{name} buff {field} is not prepared yet"),
            ));
        }
    }
    Ok(())
}
