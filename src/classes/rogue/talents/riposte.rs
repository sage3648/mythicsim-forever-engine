//! Riposte, from Go sim/rogue/talents_combat.go `registerRiposte`: a parry of the target's
//! attack readies Riposte at once, and Riposte spends the ready aura on a main hand weapon
//! strike that rolls the special table with a crit. It neither breaks Stealth nor refunds.

use crate::core::fight::{
    melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId, SpellResult, OUTCOME_PARRY,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Riposte {
    pub(crate) ready: AuraRef,
}

impl Riposte {
    /// The trigger's `OnSpellHitTaken`: a parry activates the ready aura.
    pub(crate) fn on_hit_taken<A: Agent>(&self, fight: &mut Fight<A>, result: &SpellResult) {
        if result.outcome & OUTCOME_PARRY != 0 {
            fight.activate_aura(self.ready);
        }
    }

    /// Go's extra cast condition.
    pub(crate) fn can_cast<A: Agent>(&self, fight: &Fight<A>) -> bool {
        fight.aura(self.ready).active
    }

    /// Riposte's effect.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        fight.deactivate_aura(self.ready);
        let attack_power = fight.melee_attack_power();
        let damage = fight.mh_weapon_damage(attack_power);
        let result = fight.calc_physical_damage(
            spell,
            target,
            damage,
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
    }
}
