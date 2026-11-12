//! Lightning Shield's orb under Forever's rules. The client's proc (every rank: ProcFlags
//! 139944, 100%, 3 charges, 3.5 sec internal cooldown) hears the melee hits the shaman takes: a
//! landed one spends a charge and the orb strikes the attacker. The Go reference's trigger hears
//! only the shield self proc, so its orbs never fire. Only a target that tanks the shaman ever
//! strikes it.

use crate::core::fight::{Agent, AuraRef, Fight, Side, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct LightningShieldOrb {
    pub(crate) aura: AuraRef,
    pub(crate) spell: SpellId,
    pub(crate) base_damage: f64,
    pub(crate) can_crit: bool,
    /// The internal cooldown between two orbs.
    pub(crate) icd: i64,
}

impl LightningShieldOrb {
    /// The aura's OnSpellHitTaken for a target's swing: a landed hit spends a charge once the
    /// internal cooldown is up, and the orb strikes the attacker. Returns when the next orb can
    /// go, if this one did.
    pub(crate) fn on_hit_taken<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        result: &SpellResult,
        ready_at: i64,
    ) -> Option<i64> {
        if !result.landed() || fight.now < ready_at || fight.aura(self.aura).stacks == 0 {
            return None;
        }
        let next = fight.now + self.icd;
        fight.remove_stack(self.aura);
        fight.cast(self.spell, result.attacker);
        Some(next)
    }

    /// The orb's ApplyEffects: its damage, a magic hit that crits only if the row allows it.
    pub(crate) fn hit<A: Agent>(&self, fight: &mut Fight<A>, target: Side) {
        let result = if self.can_crit {
            fight.calc_damage(self.spell, target, self.base_damage)
        } else {
            fight.calc_damage_hit_only(self.spell, target, self.base_damage)
        };
        fight.deal_damage(self.spell, result, false);
    }
}
