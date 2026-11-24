//! Go sim/core/professions.go.
//!
//! Under Forever's rules the gathering professions have no combat passive. The client (wago.tools
//! `product=wowf`, 1.60.1.71037) gives Skinning, Mining and Herbalism only their skill ranks
//! (1240952 to 1240987 and the older rank spells, which set the skill cap), gathering, smelting,
//! tracking and camp features (Camp Chair, Lodestone, Incense Candle and their upgrades). No spell
//! on those skill lines carries a damage or health aura. The 5% damage to Beasts and Dragonkin and
//! the 5% health below were read off a pre-beta demo slide and never shipped, so they apply under
//! [`Rules::GoReference`] only, which keeps the frozen Go goldens.

use std::rc::Rc;

use super::env::Environment;
use super::sim::Rules;
use super::stats::Stat;

/// Go `applyProfessionEffects`.
pub(crate) fn apply_profession_effects(env: &mut Environment) {
    if env.sim.rules == Rules::Forever {
        return;
    }
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
