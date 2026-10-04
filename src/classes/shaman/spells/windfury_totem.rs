//! The shaman's own Windfury Totem, from Go sim/shaman/totems.go `registerWindfuryTotemSpell`:
//! the totem's aura refreshes a tracking aura and a dummy aura every period, ticking at once;
//! the dummy's exclusive effect turns the trigger on, which grants charges of attack power and
//! an extra main hand attack, and landed autos spend the charges. Both rows ask for a hit that
//! lands and deals damage.

use crate::{
    contracts::prepared_v2::Effect,
    core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult},
};

#[derive(Clone, Debug)]
pub(crate) struct WindfuryTotem {
    pub(crate) totem: AuraRef,
    tracking: AuraRef,
    dummy: AuraRef,
    trigger: AuraRef,
    proc_aura: AuraRef,
    bit: u32,
    extra: SpellId,
    triggers: Vec<bool>,
    spend: Vec<bool>,
    white: Vec<bool>,
    chance: f64,
    pub(crate) period: i64,
    pub(crate) duration: i64,
    gain_log: String,
    expire_log: String,
}

/// Spellbook positions as a mask.
fn mask(count: usize, spells: &[usize]) -> Result<Vec<bool>, String> {
    let mut mask = vec![false; count];
    for &spell in spells {
        *mask
            .get_mut(spell)
            .ok_or_else(|| format!("Windfury Totem names spell {spell}"))? = true;
    }
    Ok(mask)
}

pub(crate) fn bind<A: Agent>(fight: &Fight<A>, effect: &Effect) -> Result<WindfuryTotem, String> {
    let Effect::WindfuryTotemSelf {
        totem_aura,
        duration_ns,
        period_ns,
        tracking_aura,
        dummy_aura,
        trigger_aura,
        trigger_spells,
        trigger_proc_chance,
        proc_aura,
        spend_spells,
        extra_spell,
        white_spells,
        proc_gain_log,
        proc_expire_log,
        ..
    } = effect
    else {
        return Err("not a Windfury Totem effect".into());
    };
    let count = fight.spells.len();
    let bit = fight
        .stat_aura_labels
        .iter()
        .position(|label| label == proc_aura)
        .map(|bit| 1u32 << bit)
        .ok_or_else(|| format!("{proc_aura} is not a stat aura"))?;
    if *extra_spell >= count {
        return Err(format!("Windfury Totem names spell {extra_spell}"));
    }
    Ok(WindfuryTotem {
        totem: fight.player_aura(totem_aura)?,
        tracking: fight.player_aura(tracking_aura)?,
        dummy: fight.player_aura(dummy_aura)?,
        trigger: fight.player_aura(trigger_aura)?,
        proc_aura: fight.player_aura(proc_aura)?,
        bit,
        extra: *extra_spell,
        triggers: mask(count, trigger_spells)?,
        spend: mask(count, spend_spells)?,
        white: mask(count, white_spells)?,
        chance: *trigger_proc_chance,
        period: *period_ns,
        duration: *duration_ns,
        gain_log: proc_gain_log.clone(),
        expire_log: proc_expire_log.clone(),
    })
}

impl WindfuryTotem {
    /// The periodic action's tick: refresh the tracking aura and the dummy aura, whose
    /// exclusive effect turns the trigger on.
    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.tracking);
        fight.activate_aura(self.dummy);
    }

    /// The totem aura's expiry: the dummy aura and the tracking aura go.
    pub(crate) fn on_totem_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_aura(self.dummy);
        fight.deactivate_aura(self.tracking);
    }

    /// The dummy aura's exclusive effect turns the trigger on as it activates.
    pub(crate) fn on_dummy_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.trigger);
    }

    /// The dummy aura's exclusive effect turns the trigger off as it fades.
    pub(crate) fn on_dummy_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_aura(self.trigger);
    }

    /// The trigger: landed hits it hears, its cooldown and chance, then the charges and the
    /// extra attack, which a main hand swing replacement may replace.
    pub(crate) fn on_trigger_hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.triggers[spell] || !result.landed() || result.damage == 0.0 {
            return;
        }
        let icd = fight.aura(self.trigger).icd;
        if let Some((timer, _)) = icd {
            if fight.timers[timer] > fight.now {
                return;
            }
        }
        if self.chance != 1.0 && fight.random_for_aura(self.trigger) > self.chance {
            return;
        }
        if let Some((timer, duration)) = icd {
            fight.timers[timer] = fight.now + duration;
        }
        fight.activate_aura(self.proc_aura);
        let mut charges = fight.aura(self.proc_aura).max_stacks;
        if self.white[spell] {
            charges -= 1;
        }
        fight.set_stacks(self.proc_aura, charges);
        let mut extra = self.extra;
        if fight.config.melee.replace_main_hand_swing {
            extra = A::replace_mh_swing(fight, extra);
        }
        fight.cast(extra, result.target);
    }

    /// The charges' spender on the proc aura, at once.
    pub(crate) fn on_spend_hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if self.spend[spell] && result.landed() && result.damage != 0.0 {
            fight.remove_stack(self.proc_aura);
        }
    }

    /// The proc aura's gain: the stats aura's line, then its stats.
    pub(crate) fn on_proc_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        if fight.log.is_some() {
            let line = self.gain_log.clone();
            fight.player_log(&line);
        }
        fight.set_stat_aura(self.bit, true);
    }

    pub(crate) fn on_proc_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        if fight.log.is_some() {
            let line = self.expire_log.clone();
            fight.player_log(&line);
        }
        fight.set_stat_aura(self.bit, false);
    }
}
