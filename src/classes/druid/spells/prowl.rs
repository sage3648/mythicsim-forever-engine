//! Prowl (9913), from Go sim/druid/prowl.go: a prepull cast that enters Cat Form if needed and
//! activates its aura. The aura slows movement and lets the rotation act before each main
//! hand swing, as a replaced swing does; the druid's next hit ends it, and its expiry starts
//! the cooldown again.

use crate::core::fight::{AuraRef, Fight, SpellId, TimerId};

use super::super::{agent::DruidAgent, forms};

#[derive(Clone, Debug)]
pub(crate) struct Prowl {
    pub(crate) aura: AuraRef,
    movement_speed_multiplier: f64,
    icd: (TimerId, i64),
}

pub(crate) fn bind(
    fight: &mut Fight<DruidAgent>,
    spell: SpellId,
    aura: &str,
    movement_speed_multiplier: f64,
) -> Result<Prowl, String> {
    let aura = fight.player_aura(aura)?;
    let icd = fight.spells[spell].cd.ok_or("Prowl has no cooldown")?;
    Ok(Prowl {
        aura,
        movement_speed_multiplier,
        icd,
    })
}

impl Prowl {
    /// `ExtraCastCondition`: only before the pull.
    pub(crate) fn can_cast(&self, fight: &Fight<DruidAgent>) -> bool {
        fight.now < 0
    }

    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>) {
        if fight.agent.form & forms::CAT == 0 {
            let cat = fight.agent.cat_form.clone().expect("Prowl needs Cat Form");
            fight.activate_aura(cat.aura);
        }
        fight.activate_aura(self.aura);
    }

    pub(crate) fn on_gain(&self, fight: &mut Fight<DruidAgent>) {
        forms::multiply_movement_speed(fight, self.movement_speed_multiplier);
        fight.autos.react_before_mh_swing = true;
    }

    /// `OnSpellHitDealt`: any hit the druid deals ends it.
    pub(crate) fn on_spell_hit_dealt(&self, fight: &mut Fight<DruidAgent>) {
        fight.deactivate_aura(self.aura);
    }

    pub(crate) fn on_expire(&self, fight: &mut Fight<DruidAgent>) {
        let (timer, duration) = self.icd;
        fight.timers[timer] = fight.now + duration;
        forms::multiply_movement_speed(fight, 1.0 / self.movement_speed_multiplier);
        fight.autos.react_before_mh_swing = false;
    }
}
