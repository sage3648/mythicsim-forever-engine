//! Judgement of the Crusader, from Go sim/paladin/seal_of_the_crusader.go and
//! sim/core/buffs/paladin.go `JudgementOfTheCrusaderAura`: the judgement lands without a roll
//! and puts up its rank's debuff, which raises the Holy spell damage the target takes. Every
//! rank and the raid's debuff share one single aura category whose bids are the bonuses, so
//! the core's exclusive category decides which one holds.

use crate::core::fight::{school_index, AuraRef, Fight, Outcome, Side, SpellId};

use super::super::agent::PaladinAgent;

/// A member of the "Judgement of the Crusader" category: the Holy spell damage it adds
/// while it holds, and whether it is the raid's debuff, whose bonus the prepared target
/// already carries from the reset.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CrusaderJudgement {
    pub(crate) bonus: f64,
    pub(crate) raid: bool,
}

/// Judgement of the Crusader's `ApplyEffects`: an outcome that always hits, then the rank's
/// debuff.
pub(crate) fn judge(fight: &mut Fight<PaladinAgent>, spell: SpellId, target: Side, aura: AuraRef) {
    let result = fight.calc_outcome(spell, target, Outcome::AlwaysHit);
    fight.deal_damage(spell, result, false);
    fight.activate_aura(aura);
}

/// The exclusive effect's gain and expiry: the member's Holy spell damage on the target. The
/// raid's debuff gains only at the reset, where the prepared target already holds its bonus.
pub(crate) fn toggle(fight: &mut Fight<PaladinAgent>, member: usize, active: bool) {
    let judgement = fight.agent.crusader_judgements[member];
    if judgement.raid && active {
        return;
    }
    let delta = if active {
        judgement.bonus
    } else {
        -judgement.bonus
    };
    fight.add_target_school_bonus_spell_damage(Side::Target, school_index(2), delta);
}
