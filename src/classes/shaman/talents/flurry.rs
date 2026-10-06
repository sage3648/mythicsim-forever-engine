//! Flurry (talent 16256), from Go sim/shaman/talents_enhancement.go `applyFlurry`: every melee
//! hit reaches the trigger, a spell flagged Proc only where the talent row allows it, and its
//! handler runs one spell batch window later.
//! A crit grants every charge of melee speed; otherwise a white hit spends one, at most once
//! per charge cooldown.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Flurry {
    pub(crate) trigger: AuraRef,
    pub(crate) aura: AuraRef,
    speed: f64,
    charge_icd: i64,
    can_proc_from_procs: bool,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    trigger: &str,
    aura: &str,
    speed: f64,
    charge_icd: i64,
    can_proc_from_procs: bool,
) -> Result<Flurry, String> {
    Ok(Flurry {
        trigger: fight.player_aura(trigger)?,
        aura: fight.player_aura(aura)?,
        speed,
        charge_icd,
        can_proc_from_procs,
    })
}

impl Flurry {
    /// The aura's `AttachMultiplyMeleeSpeed`.
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_melee_speed(self.speed);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_melee_speed(1.0 / self.speed);
    }

    /// The trigger: any melee hit schedules the handler.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let state = &fight.spells[spell];
        if state.melee_proc && (self.can_proc_from_procs || !state.flags.proc) {
            fight.schedule_delayed_proc(self.trigger, spell, *result);
        }
    }

    /// The delayed handler. `white` is whether the spell is a melee auto attack; `icd` is the
    /// charge cooldown's ready time, which the handler may move.
    pub(crate) fn handle<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        result: &SpellResult,
        white: bool,
        icd: &mut i64,
    ) {
        if result.crit() {
            fight.activate_aura(self.aura);
            let max = fight.aura(self.aura).max_stacks;
            fight.set_stacks(self.aura, max);
            return;
        }
        if fight.aura(self.aura).active && white && *icd <= fight.now {
            *icd = fight.now + self.charge_icd;
            fight.remove_stack(self.aura);
        }
    }
}
