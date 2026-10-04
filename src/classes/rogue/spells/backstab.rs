//! Backstab, from Go sim/rogue/backstab.go: from behind with a main hand dagger, the highest
//! rank's base plus normalized main hand damage, scaled by the weapon share in the spell's
//! damage multiplier, on the weapon special table. A landed strike gives a combo point and
//! Puncturing Wounds may add another; a missed one refunds most of its energy.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

use super::finisher::combo_point_metrics;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Backstab {
    pub(crate) base_damage: f64,
    pub(crate) main_hand_dagger: bool,
    pub(crate) extra_combo_point_chance: f64,
    pub(crate) extra_combo_point_metrics: usize,
}

impl Backstab {
    /// Go `ExtraCastCondition`.
    pub(crate) fn can_cast<A: Agent>(&self, fight: &Fight<A>) -> bool {
        !fight.config.melee.in_front_of_target && self.main_hand_dagger
    }

    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        let attack_power = fight.melee_attack_power();
        let damage = self.base_damage + fight.mh_normalized_weapon_damage(attack_power);
        let result = fight.calc_physical_damage(
            spell,
            target,
            damage,
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
        if result.landed() {
            let metrics = combo_point_metrics(fight, spell);
            fight.add_combo_points(1, metrics);
            if self.extra_combo_point_chance > 0.0
                && fight.proc(self.extra_combo_point_chance, "Puncturing Wounds")
            {
                fight.add_combo_points(1, self.extra_combo_point_metrics);
            }
        } else {
            fight.issue_refund(spell);
        }
    }
}
