//! Stealth and Vanish, from Go sim/rogue/stealth.go, vanish.go and rogue.go `BreakStealth`.
//! Stealth is cast before the pull; Vanish stops the auto attacks and stealths again. Every
//! Rogue strike breaks Stealth first, which starts the auto attacks again.

use crate::core::fight::{Agent, AuraRef, Fight};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Stealth {
    pub(crate) aura: AuraRef,
}

impl Stealth {
    /// Go `IsStealthed`.
    pub(crate) fn active<A: Agent>(&self, fight: &Fight<A>) -> bool {
        fight.aura(self.aura).active
    }

    /// Go `BreakStealth`: leave Stealth and resume the auto attacks.
    pub(crate) fn break_stealth<A: Agent>(&self, fight: &mut Fight<A>) {
        if self.active(fight) {
            fight.deactivate_aura(self.aura);
            fight.enable_melee_swing();
        }
    }

    /// The Stealth spell's `ExtraCastCondition`: only before the pull.
    pub(crate) fn can_cast<A: Agent>(fight: &Fight<A>) -> bool {
        fight.now < 0
    }

    /// The Stealth spell's effect.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
    }

    /// Vanish's effect: the auto attacks stop, then Stealth.
    pub(crate) fn vanish<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.cancel_melee_swing();
        fight.activate_aura(self.aura);
    }
}
