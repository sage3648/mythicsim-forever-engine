//! Bane of Agony (11713), from Go sim/warlock/agony.go. Forever moved Curse of Agony onto
//! the bane slot, which it takes from Bane of Doom. The dot ramps: the snapshot pays a share of the tick, and every few
//! ticks that share is added back to the stored amount.

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
/// and reads the old ramp step; then `OnSnapshot` raises the tick by Amplify Curse while it
/// is active, spending it, stores the new step and snapshots it.
pub(crate) fn apply(fight: &mut Fight<WarlockAgent>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHitNoHitCounter);
    if result.landed() {
        let mut agony = state(fight);
        let aura = fight.dots[agony.dot].aura;
        super::take_bane_slot(fight, aura);
        fight.deactivate_aura(aura);
        let mut base = agony.tick_base;
        if let Some((amplify, factor)) = agony.amplify {
            if fight.aura(amplify).active {
                base *= factor;
                fight.deactivate_aura(amplify);
            }
        }
        agony.ramp_step = base * agony.ramp_share;
        fight.agent.bane_of_agony = Some(agony);
        fight.dots[agony.dot].tick_base = Some(agony.ramp_step);
        fight.apply_dot(agony.dot);
    }
    fight.deal_damage(spell, result, false);
}

/// `OnTick`: the snapshot tick, then every `ramp_every_ticks` ticks the ramp step is added
/// to the stored amount.
pub(crate) fn tick(fight: &mut Fight<WarlockAgent>) {
    let agony = state(fight);
    fight.snapshot_dot_tick(agony.dot);
    if fight.dots[agony.dot].tick_count() % agony.ramp_every_ticks == 0 {
        fight.dots[agony.dot].snapshot_base += agony.ramp_step;
    }
}
