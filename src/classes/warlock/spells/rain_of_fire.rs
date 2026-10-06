//! Rain of Fire (11678, tick 1282385), from Go sim/warlock/rain_of_fire.go. The cast itself
//! rolls a hit on every target, which deals no damage but is what a spell hit can proc. The
//! channel is an area dot on the warlock whose every tick casts the triggered tick spell,
//! which hits each target in turn for a fixed amount, rolled to hit and to crit. The tick
//! rows are procs, so only listeners that can proc from procs hear them.

use crate::core::fight::{Agent, Fight, Outcome, Side, SpellId};

use super::find_spell;

/// Rain of Fire's bound tick spell and amount.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RainOfFire {
    tick: SpellId,
    tick_base: f64,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    spell_id: i32,
    tick_spell_id: i32,
    tick_base: f64,
) -> Result<RainOfFire, String> {
    find_spell(fight, spell_id)?;
    Ok(RainOfFire {
        tick: find_spell(fight, tick_spell_id)?,
        tick_base,
    })
}

/// The channel cast: a hit roll on every target that deals no damage, as the cast's own
/// dummy lands on every enemy in the area, then `AOEDot().Apply`.
pub(crate) fn apply_channel<A: Agent>(fight: &mut Fight<A>, spell: SpellId) {
    for position in 0..fight.targets.len() {
        let result =
            fight.calc_outcome(spell, Side::target(position), Outcome::MagicHitNoHitCounter);
        fight.deal_damage(spell, result, false);
    }
    let dot = fight.spells[spell]
        .dot
        .expect("Rain of Fire has a channel dot");
    fight.apply_dot(dot);
}

impl RainOfFire {
    /// The channel's `OnTick`: cast the tick spell at the dot's unit, the warlock, so the cast
    /// counts against the warlock in the metrics. Its damage targets come from the encounter.
    pub(crate) fn on_channel_tick<A: Agent>(&self, fight: &mut Fight<A>, dot_side: Side) {
        fight.cast(self.tick, dot_side);
    }

    /// The tick spell's `ApplyEffects`: `CalcAndDealAoeDamage` with `OutcomeMagicHitAndCrit`.
    pub(crate) fn apply_tick<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        let tick_base = self.tick_base;
        fight.calc_and_deal_aoe_damage_with_variance(spell, |_| tick_base, Fight::calc_damage);
    }
}
