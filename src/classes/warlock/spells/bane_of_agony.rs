//! Bane of Agony (11713), from Go sim/warlock/agony.go. Forever moved Curse of Agony onto
//! the bane slot, which it takes from Bane of Doom. The dot ramps: the snapshot pays a share of the tick, and every few
//! ticks that share is added back to the stored amount. Amplify Curse raises the whole tick,
//! the ramping base and the spell power share alike (Go `AmplifyCurseWholeTick`, patch 177):
//! the factor the snapshot spends is kept for the target and applied to each tick's damage.

use crate::{
    classes::warlock::agent::WarlockAgent,
    core::fight::{AuraRef, DotId, Fight, Outcome, Side, SpellId},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct BaneOfAgony {
    pub(crate) dot: DotId,
    tick_base: f64,
    ramp_share: f64,
    ramp_every_ticks: i32,
    /// Go's `rampStep`, the share of the tick the last snapshot stored.
    ramp_step: f64,
    /// Amplify Curse's aura and the factor its snapshot spends it for.
    pub(crate) amplify: Option<(AuraRef, f64)>,
}

impl BaneOfAgony {
    pub(crate) fn new(dot: DotId, tick_base: f64, ramp_share: f64, ramp_every_ticks: i32) -> Self {
        Self {
            dot,
            tick_base,
            ramp_share,
            ramp_every_ticks,
            ramp_step: 0.0,
            amplify: None,
        }
    }
}

fn state(fight: &Fight<WarlockAgent>) -> BaneOfAgony {
    fight.agent.bane_of_agony.expect("Bane of Agony is bound")
}

/// `ApplyEffects`: a hit roll without a hit counter, the dot when it lands, then the empty
/// outcome. `Dot.Apply` deactivates a running copy first, whose last tick may still be due
/// and reads the old ramp step; then `OnSnapshot` spends Amplify Curse while it is active,
/// keeping its factor for this target's ticks, stores the new step and snapshots it.
pub(crate) fn apply(fight: &mut Fight<WarlockAgent>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHitNoHitCounter);
    if result.landed() {
        let mut agony = state(fight);
        let dot = fight.dot_on(agony.dot, target);
        let aura = fight.dots[dot].aura;
        super::take_bane_slot(fight, aura);
        fight.deactivate_aura(aura);
        let base = agony.tick_base;
        let mut whole_tick_amp = 1.0;
        if let Some((amplify, factor)) = agony.amplify {
            if fight.aura(amplify).active {
                whole_tick_amp = factor;
                fight.deactivate_aura(amplify);
            }
        }
        let amps = &mut fight.agent.agony_whole_tick_amp;
        if amps.len() <= dot {
            amps.resize(dot + 1, 1.0);
        }
        amps[dot] = whole_tick_amp;
        agony.ramp_step = base * agony.ramp_share;
        fight.agent.bane_of_agony = Some(agony);
        fight.dots[dot].tick_base = Some(agony.ramp_step);
        fight.apply_dot(dot);
    }
    fight.deal_damage(spell, result, false);
}

/// `OnTick`: the snapshot tick raised by the Amplify Curse factor its snapshot spent, then every
/// `ramp_every_ticks` ticks the ramp step is added to the stored amount.
pub(crate) fn tick(fight: &mut Fight<WarlockAgent>, dot: DotId) {
    let agony = state(fight);
    let mut result = fight.snapshot_dot_tick_calc(dot);
    let amp = fight
        .agent
        .agony_whole_tick_amp
        .get(dot)
        .copied()
        .unwrap_or(1.0);
    let spell = fight.dots[dot].spell;
    fight.scale_damage(spell, &mut result, amp);
    fight.deal_damage(spell, result, true);
    // The ramp ends with the curse's own ticks: the extra ticks a Wrack channel adds (Forever's
    // rules since 11 November 2026) deal the amount reached and raise it no further.
    let ticks = fight.dots[dot].tick_count();
    if ticks % agony.ramp_every_ticks == 0 && ticks < fight.dots[dot].base_tick_count {
        fight.dots[dot].snapshot_base += agony.ramp_step;
    }
}
