//! Judgement Refresh, from Go sim/paladin/judgement.go: every melee strike that lands
//! refreshes the active judgement debuffs on its target.

use crate::core::fight::{Agent, AuraRef, Fight, Side, SpellId, SpellResult, OUTCOME_LANDED};

#[derive(Clone, Debug)]
pub(crate) struct JudgementRefresh {
    judgements: Vec<AuraRef>,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    judgements: &[String],
) -> Result<JudgementRefresh, String> {
    let judgements = judgements
        .iter()
        .map(|label| {
            fight.trackers[Side::Target.index()]
                .find(label)
                .map(|index| AuraRef {
                    side: Side::Target,
                    index,
                })
                .ok_or_else(|| format!("judgement aura {label} is not registered"))
        })
        .collect::<Result<_, _>>()?;
    Ok(JudgementRefresh { judgements })
}

impl JudgementRefresh {
    /// The judgement auras, in Go's order.
    pub(crate) fn judgements(&self) -> &[AuraRef] {
        &self.judgements
    }

    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let state = &fight.spells[spell];
        if state.flags.proc || !state.melee_proc || result.outcome & OUTCOME_LANDED == 0 {
            return;
        }
        for &aura in &self.judgements {
            if fight.aura(aura).active {
                fight.refresh_aura(aura);
            }
        }
    }
}
