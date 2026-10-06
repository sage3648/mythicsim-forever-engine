//! Curse of Recklessness (11717), from Go sim/warlock/curse_of_recklessness.go and core's
//! debuff in buffs/debuffs_auto_gen.go. The cast rolls to hit without a hit counter and, when
//! it lands, takes the curse slot and activates the debuff on the target. The debuff lowers
//! the target's armor through the per-stat exclusive Minor Armor Reduction category, beside
//! the raid's permanent members such as Faerie Fire, so its gain changes the target's armor
//! by the net amount Go measures and its expiry restores it. Its attack power on the target
//! matters only to a target that swings, which the gate refuses.

use crate::{
    classes::warlock::agent::WarlockAgent,
    core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId},
};

use super::take_curse_slot;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CurseOfRecklessness {
    pub(crate) aura: AuraRef,
    armor_delta: f64,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    aura: &str,
    armor_delta: f64,
) -> Result<CurseOfRecklessness, String> {
    let index = fight.trackers[Side::Target.index()]
        .find(aura)
        .ok_or_else(|| format!("target aura {aura} is not registered"))?;
    Ok(CurseOfRecklessness {
        aura: AuraRef {
            side: Side::Target,
            index,
        },
        armor_delta,
    })
}

impl CurseOfRecklessness {
    /// `ApplyEffects`.
    pub(crate) fn apply(&self, fight: &mut Fight<WarlockAgent>, spell: SpellId, target: Side) {
        let result = fight.calc_outcome(spell, target, Outcome::MagicHitNoHitCounter);
        if result.landed() {
            let aura = fight.aura_on(self.aura, target);
            take_curse_slot(fight, aura);
            fight.activate_aura(aura);
        }
        fight.deal_damage(spell, result, false);
    }

    /// The debuff's gain on its target, the side of the aura.
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>, target: Side) {
        fight.add_target_armor(target, self.armor_delta);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>, target: Side) {
        fight.add_target_armor(target, -self.armor_delta);
    }
}
