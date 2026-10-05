//! Blade Flurry (13877), from Go sim/rogue/talents_combat.go `registerBladeFlurry`: the cast
//! activates an aura that multiplies attack speed while it lasts. Against two or more targets
//! each melee hit that deals damage casts the extra hit (22482) on the next target for the
//! same damage, which always hits and ignores resists and modifiers; Go then takes the cast
//! back off the target of the hit that triggered it.

use crate::core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct BladeFlurry {
    pub(crate) aura: AuraRef,
    pub(crate) attack_speed_multiplier: f64,
    pub(crate) hit: SpellId,
}

impl BladeFlurry {
    /// The aura's `OnSpellHitDealt`; returns the damage the extra hit deals, which it casts.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) -> Option<f64> {
        if fight.targets.len() < 2 || result.damage == 0.0 || !fight.spells[spell].melee_proc {
            return None;
        }
        Some(result.damage)
    }

    /// The extra hit's cast on the target after the one hit, and the cast taken back off it.
    pub(crate) fn strike<A: Agent>(&self, fight: &mut Fight<A>, hit_target: Side) {
        let next = fight.next_target(hit_target);
        fight.cast(self.hit, next);
        fight.spells[self.hit].metrics[hit_target.index()].casts -= 1;
    }

    /// The extra hit's `ApplyEffects`: `CalcAndDealDamage` with `OutcomeAlwaysHit`.
    pub(crate) fn apply_hit<A: Agent>(
        fight: &mut Fight<A>,
        spell: SpellId,
        target: Side,
        damage: f64,
    ) {
        let result = fight.calc_damage_with(spell, target, damage, Outcome::AlwaysHit);
        fight.deal_damage(spell, result, false);
    }

    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
    }

    /// Go `AttachMultiplyAttackSpeed`.
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_attack_speed(self.attack_speed_multiplier);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_attack_speed(1.0 / self.attack_speed_multiplier);
    }
}
