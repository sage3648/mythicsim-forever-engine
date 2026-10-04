//! Expose Armor, from Go sim/rogue/expose_armor.go: a finisher whose metrics split by the
//! combo points spent. The rogue casts it only with combo points worth at least the bid that
//! holds the target's major armor category. It rolls the special hit table without a crit; a
//! landed hit activates the debuff, which bids its armor a combo point when it is gained, then
//! applies the finisher, and Improved Expose Armor hands combo points back on a five point
//! spend. A refreshed debuff keeps the bid it was gained with. A permanent member that holds
//! the category from the reset refuses every activation, which Go still counts as a proc.

use crate::core::fight::{melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId};

use super::finisher::Finisher;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExposeArmor {
    pub(crate) aura: AuraRef,
    pub(crate) armor_per_combo_point: f64,
    pub(crate) points_back: i32,
    pub(crate) points_back_metrics: usize,
    /// The bid of the permanent member that blocks the debuff for good.
    pub(crate) blocking_priority: Option<f64>,
}

impl ExposeArmor {
    /// Go `GetExposeArmorValue`.
    fn value<A: Agent>(&self, fight: &Fight<A>) -> f64 {
        self.armor_per_combo_point * f64::from(fight.energy_bar().combo_points)
    }

    /// Go's extra cast condition with `CanApplyExposeArmorAura`.
    pub(crate) fn can_cast<A: Agent>(&self, fight: &Fight<A>) -> bool {
        if fight.energy_bar().combo_points == 0 {
            return false;
        }
        let active = match self.blocking_priority {
            Some(priority) => Some(priority),
            None => fight
                .exclusive_active_for(self.aura)
                .map(|(_, priority)| priority),
        };
        active.is_none_or(|priority| self.value(fight) >= priority)
    }

    /// Expose Armor's effect; Stealth has already broken.
    pub(crate) fn apply<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        target: Side,
        finisher: &Finisher,
    ) {
        let points = fight.energy_bar().combo_points;
        let result = fight.calc_physical_outcome(
            spell,
            target,
            PhysicalOutcome::MeleeSpecialHit { count: true },
        );
        if result.landed() {
            if self.blocking_priority.is_some() {
                fight.aura_mut(self.aura).procs += 1;
            } else {
                fight.activate_aura(self.aura);
            }
            finisher.apply(fight, spell);
            if self.points_back > 0 && points == 5 {
                fight.add_combo_points(self.points_back, self.points_back_metrics);
            }
        } else {
            fight.issue_refund(spell);
        }
        fight.deal_damage(spell, result, false);
    }

    /// The debuff's `OnGain`: it bids what the combo points about to be spent are worth.
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        let value = self.value(fight);
        fight.set_exclusive_priority(self.aura, value);
    }
}
