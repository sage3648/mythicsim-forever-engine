//! Go sim/core/procs.go: the dynamic proc manager a proc trigger or an item effect rolls its
//! procs-per-minute and fixed chances against.
//!
//! Preparation never runs a fight, so the manager is data: for each weapon hand mask, the chance
//! one hit of it procs. The exporter reads it as Go's reflection does (`procMasks`,
//! `procChances`). Go also rebuilds a manager when a weapon swap happens and keeps the mask a
//! closure computes; preparation refuses item swapping, so a manager is built once from the
//! mask the caller computed.

use super::sim::{Sim, UnitId};
use super::spell::ProcMask;

/// Go `DynamicProcManager`, with `staticProc` chances, the only `DynamicProc` core has.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DynamicProcManager {
    pub proc_masks: Vec<ProcMask>,
    pub proc_chances: Vec<f64>,
}

impl DynamicProcManager {
    /// Go `DynamicProcManager.Chance`: the chance of the first entry whose mask matches.
    pub(crate) fn chance(&self, proc_mask: ProcMask) -> f64 {
        for (mask, chance) in self.proc_masks.iter().zip(&self.proc_chances) {
            if mask.matches(proc_mask) {
                return *chance;
            }
        }
        0.0
    }
}

const PROC_MASK_MELEE_OH: u32 = ProcMask::MELEE_OH.0;
const PROC_MASK_RANGED: u32 = ProcMask::RANGED.0;

impl Sim {
    /// Go `Character.NewLegacyPPMManager`, `NewStaticLegacyPPMManager`,
    /// `NewDynamicLegacyProcForEnchant`, `NewDynamicLegacyProcForWeapon` and the other
    /// constructors: they differ only in item swap callbacks and in how the mask is computed,
    /// which the caller does.
    pub(crate) fn new_ppm_manager(
        &self,
        unit: UnitId,
        ppm: f64,
        proc_mask: ProcMask,
    ) -> DynamicProcManager {
        self.new_dynamic_weapon_proc_manager(unit, ppm, 0.0, proc_mask)
    }

    /// Go `Character.NewFixedProcChanceManager`.
    pub(crate) fn new_fixed_proc_chance_manager(
        &self,
        unit: UnitId,
        fixed_proc_chance: f64,
        proc_mask: ProcMask,
    ) -> DynamicProcManager {
        self.new_dynamic_weapon_proc_manager(unit, 0.0, fixed_proc_chance, proc_mask)
    }

