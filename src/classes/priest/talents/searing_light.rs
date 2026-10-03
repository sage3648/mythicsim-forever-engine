//! Searing Light (14909, aura 1284536), from Go sim/priest/talents_holy.go
//! `applySearingLight`: Holy Fire ticks may grant Holy Purpose, which makes the next Holy Nova
//! free and ends when Holy Nova is cast. Its Holy damage bonus is static and arrives prepared.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult};

#[derive(Clone, Debug)]
pub(crate) struct SearingLight {
    pub(crate) aura: AuraRef,
    trigger: AuraRef,
    proc_chance: f64,
    immediate: bool,
    trigger_spells: Vec<bool>,
    cost_mod: ModId,
    cancels: Vec<bool>,
}

fn mask(spells: &[usize], len: usize) -> Vec<bool> {
    let mut mask = vec![false; len];
    for &spell in spells {
        if spell < len {
            mask[spell] = true;
        }
    }
    mask
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    trigger_aura: &str,
    proc_chance: f64,
    immediate: bool,
    trigger_spells: &[usize],
    cost_percent_add: f64,
    cost_spells: &[usize],
    cancel_spells: &[usize],
) -> Result<SearingLight, String> {
    let aura = fight.player_aura(aura)?;
    let trigger = fight.player_aura(trigger_aura)?;
    let len = fight.spells.len();
    let affected = cost_spells.iter().copied().filter(|&s| s < len).collect();
    let cost_mod = fight.register_mod(ModKind::PowerCostPercentAdd, cost_percent_add, 0, affected);
    Ok(SearingLight {
        aura,
        trigger,
        proc_chance,
        immediate,
        trigger_spells: mask(trigger_spells, len),
        cost_mod,
        cancels: mask(cancel_spells, len),
    })
}

impl SearingLight {
    /// The proc trigger on periodic damage dealt: a Holy Fire tick, any outcome.
    pub(crate) fn on_periodic_damage_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.trigger_spells[spell] {
            return;
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return;
        }
        if self.immediate {
            fight.activate_aura(self.aura);
        } else {
            fight.schedule_delayed_proc(self.trigger, spell, *result);
        }
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cost_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cost_mod);
    }

    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        if self.cancels[spell] {
            fight.deactivate_aura(self.aura);
        }
    }
}
