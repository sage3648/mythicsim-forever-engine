//! Go sim/core/professions.go.

use std::rc::Rc;

use super::env::Environment;
use super::stats::Stat;

/// Go `applyProfessionEffects`.
pub(crate) fn apply_profession_effects(env: &mut Environment) {
    let unit = env.player;
    if env.sim.has_profession(unit, "Skinning") {
        env.post_finalize
            .push(Rc::new(move |env: &mut Environment| {
                for defender in env.sim.all_units() {
                    let mob_type = env.sim.unit(defender).mob_type.clone();
                    if mob_type == "MobTypeBeast" || mob_type == "MobTypeDragonkin" {
                        env.attack_table_mut(unit, defender).damage_dealt_multiplier *= 1.05;
                    }
                }
            }));
    }
    if env.sim.has_profession(unit, "Mining") {
        env.sim.unit_mut(unit).sdm.multiply_stat(Stat::Health, 1.05);
    }
}
