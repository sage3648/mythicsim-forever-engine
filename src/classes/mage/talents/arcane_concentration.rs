//! Arcane Concentration (talent 11213) and Clearcasting (12536), from Go
//! sim/mage/talents_arcane.go `registerArcaneConcentration`. A landed damaging Mage
//! spell can make the next costed Mage cast free, at most once per internal cooldown.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

/// Go `MageSpellsAllDamaging`, shared with Fingers of Frost.
pub(crate) use super::fingers_of_frost::DAMAGING;

#[derive(Clone, Debug)]
pub(crate) struct ArcaneConcentration {
    pub(crate) clearcasting: AuraRef,
    pub(crate) trigger: AuraRef,
    proc_chance: f64,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    clearcasting: &str,
    trigger: &str,
    proc_chance: f64,
) -> Result<ArcaneConcentration, String> {
    Ok(ArcaneConcentration {
        clearcasting: fight.player_aura(clearcasting)?,
        trigger: fight.player_aura(trigger)?,
        proc_chance,
    })
}

impl ArcaneConcentration {
    /// Clearcasting OnGain and OnExpire: Go's integer spell cost percentage.
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.player.spell_cost_percent_modifier -= 100;
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.player.spell_cost_percent_modifier += 100;
    }

    /// Clearcasting OnCastComplete. A Clearcasting gained in this timestep belongs to the
    /// next cast; any other costed Mage cast consumes it.
    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        let aura = fight.aura(self.clearcasting);
        if aura.remaining(fight.now) == aura.duration {
            return;
        }
        let state = &fight.spells[spell];
        if state.class_spell.is_none() || state.cost.is_none_or(|cost| cost.base == 0) {
            return;
        }
        fight.deactivate_aura(self.clearcasting);
    }

    /// The "Arcane Concentration" proc trigger, with its internal cooldown.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let state = &fight.spells[spell];
        let damaging = state
            .class_spell
            .as_deref()
            .is_some_and(|class| DAMAGING.contains(&class));
        if state.flags.proc || !damaging || !result.landed() {
            return;
        }
        let icd = fight.aura(self.trigger).icd;
        if let Some((timer, _)) = icd {
            if fight.timers[timer] > fight.now {
                return;
            }
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return;
        }
        if let Some((timer, duration)) = icd {
            fight.timers[timer] = fight.now + duration;
        }
        fight.activate_aura(self.clearcasting);
    }
}
