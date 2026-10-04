//! Rogue talent proc triggers, from Go sim/rogue/talents_assassination.go and
//! talents_subtlety.go through core `AttachProcTriggerCallback`: Seal Fate's and Initiative's
//! combo point, Cutthroat's aura and Thousand Cuts' stack. A trigger hears its spells' hits, or
//! their ticks for Thousand Cuts, checks the outcome and its cooldown, rolls its chance under
//! its own label and runs the handler a spell batch window later.

use crate::core::fight::{
    Agent, AuraRef, Fight, SpellId, SpellResult, OUTCOME_CRIT, OUTCOME_LANDED,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Handler {
    /// A combo point under its own metrics.
    ComboPoint(usize),
    /// Activate an aura.
    Activate(AuraRef),
    /// Activate an aura and add a stack.
    Stack(AuraRef),
    /// Go `AutoAttacks.ExtraMHAttack`: the main hand swings now.
    ExtraAttack,
}

#[derive(Clone, Debug)]
pub(crate) struct Proc {
    pub(crate) handler: Handler,
    pub(crate) chance: f64,
    /// The outcome the result needs, or none.
    pub(crate) outcome: u16,
    pub(crate) periodic: bool,
    pub(crate) delay: i64,
    /// Whether each spell, by spellbook position, is one the trigger hears.
    pub(crate) spells: Vec<bool>,
}

impl Proc {
    /// Go's proc trigger callback.
    pub(crate) fn callback<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        aura: AuraRef,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.spells[spell] {
            return;
        }
        if self.outcome != 0 && result.outcome & self.outcome == 0 {
            return;
        }
        let icd = fight.aura(aura).icd;
        if let Some((timer, _)) = icd {
            if fight.timers[timer] > fight.now {
                return;
            }
        }
        if self.chance != 1.0 && fight.random_for_aura(aura) > self.chance {
            return;
        }
        if let Some((timer, duration)) = icd {
            fight.timers[timer] = fight.now + duration;
        }
        // A trigger without a delay is Go's TriggerImmediately.
        if self.delay == 0 {
            self.handle(fight);
            return;
        }
        let result = *result;
        fight.schedule(
            fight.now + self.delay,
            crate::core::fight::PRIORITY_DOT,
            crate::core::fight::Action::DelayedProc {
                aura,
                spell,
                result,
            },
        );
    }

    /// The handler, a spell batch window after the roll.
    pub(crate) fn handle<A: Agent>(&self, fight: &mut Fight<A>) {
        match self.handler {
            Handler::ComboPoint(metrics) => fight.add_combo_points(1, metrics),
            Handler::Activate(aura) => fight.activate_aura(aura),
            Handler::Stack(aura) => {
                fight.activate_aura(aura);
                fight.add_stack(aura);
            }
            Handler::ExtraAttack => fight.extra_mh_attacks(1),
        }
    }
}

/// The outcome mask a trigger names.
pub(crate) fn outcome_mask(outcome: &str) -> Result<u16, String> {
    match outcome {
        "crit" => Ok(OUTCOME_CRIT),
        "landed" => Ok(OUTCOME_LANDED),
        "any" => Ok(0),
        other => Err(format!("unknown proc outcome {other}")),
    }
}
