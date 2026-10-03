//! Omen of Clarity (16864) and Clearcasting (16870), from Go sim/druid/omen_of_clarity.go, a
//! baseline Balance passive in Forever that Go wires in with the talents. A landed hit from
//! a listened spell can grant Clearcasting at two procs a minute of its cast time, or of the
//! default GCD for instants; Moonkin Form doubles the chance and halves the cooldown.
//! Clearcasting makes the next costed spell in its mask free.

use crate::core::fight::{
    Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult, TimerId, OUTCOME_LANDED,
};

#[derive(Clone, Debug)]
pub(crate) struct OmenOfClarity {
    pub(crate) aura: AuraRef,
    trigger: AuraRef,
    trigger_spells: Vec<bool>,
    cost_spells: Vec<bool>,
    icd_timer: TimerId,
    icd: i64,
    ppm: f64,
    gcd: i64,
    moonkin: Option<AuraRef>,
    moonkin_chance: f64,
    moonkin_cooldown: f64,
    trigger_immediately: bool,
    proc_chance: f64,
    cost_mod: ModId,
}

/// The exported parameters.
pub(crate) struct Params<'a> {
    pub(crate) aura: &'a str,
    pub(crate) trigger: &'a str,
    pub(crate) trigger_spells: &'a [usize],
    pub(crate) cost_spells: &'a [usize],
    pub(crate) icd: i64,
    pub(crate) ppm: f64,
    pub(crate) gcd: i64,
    pub(crate) moonkin: Option<&'a str>,
    pub(crate) moonkin_chance: f64,
    pub(crate) moonkin_cooldown: f64,
    pub(crate) trigger_immediately: bool,
    pub(crate) proc_chance: f64,
    pub(crate) cost_percent_add: f64,
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

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    params: Params,
) -> Result<OmenOfClarity, String> {
    let aura = fight.player_aura(params.aura)?;
    let trigger = fight.player_aura(params.trigger)?;
    let (icd_timer, _) = fight
        .aura(trigger)
        .icd
        .ok_or_else(|| format!("{} has no cooldown", params.trigger))?;
    let moonkin = params
        .moonkin
        .map(|label| fight.player_aura(label))
        .transpose()?;
    let len = fight.spells.len();
    // Go AttachSpellMod: only spells with a cost carry the modifier.
    let costed: Vec<SpellId> = params
        .cost_spells
        .iter()
        .copied()
        .filter(|&spell| spell < len && fight.spells[spell].cost.is_some())
        .collect();
    let cost_mod = fight.register_mod(
        ModKind::PowerCostPercentAdd,
        params.cost_percent_add,
        0,
        costed,
    );
    Ok(OmenOfClarity {
        aura,
        trigger,
        trigger_spells: mask(len, params.trigger_spells),
        cost_spells: mask(len, params.cost_spells),
        icd_timer,
        icd: params.icd,
        ppm: params.ppm,
        gcd: params.gcd,
        moonkin,
        moonkin_chance: params.moonkin_chance,
        moonkin_cooldown: params.moonkin_cooldown,
        trigger_immediately: params.trigger_immediately,
        proc_chance: params.proc_chance,
        cost_mod,
    })
}

impl OmenOfClarity {
    /// Go `AttachProcTriggerCallback`'s OnSpellHitDealt with Omen's extra condition.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.trigger_spells[spell] || result.outcome & OUTCOME_LANDED == 0 {
            return;
        }
        if fight.timers[self.icd_timer] > fight.now {
            return;
        }
        // The condition: every listened spell is a spell, so its cast time sets the chance.
        let cast_time = fight.spells[spell].default_cast.cast_time;
        let seconds = crate::core::time::seconds(if cast_time > 0 { cast_time } else { self.gcd });
        let mut chance = self.ppm * seconds / 60.0;
        let mut icd = self.icd;
        if self
            .moonkin
            .is_some_and(|moonkin| fight.aura(moonkin).active)
        {
            chance *= self.moonkin_chance;
            icd = (self.icd as f64 * self.moonkin_cooldown) as i64;
        }
        if !fight.proc(chance, "Omen of Clarity") {
            return;
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return;
        }
        if icd != 0 {
            fight.timers[self.icd_timer] = fight.now + icd;
        }
        if self.trigger_immediately {
            fight.activate_aura(self.aura);
        } else {
            fight.schedule_delayed_proc(self.trigger, spell, *result);
        }
    }

    /// The delayed handler: Clearcasting.
    pub(crate) fn on_delayed_proc<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cost_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cost_mod);
    }

    /// Clearcasting's OnCastComplete: a costed spell in its mask spends it.
    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        let costed = fight.spells[spell].cost.is_some_and(|cost| cost.base > 0);
        if self.cost_spells[spell] && costed {
            fight.deactivate_aura(self.aura);
        }
    }
}
