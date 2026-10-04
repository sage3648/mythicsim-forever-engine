//! Eviscerate, from Go sim/rogue/eviscerate.go: a finisher whose metrics split by the combo
//! points spent. The rolled base, a bonus a point and 3% of attack power a point land on the
//! special table; a landed hit applies the finisher, a missed one refunds most of its energy,
//! and the damage is dealt last.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

use super::finisher::Finisher;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Eviscerate {
    pub(crate) damage_average: f64,
    pub(crate) damage_variance: f64,
    pub(crate) combo_point_damage: f64,
    pub(crate) attack_power_per_combo_point: f64,
}

impl Eviscerate {
    pub(crate) fn apply<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        target: Side,
        finisher: &Finisher,
    ) {
        let points = f64::from(fight.energy_bar().combo_points);
        let roll = fight.effect_roll(self.damage_average, self.damage_variance);
        // Go's arm64 build fuses both multiplies into their adds.
        let base = (self.attack_power_per_combo_point * points).mul_add(
            fight.melee_attack_power(),
            self.combo_point_damage.mul_add(points, roll),
        );
        let result = fight.calc_physical_damage(
            spell,
            target,
            base,
            PhysicalOutcome::MeleeSpecialHitAndCrit { count: true },
        );
        if result.landed() {
            finisher.apply(fight, spell);
        } else {
            fight.issue_refund(spell);
        }
        fight.deal_damage(spell, result, false);
    }
}
