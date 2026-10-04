//! Blood Frenzy (16958), from Go sim/druid/talents_feral_combat.go `applyBloodFrenzy`: a cat
//! builder crit grants a combo point, and a melee crit in Bear Form grants Rage, each one
//! spell batch window later. The two triggers roll their chance under their own names.

use crate::{
    contracts::prepared_v2::ActionId,
    core::fight::{AuraRef, Fight, ResourceKind, SpellId, SpellResult, OUTCOME_CRIT},
};

use super::super::{agent::DruidAgent, forms};

#[derive(Clone, Debug)]
pub(crate) struct BloodFrenzy {
    trigger: AuraRef,
    proc_chance: f64,
    trigger_spells: Vec<bool>,
    trigger_immediately: bool,
    combo_point_metrics: usize,
    bear: AuraRef,
    bear_spells: Vec<bool>,
    bear_rage: f64,
    rage_metrics: usize,
}

/// The exported parameters.
pub(crate) struct Params<'a> {
    pub(crate) trigger: &'a str,
    pub(crate) bear_trigger: &'a str,
    pub(crate) proc_chance: f64,
    pub(crate) trigger_spells: &'a [usize],
    pub(crate) trigger_immediately: bool,
    pub(crate) metrics_action_id: &'a ActionId,
    pub(crate) bear_trigger_spells: &'a [usize],
    pub(crate) bear_rage: f64,
}

fn mask(len: usize, spells: &[usize]) -> Vec<bool> {
    let mut mask = vec![false; len];
    for &spell in spells {
        if let Some(slot) = mask.get_mut(spell) {
            *slot = true;
        }
    }
    mask
}

/// Go registers the Rage metrics, then the combo point metrics.
pub(crate) fn bind(fight: &mut Fight<DruidAgent>, params: Params) -> Result<BloodFrenzy, String> {
    let trigger = fight.player_aura(params.trigger)?;
    let bear = fight.player_aura(params.bear_trigger)?;
    let len = fight.spells.len();
    let rage_metrics =
        fight.new_resource_metrics(params.metrics_action_id.clone(), ResourceKind::Rage);
    let combo_point_metrics =
        fight.new_resource_metrics(params.metrics_action_id.clone(), ResourceKind::ComboPoints);
    Ok(BloodFrenzy {
        trigger,
        proc_chance: params.proc_chance,
        trigger_spells: mask(len, params.trigger_spells),
        trigger_immediately: params.trigger_immediately,
        combo_point_metrics,
        bear,
        bear_spells: mask(len, params.bear_trigger_spells),
        bear_rage: params.bear_rage,
        rage_metrics,
    })
}

impl BloodFrenzy {
    /// Go `AttachProcTriggerCallback`'s OnSpellHitDealt: a crit, in Cat Form, at the chance.
    pub(crate) fn on_spell_hit_dealt(
        &self,
        fight: &mut Fight<DruidAgent>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.trigger_spells[spell] || result.outcome & OUTCOME_CRIT == 0 {
            return;
        }
        if fight.agent.form & forms::CAT == 0 {
            return;
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return;
        }
        if self.trigger_immediately {
            self.on_delayed_proc(fight);
        } else {
            fight.schedule_delayed_proc(self.trigger, spell, *result);
        }
    }

    /// The handler: a combo point.
    pub(crate) fn on_delayed_proc(&self, fight: &mut Fight<DruidAgent>) {
        fight.add_combo_points(1, self.combo_point_metrics);
    }

    /// The bear trigger: a melee crit, in Bear Form, at the chance.
    pub(crate) fn on_bear_hit_dealt(
        &self,
        fight: &mut Fight<DruidAgent>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.bear_spells[spell] || result.outcome & OUTCOME_CRIT == 0 {
            return;
        }
        if fight.agent.form & forms::BEAR == 0 {
            return;
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.bear) > self.proc_chance {
            return;
        }
        if self.trigger_immediately {
            self.on_bear_delayed_proc(fight);
        } else {
            fight.schedule_delayed_proc(self.bear, spell, *result);
        }
    }

    /// The bear handler: Rage.
    pub(crate) fn on_bear_delayed_proc(&self, fight: &mut Fight<DruidAgent>) {
        fight.add_rage(self.bear_rage, self.rage_metrics);
    }
}
