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
        // Go spelldata Effect.Roll.
        let roll = if self.damage_variance == 0.0 {
            self.damage_average
        } else {
            let low = self.damage_average * (1.0 - self.damage_variance / 2.0);
            let high = self.damage_average * (1.0 + self.damage_variance / 2.0);
            low + (high - low) * fight.random("Damage Roll")
        };
        let base = roll
            + self.combo_point_damage * points
            + self.attack_power_per_combo_point * points * fight.melee_attack_power();
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
