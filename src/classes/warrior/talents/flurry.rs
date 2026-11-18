//! Flurry (12319, buff 12966): a landed melee crit grants melee speed for a number of charges.
//! Under Forever's rules every white swing spends one whatever its outcome (hit, glance, miss,
//! dodge or parry), a Heroic Strike or Cleave spends one a cast whatever its outcome and however
//! many targets it hits (the hotfixes of 16 November 2026), and a crit of Whirlwind's off hand
//! strike starts it as any other strike's does. Go's `registerFlurry`,
//! which `--go-rules` keeps, spends one on each landed white hit, Heroic Strike and Cleave
//! included, and never starts it from Whirlwind's off hand strike.
//!
//! Evidence: three level 30 Forever logs on build 1.60.1.70450 with Flurry 1/5
//! (magey/forever-warrior discussion 30, counted in wowsims/forever PR 745). With Flurry up,
//! white misses spent a charge 21/21, 83/83 and 76/76, dodges 27/27, 28/28 and 20/20, parries
//! 25/25, 24/24 and 23/23; missed Heroic Strikes 9/9, 4/4 and 10/10; landed Heroic Strikes
//! 0/109 and 0/74 (and 3/82, each sharing its moment with a white swing), dodged or parried
//! ones 0/38. The logs hold one Cleave, so Cleave following Heroic Strike is an assumption.
//! Heroic Strike and Cleave since the hotfixes of 16 November 2026 (the buff's row now procs on
//! melee abilities, ProcFlags 0x14): public Forever reports on forever.warcraftlogs.com of 17
//! November (QDVfcwHpjZRzL94q, Wy6TdamAk4ZxJ8MC, 1g2QAZPJrLTjCz3m and six more, nine Fury
//! warriors) show a charge spent by 94 of 94 landed and 12 of 12 avoided Heroic Strikes and 372
//! of 372 Cleaves with Flurry up, and one charge by 327 of 328 Cleaves that hit two targets.
//! Combat logs of client 1.60.1.70888 hold 97 Whirlwinds of eight warriors: 16 of the 18 whose
//! off hand strike alone crit started Flurry.
//!
//! The party's Windfury Totem strike spends no charge since push 114115, though its crit starts
//! Flurry: a combat log of a level 60 dual wield Fury warrior in Upper Blackrock Spire on 16
//! November 2026 (client 1.60.1.70938, after the push) holds 74 strikes with Flurry up, 68 with
//! no charge removed, 4 whose removal a white swing or strike of the same moment explains, and 2
//! dodged or parried ones beside a refresh; the strike's crit started or refreshed Flurry 22 of
//! 22 times. Without the push's rule the strike, a main hand autoattack here, spends one as a
//! white swing does.

use crate::classes::warrior::agent::WarriorAgent;
use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult, OUTCOME_MISS};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Flurry {
    pub(crate) aura: AuraRef,
    pub(crate) melee_speed_multiplier: f64,
    pub(crate) charges: i32,
    /// Forever's rules (below); without them, as in Go, a landed white hit spends a charge.
    pub(crate) every_swing_spends: bool,
    /// Forever's rules: a crit of Whirlwind's off hand strike starts Flurry. Without them, as
    /// in Go, that strike never does.
    pub(crate) whirlwind_off_hand_starts: bool,
    /// Forever's rules since the hotfixes of 16 November 2026: a Heroic Strike or Cleave spends
    /// one charge a cast whatever its outcome. Without it, only a miss spends one.
    pub(crate) strikes_spend_once: bool,
    /// Forever's rules since push 114115: the Windfury Totem strike spends no charge.
    pub(crate) totem_strike_spends_none: bool,
}

/// The trigger's OnSpellHitDealt, which acts at once.
pub(crate) fn on_hit(
    fight: &mut Fight<WarriorAgent>,
    params: Flurry,
    spell: SpellId,
    result: &SpellResult,
) {
    let state = &fight.spells[spell];
    if state.flags.proc || !state.melee_proc || !(params.every_swing_spends || result.landed()) {
        return;
    }
    let class_spell = state.class_spell.as_deref();
    if class_spell == Some("whirlwind_off_hand") && !params.whirlwind_off_hand_starts {
        return;
    }
    let strike = matches!(class_spell, Some("heroic_strike" | "cleave"));
    let white_hit = state.white_hit;
    if params.strikes_spend_once && strike {
        // One charge a cast, before a crit starts Flurry again: a Cleave deals each target's
        // hit at the same moment, and only the first spends.
        let cast = Some((spell, fight.now));
        if fight.agent.flurry_strike_spent != cast {
            fight.agent.flurry_strike_spent = cast;
            if fight.aura(params.aura).active {
                fight.remove_stack(params.aura);
            }
        }
    }
    if result.crit() {
        fight.activate_aura(params.aura);
        fight.set_stacks(params.aura, params.charges);
        return;
    }
    if !fight.aura(params.aura).active {
        return;
    }
    // Heroic Strike and Cleave carry the main hand's auto mask too (ProcMask::MELEE_MH), so
    // they are told apart from the white swing by name.
    let spends =
        if params.totem_strike_spends_none && fight.windfury_strike_attack_power(spell).is_some() {
            false
        } else if !params.every_swing_spends {
            white_hit
        } else if strike {
            !params.strikes_spend_once && result.outcome & OUTCOME_MISS != 0
        } else {
            white_hit
        };
    if spends {
        fight.remove_stack(params.aura);
    }
}

/// Go `AttachMultiplyMeleeSpeed`: the gain multiplies, the expiry multiplies by the
/// reciprocal.
pub(crate) fn on_gain<A: Agent>(fight: &mut Fight<A>, params: Flurry) {
    fight.multiply_melee_speed(params.melee_speed_multiplier);
}

pub(crate) fn on_expire<A: Agent>(fight: &mut Fight<A>, params: Flurry) {
    fight.multiply_melee_speed(1.0 / params.melee_speed_multiplier);
}
