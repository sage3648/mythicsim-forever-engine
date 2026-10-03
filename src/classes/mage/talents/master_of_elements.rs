//! Master of Elements (29074), from Go sim/mage/talents_fire.go `registerMasterOfElements`:
//! a Fire or Frost Mage spell's crit refunds a share of its base cost, at most once per
//! internal cooldown, and only when the cast cost mana.

use crate::{
    classes::mage::masks::{is_class, ALL},
    contracts::prepared_v2::ActionId,
    core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult},
};

/// Go `SpellSchoolFire | SpellSchoolFrost`.
const FIRE_OR_FROST: u8 = 4 | 16;

#[derive(Clone, Debug)]
pub(crate) struct MasterOfElements {
    trigger: AuraRef,
    refund: f64,
    metrics: usize,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    trigger: &str,
    refund: f64,
    metrics_action_id: &ActionId,
) -> Result<MasterOfElements, String> {
    let trigger = fight.player_aura(trigger)?;
    let metrics = fight.new_mana_metrics(metrics_action_id.clone());
    Ok(MasterOfElements {
        trigger,
        refund,
        metrics,
    })
}

impl MasterOfElements {
    /// The trigger's OnSpellHitDealt: Go `AttachProcTriggerCallback` with no proc roll.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let state = &fight.spells[spell];
        if state.flags.proc || !is_class(state.class_spell.as_deref(), ALL) || !result.crit() {
            return;
        }
        let icd = fight.aura(self.trigger).icd;
        if let Some((timer, _)) = icd {
            if fight.timers[timer] > fight.now {
                return;
            }
        }
        let Some(cost) = state.cost else {
            return;
        };
        if state.school & FIRE_OR_FROST == 0 || state.cur_cast.cost <= 0.0 {
            return;
        }
        if let Some((timer, duration)) = icd {
            fight.timers[timer] = fight.now + duration;
        }
        fight.add_mana(f64::from(cost.base) * self.refund, self.metrics);
    }
}
