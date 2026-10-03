//! Blood Frenzy (16958), from Go sim/druid/talents_feral_combat.go `applyBloodFrenzy`: a cat
//! builder crit grants a combo point one spell batch window later. Its bear half needs Bear
//! Form, which the supported builds never enter.

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
}

pub(crate) fn bind(
    fight: &mut Fight<DruidAgent>,
    trigger: &str,
    proc_chance: f64,
    trigger_spells: &[usize],
    trigger_immediately: bool,
    metrics_action_id: &ActionId,
) -> Result<BloodFrenzy, String> {
    let trigger = fight.player_aura(trigger)?;
    let mut spells = vec![false; fight.spells.len()];
    for &spell in trigger_spells {
        if let Some(slot) = spells.get_mut(spell) {
            *slot = true;
        }
    }
    let combo_point_metrics =
        fight.new_resource_metrics(metrics_action_id.clone(), ResourceKind::ComboPoints);
    Ok(BloodFrenzy {
        trigger,
        proc_chance,
        trigger_spells: spells,
        trigger_immediately,
        combo_point_metrics,
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
}
