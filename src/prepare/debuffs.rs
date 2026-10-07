//! Go sim/core/debuffs.go and sim/core/buffs/debuffs.go: the raid's debuffs on each target.

use crate::contracts::request::Message;

use super::env::Environment;
use super::sim::UnitId;
use super::Refusal;

/// Go `applyDebuffEffects`, by way of `buffs.applyDebuffs`.
pub(crate) fn apply_debuff_effects(
    env: &mut Environment,
    target: UnitId,
    _index: usize,
    debuffs: &Message,
    raid: &Message,
) -> Result<(), Refusal> {
    super::buffs::apply_generated_debuffs(env, target, debuffs, raid)?;
    if debuffs.bool("judgement_of_the_crusader") {
        return Err(Refusal::new(
            "debuff",
            "Judgement of the Crusader is not prepared yet".to_string(),
        ));
    }
    Ok(())
}
