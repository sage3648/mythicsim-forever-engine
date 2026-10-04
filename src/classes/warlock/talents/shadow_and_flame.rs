//! Shadow and Flame (426316), from Go sim/warlock/talents_destruction.go
//! `applyShadowAndFlame`: a landed Conflagrate raises the warlock's Shadow damage dealt and a
//! landed Shadowburn its Fire damage dealt, each for 20 seconds (1293816 and 426311), through
//! `AttachMultiplicativePseudoStatBuff`. Its chance to spare Immolate is in Conflagrate.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

/// Go `stats.SchoolIndexShadow` and `SchoolIndexFire`.
const SHADOW_INDEX: usize = 7;
const FIRE_INDEX: usize = 3;

#[derive(Clone, Debug)]
pub(crate) struct ShadowAndFlame {
    pub(crate) shadow_aura: AuraRef,
    pub(crate) fire_aura: AuraRef,
    multiplier: f64,
    trigger_spells: Vec<SpellId>,
    shadow_spells: Vec<SpellId>,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    shadow_aura: &str,
    fire_aura: &str,
    multiplier: f64,
    trigger_spells: &[usize],
    shadow_spells: &[usize],
) -> Result<ShadowAndFlame, String> {
    Ok(ShadowAndFlame {
        shadow_aura: fight.player_aura(shadow_aura)?,
        fire_aura: fight.player_aura(fire_aura)?,
        multiplier,
        trigger_spells: trigger_spells.to_vec(),
        shadow_spells: shadow_spells.to_vec(),
    })
}

impl ShadowAndFlame {
    /// The trigger's OnSpellHitDealt: Go `AttachProcTriggerCallback` with the Conflagrate and
    /// Shadowburn class masks, `OutcomeLanded`, no proc roll and `TriggerImmediately`.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.trigger_spells.contains(&spell) || !result.landed() {
            return;
        }
        if self.shadow_spells.contains(&spell) {
            fight.activate_aura(self.shadow_aura);
        } else {
            fight.activate_aura(self.fire_aura);
        }
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>, aura: AuraRef) {
        fight.multiply_school_damage_dealt(self.school(aura), self.multiplier);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>, aura: AuraRef) {
        fight.divide_school_damage_dealt(self.school(aura), self.multiplier);
    }

    fn school(&self, aura: AuraRef) -> usize {
        if aura == self.shadow_aura {
            SHADOW_INDEX
        } else {
            FIRE_INDEX
        }
    }
}
