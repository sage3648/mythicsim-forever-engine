//! Resourcefulness (440529) and Expose Prey (1310532), from Go sim/hunter/talents_survival.go,
//! and Rapid Recuperation (1223987) from talents_marksmanship.go, which works the same way.
//! Both are proc triggers whose handlers wait a spell batch window: a crit grants
//! Resourcefulness's casting mana regeneration, which Go adds to the pseudo stat without
//! recomputing the regeneration rates; a melee or ranged hit on a target with Hunter's Mark
//! opens the Mongoose Bite window once the handler finds it landed.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult, OUTCOME_CRIT};

/// A bound Survival proc trigger.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SurvivalProc {
    pub(crate) trigger: AuraRef,
    pub(crate) aura: AuraRef,
    chance: f64,
}

impl SurvivalProc {
    pub(crate) fn bind<A: Agent>(
        fight: &Fight<A>,
        trigger: &str,
        aura: &str,
        chance: f64,
    ) -> Result<Self, String> {
        Ok(SurvivalProc {
            trigger: fight.player_aura(trigger)?,
            aura: fight.player_aura(aura)?,
            chance,
        })
    }

    /// The chance roll, then the handler a spell batch window later.
    fn roll<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, result: &SpellResult) {
        if self.chance != 1.0 && fight.random_for_aura(self.trigger) > self.chance {
            return;
        }
        fight.schedule_delayed_proc(self.trigger, spell, *result);
    }

    /// Resourcefulness Trigger's `OnSpellHitDealt`: any spell's crit.
    pub(crate) fn resourcefulness_hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if fight.spells[spell].flags.proc || result.outcome & OUTCOME_CRIT == 0 {
            return;
        }
        self.roll(fight, spell, result);
    }

    /// Expose Prey's `OnSpellHitDealt`: melee and ranged hits of any outcome.
    pub(crate) fn expose_prey_hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if fight.spells[spell].flags.proc || !fight.spells[spell].melee_or_ranged_proc {
            return;
        }
        self.roll(fight, spell, result);
    }

    /// Rapid Recuperation Trigger's `OnSpellHitDealt`: Serpent Sting's hit of any outcome, whose
    /// handler checks it landed.
    pub(crate) fn rapid_recuperation_hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
        sting: bool,
    ) {
        if fight.spells[spell].flags.proc || !sting {
            return;
        }
        self.roll(fight, spell, result);
    }

    /// Expose Prey's handler: a landed hit on a marked target opens the window.
    pub(crate) fn expose_prey_proc<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        result: &SpellResult,
        marked: bool,
    ) {
        if result.landed() && marked {
            fight.activate_aura(self.aura);
        }
    }

    /// Resourcefulness's gain and expiry on the casting regeneration rate.
    pub(crate) fn resourcefulness_changed<A: Agent>(
        fight: &mut Fight<A>,
        regen: f64,
        gained: bool,
    ) {
        if gained {
            fight.player.spirit_regen_rate_casting += regen;
        } else {
            fight.player.spirit_regen_rate_casting -= regen;
        }
    }
}