    /// Go `Character.newDynamicWeaponProcManager`.
    pub(crate) fn new_dynamic_weapon_proc_manager(
        &self,
        unit: UnitId,
        ppm: f64,
        fixed_proc_chance: f64,
        proc_mask: ProcMask,
    ) -> DynamicProcManager {
        assert!(
            !(ppm != 0.0 && fixed_proc_chance != 0.0),
            "Cannot simultaneously specify both a ppm and a fixed proc chance!"
        );
        let aa = &self.unit(unit).auto_attacks;
        if !aa.auto_swing_melee && !aa.auto_swing_ranged {
            return DynamicProcManager::default();
        }
        let mut masks: Vec<ProcMask> = Vec::with_capacity(2);
        let mut chances: Vec<f64> = Vec::with_capacity(2);
        let mut merge_or_append = |speed: f64, mask: u32| {
            if speed == 0.0 || mask == 0 {
                return;
            }
            for (i, chance) in chances.iter().enumerate() {
                if *chance == speed {
                    masks[i].0 |= mask;
                    return;
                }
            }
            masks.push(ProcMask(mask));
            chances.push(speed);
        };
        let off_hand_proc_speed = if aa.oh.swing_speed == 0.0 {
            aa.mh.swing_speed
        } else {
            aa.oh.swing_speed
        };
        // "Everything else", even if not explicitly flagged main hand.
        merge_or_append(
            aa.mh.swing_speed,
            proc_mask.0 & !PROC_MASK_RANGED & !PROC_MASK_MELEE_OH,
        );
        merge_or_append(off_hand_proc_speed, proc_mask.0 & PROC_MASK_MELEE_OH);
        merge_or_append(aa.ranged.swing_speed, proc_mask.0 & PROC_MASK_RANGED);
        for chance in &mut chances {
            if fixed_proc_chance != 0.0 {
                *chance = fixed_proc_chance;
            } else {
                *chance *= ppm / 60.0;
            }
        }
        DynamicProcManager {
            proc_masks: masks,
            proc_chances: chances,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::attack::Weapon;
    use crate::prepare::sim::{Unit, UnitType};

    fn sim_with_swings(
        mh: f64,
        oh: f64,
        ranged: f64,
        melee: bool,
        auto_ranged: bool,
    ) -> (Sim, UnitId) {
        let mut sim = Sim::new();
        let id = sim.add_unit(Unit::new(UnitType::Player, "player".to_string()));
        let aa = &mut sim.unit_mut(id).auto_attacks;
        aa.auto_swing_melee = melee;
        aa.auto_swing_ranged = auto_ranged;
        aa.mh = Weapon {
            swing_speed: mh,
            ..Weapon::default()
        };
        aa.oh = Weapon {
            swing_speed: oh,
            ..Weapon::default()
        };
        aa.ranged = Weapon {
            swing_speed: ranged,
            ..Weapon::default()
        };
        (sim, id)
    }

    #[test]
    fn a_unit_that_never_swings_has_an_empty_manager() {
        let (sim, id) = sim_with_swings(2.0, 0.0, 0.0, false, false);
        let dpm = sim.new_ppm_manager(id, 6.0, ProcMask::MELEE);
        assert_eq!(dpm, DynamicProcManager::default());
        assert_eq!(dpm.chance(ProcMask::MELEE_MH_AUTO), 0.0);
    }

    #[test]
    fn equal_swing_speeds_share_one_entry() {
        let (sim, id) = sim_with_swings(2.6, 2.6, 0.0, true, false);
        let dpm = sim.new_ppm_manager(id, 6.0, ProcMask::MELEE);
        assert_eq!(dpm.proc_masks.len(), 1);
        assert_eq!(dpm.proc_masks[0], ProcMask::MELEE);
        assert_eq!(dpm.proc_chances[0], 2.6 * (6.0 / 60.0));
    }

    #[test]
    fn hands_proc_at_their_own_speed_and_a_missing_off_hand_uses_the_main_hand() {
        let (sim, id) = sim_with_swings(2.6, 1.5, 3.0, true, true);
        let dpm = sim.new_ppm_manager(id, 3.0, ProcMask::MELEE_OR_RANGED);
        assert_eq!(dpm.proc_masks.len(), 3);
        assert_eq!(dpm.chance(ProcMask::MELEE_MH_AUTO), 2.6 * (3.0 / 60.0));
        assert_eq!(dpm.chance(ProcMask::MELEE_OH_SPECIAL), 1.5 * (3.0 / 60.0));
        assert_eq!(dpm.chance(ProcMask::RANGED_AUTO), 3.0 * (3.0 / 60.0));
        assert_eq!(dpm.chance(ProcMask::SPELL_DAMAGE), 0.0);

        let (sim, id) = sim_with_swings(2.6, 0.0, 0.0, true, false);
        let dpm = sim.new_ppm_manager(id, 3.0, ProcMask::MELEE);
        assert_eq!(dpm.proc_masks.len(), 1);
    }

    #[test]
    fn a_fixed_chance_replaces_the_swing_speed() {
        let (sim, id) = sim_with_swings(2.6, 1.5, 0.0, true, false);
        let dpm = sim.new_fixed_proc_chance_manager(id, 0.2, ProcMask::MELEE);
        assert_eq!(dpm.proc_chances, vec![0.2, 0.2]);
    }

    #[test]
    #[should_panic(expected = "both a ppm and a fixed proc chance")]
    fn a_ppm_and_a_fixed_chance_are_refused() {
        let (sim, id) = sim_with_swings(2.6, 1.5, 0.0, true, false);
        sim.new_dynamic_weapon_proc_manager(id, 1.0, 0.1, ProcMask::MELEE);
    }
}
