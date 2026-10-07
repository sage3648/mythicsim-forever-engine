//! Go sim/core/attack.go `WeaponFromMainHand`, `WeaponFromOffHand` and `WeaponFromRanged`: the
//! weapons a character's equipped items make.

use crate::prepare::attack::Weapon;
use crate::prepare::sim::{Sim, UnitId};

/// Go `Character.WeaponFromMainHand`.
pub(super) fn weapon_from_main_hand(sim: &Sim, unit: UnitId) -> Weapon {
    match sim.mh_weapon(unit) {
        Some(item) => Weapon::from_item(item, sim.unit(unit).pseudo_stats.bonus_mh_dps),
        None => Weapon::unarmed(),
    }
}

/// Go `Character.WeaponFromOffHand`.
pub(super) fn weapon_from_off_hand(sim: &Sim, unit: UnitId) -> Weapon {
    match sim.oh_weapon(unit) {
        Some(item) => Weapon::from_item(item, sim.unit(unit).pseudo_stats.bonus_oh_dps),
        None => Weapon::default(),
    }
}

/// Go `Character.WeaponFromRanged`.
pub(super) fn weapon_from_ranged(sim: &Sim, unit: UnitId) -> Weapon {
    match sim.ranged_weapon(unit) {
        Some(item) => Weapon::from_item(item, sim.unit(unit).pseudo_stats.bonus_ranged_dps),
        None => Weapon::default(),
    }
}
