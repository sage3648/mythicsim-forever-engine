//! Mutilate, from Go sim/rogue/talents_assassination.go `registerMutilate`: with two daggers,
//! the parent rolls the special hit table without a crit. A landed roll gives two combo points,
//! then casts the off hand hit and the main hand hit; a missed one refunds most of its energy.
//! The outcome is dealt last. Each hit adds the rank's flat damage to normalized weapon damage
//! on the weapon special table, harder while one of the rogue's lingering poisons is on the
//! target.

use crate::core::fight::{melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId};

use super::finisher::combo_point_metrics;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Mutilate {
    pub(crate) flat_damage: f64,
    pub(crate) poison_bonus: f64,
    pub(crate) combo_points: i32,
    pub(crate) daggers: bool,
    pub(crate) main_hand: SpellId,
    pub(crate) off_hand: SpellId,
}

impl Mutilate {
    /// The parent's effect; Stealth has already broken.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        let result = fight.calc_physical_outcome(
            spell,
            target,
            PhysicalOutcome::MeleeSpecialHit { count: true },
        );
        if result.landed() {
            let metrics = combo_point_metrics(fight, spell);
            fight.add_combo_points(self.combo_points, metrics);
            fight.cast(self.off_hand, target);
            fight.cast(self.main_hand, target);
        } else {
            fight.issue_refund(spell);
        }
        fight.deal_damage(spell, result, false);
    }

    /// A hand's hit. Go raises the spell's damage multiplier for the hit and restores the
    /// value it saved.
    pub(crate) fn hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        target: Side,
        poison: Option<AuraRef>,
    ) {
        let attack_power = fight.melee_attack_power();
        let weapon = if spell == self.main_hand {
            fight.mh_normalized_weapon_damage(attack_power)
        } else {
            fight.oh_normalized_weapon_damage(attack_power)
        };
        let base = self.flat_damage + weapon;
        let saved = fight.spells[spell].damage_multiplier;
        if poison.is_some_and(|aura| fight.aura(aura).active) {
            fight.spells[spell].damage_multiplier *= 1.0 + self.poison_bonus;
        }
        let result = fight.calc_physical_damage(
            spell,
            target,
            base,
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
        fight.spells[spell].damage_multiplier = saved;
    }
}
