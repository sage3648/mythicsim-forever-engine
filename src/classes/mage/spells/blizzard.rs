//! Blizzard (10187, tick 1279949) with Improved Blizzard (12484), from Go
//! sim/mage/blizzard.go. The channel is an area dot on the mage whose every tick casts the
//! triggered tick spell. A tick deals a fixed amount to each target with
//! `OutcomeMagicHit`, so it never crits, and with Improved Blizzard each landed tick casts
//! the chill on its target, which Fingers of Frost can roll on. The tick hits each target
//! in turn, and the chills follow once every target has been hit.

use crate::core::fight::{Agent, Fight, Outcome, Side, SpellId};

/// Blizzard's bound spells and tick amount.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Blizzard {
    tick: SpellId,
    tick_base: f64,
    improved: Option<SpellId>,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    spell_id: i32,
    tick_spell_id: i32,
    tick_base: f64,
    improved_spell_id: Option<i32>,
) -> Result<Blizzard, String> {
    let find = |id: i32| {
        fight
            .spells
            .iter()
            .position(|spell| spell.id.spell_id == id && spell.id.tag == 0)
            .ok_or_else(|| format!("Blizzard spell {id} is not registered"))
    };
    find(spell_id)?;
    Ok(Blizzard {
        tick: find(tick_spell_id)?,
        tick_base,
        improved: improved_spell_id.map(find).transpose()?,
    })
}

/// The channel cast: `AOEDot().Apply`.
pub(crate) fn apply_channel<A: Agent>(fight: &mut Fight<A>, spell: SpellId) {
    let dot = fight.spells[spell].dot.expect("Blizzard has a channel dot");
    fight.apply_dot(dot);
}

impl Blizzard {
    /// The channel's `OnTick`: cast the tick spell at the dot's unit, the mage, so the cast
    /// counts against the mage in the metrics. Its damage targets come from the encounter.
    pub(crate) fn on_channel_tick<A: Agent>(&self, fight: &mut Fight<A>, dot_side: Side) {
        fight.cast(self.tick, dot_side);
    }

    /// The tick spell's `ApplyEffects`: `CalcAndDealAoeDamage` with `OutcomeMagicHit`, then
    /// the chill on each landed target.
    pub(crate) fn apply_tick<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        let tick_base = self.tick_base;
        let results = fight.calc_and_deal_aoe_damage_with_variance(
            spell,
            |_| tick_base,
            Fight::calc_damage_hit_only,
        );
        let Some(improved) = self.improved else {
            return;
        };
        for result in results.as_slice() {
            if result.landed() {
                fight.cast(improved, result.target);
            }
        }
    }
}

/// Improved Blizzard's `ApplyEffects`: `CalcAndDealOutcome` with
/// `OutcomeAlwaysHitNoHitCounter`.
pub(crate) fn apply_chill<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::AlwaysHitNoHitCounter);
    fight.deal_damage(spell, result, false);
}
