//! Natural Reaction (417051), from Go sim/druid/talents_feral_combat.go
//! `applyNaturalReaction`: a dodge in Bear Form can grant Rage one spell batch window later.
//! Its dodge rating is static.

use crate::{
    contracts::prepared_v2::ActionId,
    core::fight::{AuraRef, Fight, SpellResult},
};

use super::super::{agent::DruidAgent, forms};

/// Go `OutcomeDodge`.
const OUTCOME_DODGE: u16 = 1 << 6;

#[derive(Clone, Copy, Debug)]
pub(crate) struct NaturalReaction {
    trigger: AuraRef,
    proc_chance: f64,
    trigger_immediately: bool,
    rage: f64,
    metrics: usize,
}

pub(crate) fn bind(
    fight: &mut Fight<DruidAgent>,
    trigger: &str,
    proc_chance: f64,
    trigger_immediately: bool,
    rage: f64,
    metrics_action_id: &ActionId,
) -> Result<NaturalReaction, String> {
    let trigger = fight.player_aura(trigger)?;
    let metrics = fight.new_rage_metrics(metrics_action_id.clone());
    Ok(NaturalReaction {
        trigger,
        proc_chance,
        trigger_immediately,
        rage,
        metrics,
    })
}

impl NaturalReaction {
    /// `OnSpellHitTaken` for the target's swing: a dodge, in Bear Form, at the chance.
    pub(crate) fn on_hit_taken(&self, fight: &mut Fight<DruidAgent>, result: &SpellResult) {
        if result.outcome & OUTCOME_DODGE == 0 || fight.agent.form & forms::BEAR == 0 {
            return;
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return;
        }
        if self.trigger_immediately {
            self.on_delayed_proc(fight);
        } else {
            // The proc's spell is the target's swing, which no listener reads.
            fight.schedule_delayed_proc(self.trigger, 0, *result);
        }
    }

    /// The handler: Rage.
    pub(crate) fn on_delayed_proc(&self, fight: &mut Fight<DruidAgent>) {
        fight.add_rage(self.rage, self.metrics);
    }
}
