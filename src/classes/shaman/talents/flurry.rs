//! Flurry (talent 16256), from Go sim/shaman/talents_enhancement.go `applyFlurry`: every melee
//! hit reaches the trigger, a spell flagged Proc only where the talent row allows it, and its
//! handler runs one spell batch window later.
//! A crit grants every charge of melee speed; otherwise a white hit spends one, at most once
//! per charge cooldown.
//!
//! Under Forever's rules a Windfury Totem strike, the party's or the shaman's own, spends no
//! charge and leaves the charge cooldown alone, since the hotfixes of 17 November 2026: "Fixed
//! an issue where Flurry incorrectly lost stacks when Windfury totem attacks landed." Its crit
//! still starts Flurry, as any melee crit does: the post speaks of lost stacks only, and a
//! level 60 Fury warrior's combat log of 16 November 2026 shows the same strike's crit
//! starting the Warrior's Flurry 22 of 22 times. Without the
//! rule the strike, a copy of the main hand auto, spends a charge as a white swing does.
//!
//! Under Forever's rules a Windfury Weapon strike (439440, 439441) spends a charge as a white swing
//! does, on the same charge cooldown, except in the instant another Windfury Weapon strike's crit
//! gained the charges. Combat logs of 23 November 2026, 36 level 60 Enhancement shamans in 35
//! reports: a Stormstrike that does not crit, with charges up and the cooldown ready, spends a
//! charge 160 of 173 times when Windfury Weapon strikes follow it, 45 of 1,507 times when no hit
//! does and 7 of 171 times when only Windfury Totem strikes do; a white or Stormstrike crit
//! followed by Windfury Weapon strikes alone ends on two charges 294 of 367 times, against three
//! 2,931 of 3,052 times when no hit follows; a second Windfury Weapon strike in the same
//! millisecond as one whose crit gained the charges leaves three 293 of 298 times. The engine ends
//! such a crit on two charges every time, against 80% in the logs. Without the rule the strike, a
//! special attack, spends none.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

/// Windfury Weapon's main hand and off hand strikes.
pub(crate) const WINDFURY_WEAPON_STRIKES: [i32; 2] = [439440, 439441];

/// What the handler needs to know of the hit besides its result.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hit {
    /// A melee auto attack.
    pub(crate) white: bool,
    /// A Windfury Weapon strike.
    pub(crate) imbue_strike: bool,
}

/// When Flurry last gained its charges, and whether a Windfury Weapon strike's crit gained them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Gain {
    at: i64,
    by_imbue_strike: bool,
}

impl Default for Gain {
    fn default() -> Self {
        Gain {
            at: i64::MIN,
            by_imbue_strike: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Flurry {
    pub(crate) trigger: AuraRef,
    pub(crate) aura: AuraRef,
    speed: f64,
    charge_icd: i64,
    can_proc_from_procs: bool,
    /// Forever's rules since the hotfixes of 17 November 2026: a Windfury Totem strike spends
    /// no charge.
    totem_strike_spends_none: bool,
    /// Forever's rules: a Windfury Weapon strike spends a charge.
    imbue_strike_spends: bool,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    trigger: &str,
    aura: &str,
    speed: f64,
    charge_icd: i64,
    can_proc_from_procs: bool,
    totem_strike_spends_none: bool,
    imbue_strike_spends: bool,
) -> Result<Flurry, String> {
    Ok(Flurry {
        trigger: fight.player_aura(trigger)?,
        aura: fight.player_aura(aura)?,
        speed,
        charge_icd,
        can_proc_from_procs,
        totem_strike_spends_none,
        imbue_strike_spends,
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

    /// The delayed handler. `icd` is the charge cooldown's ready time and `gain` the last gain
    /// of charges, which the handler may move.
    pub(crate) fn handle<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
        hit: Hit,
        icd: &mut i64,
        gain: &mut Gain,
    ) {
        if result.crit() {
            fight.activate_aura(self.aura);
            let max = fight.aura(self.aura).max_stacks;
            fight.set_stacks(self.aura, max);
            *gain = Gain {
                at: fight.now,
                by_imbue_strike: hit.imbue_strike,
            };
            return;
        }
        // A Windfury Totem strike carries the main hand auto's mask, so it is told apart from
        // the white swing by its spell.
        if self.totem_strike_spends_none && fight.windfury_strike_attack_power(spell).is_some() {
            return;
        }
        let spends = if hit.imbue_strike && self.imbue_strike_spends {
            !(gain.at == fight.now && gain.by_imbue_strike)
        } else {
            hit.white
        };
        if fight.aura(self.aura).active && spends && *icd <= fight.now {
            *icd = fight.now + self.charge_icd;
            fight.remove_stack(self.aura);
        }
    }
}
