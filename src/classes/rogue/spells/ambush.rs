//! Ambush, from Go sim/rogue/ambush.go: from Stealth, or under Cutthroat, with a main hand
//! dagger. The highest rank's base plus normalized main hand damage, scaled by the weapon
//! share in the spell's damage multiplier, cannot be dodged, parried or blocked. A landed hit
//! gives a combo point; a missed one refunds most of its energy.

use crate::core::fight::{melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId};

use super::{finisher::combo_point_metrics, stealth::Stealth};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ambush {
    pub(crate) base_damage: f64,
    pub(crate) main_hand_dagger: bool,
    pub(crate) cutthroat: Option<AuraRef>,
}

impl Ambush {
    /// Go `ExtraCastCondition`.
    pub(crate) fn can_cast<A: Agent>(&self, fight: &Fight<A>, stealth: Option<Stealth>) -> bool {
        if !self.main_hand_dagger {
            return false;
        }
        stealth.is_some_and(|stealth| stealth.active(fight))
            || self.cutthroat.is_some_and(|aura| fight.aura(aura).active)
    }

    /// Ambush's effect; Stealth has already broken.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        if let Some(aura) = self.cutthroat {
            fight.deactivate_aura(aura);
        }
        let attack_power = fight.melee_attack_power();
        let damage = self.base_damage + fight.mh_normalized_weapon_damage(attack_power);
        let result = fight.calc_physical_damage(
            spell,
            target,
            damage,
            PhysicalOutcome::MeleeSpecialNoBlockDodgeParry { count: true },
        );
        fight.deal_damage(spell, result, false);
        if result.landed() {
            let metrics = combo_point_metrics(fight, spell);
            fight.add_combo_points(1, metrics);
        } else {
            fight.issue_refund(spell);
        }
    }
}
