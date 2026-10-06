//! Rain of Fire (11678, tick 1282385), from Go sim/warlock/rain_of_fire.go. The channel is an
//! area dot on the warlock whose every tick casts the triggered tick spell, which hits each
//! target in turn for a fixed amount: rolled to hit and, unless the client row forbids it,
//! to crit.

use crate::core::fight::{Agent, Fight, Side, SpellId};

use super::find_spell;

/// Rain of Fire's bound tick spell and amount.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RainOfFire {
    tick: SpellId,
    tick_base: f64,
    tick_can_crit: bool,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    spell_id: i32,
    tick_spell_id: i32,
    tick_base: f64,
    tick_can_crit: bool,
) -> Result<RainOfFire, String> {
    find_spell(fight, spell_id)?;
    Ok(RainOfFire {
        tick: find_spell(fight, tick_spell_id)?,
        tick_base,
        tick_can_crit,
    })
}

/// The channel cast: `AOEDot().Apply`.
pub(crate) fn apply_channel<A: Agent>(fight: &mut Fight<A>, spell: SpellId) {
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

    /// The tick spell's `ApplyEffects`: `CalcAndDealAoeDamage`, hit and crit unless the row
    /// cannot crit.
    pub(crate) fn apply_tick<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        let tick_base = self.tick_base;
        if self.tick_can_crit {
            fight.calc_and_deal_aoe_damage_with_variance(spell, |_| tick_base, Fight::calc_damage);
        } else {
            fight.calc_and_deal_aoe_damage_with_variance(
                spell,
                |_| tick_base,
                Fight::calc_damage_hit_only,
            );
        }
    }
}
