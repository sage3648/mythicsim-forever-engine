//! Go sim/core/debuffs.go: the raid's debuffs on each target.

use crate::contracts::request::Message;

use super::env::Environment;
use super::sim::UnitId;
use super::Refusal;

/// Go `applyDebuffEffects`.
pub(crate) fn apply_debuff_effects(
    _env: &mut Environment,
    _target: UnitId,
    _index: usize,
    debuffs: &Message,
    _raid: &Message,
) -> Result<(), Refusal> {
    if let Some(field) = debuffs.set_fields().first() {
        return Err(Refusal::new(
            "debuff",
            format!("debuff {field} is not prepared yet"),
        ));
    }
    Ok(())
}
