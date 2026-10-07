//! Hurricane (17402, tick 1278759), from Go sim/druid/hurricane.go. The channel is an area
//! dot on the druid whose every tick casts the triggered tick spell. A tick deals a fixed
//! amount to each target in unit index order with `OutcomeMagicHitAndCrit`, calculating and
//! dealing each hit in turn: the tick row has no "can't crit" flag (community #688).

use crate::core::fight::{Fight, Side, SpellId};

use super::super::agent::DruidAgent;

/// Hurricane's bound tick spell and tick amount.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hurricane {
    tick: SpellId,
    tick_base: f64,
}

pub(crate) fn bind(
    fight: &Fight<DruidAgent>,
    spell_id: i32,
    tick_spell_id: i32,
    tick_base: f64,
) -> Result<Hurricane, String> {
    let find = |id: i32| {
        fight
            .spells
            .iter()
            .position(|spell| spell.id.spell_id == id && spell.id.tag == 0)
            .ok_or_else(|| format!("Hurricane spell {id} is not registered"))
    };
    find(spell_id)?;
    Ok(Hurricane {
        tick: find(tick_spell_id)?,
        tick_base,
    })
}

/// The channel cast: `AOEDot().Apply`.
pub(crate) fn apply_channel(fight: &mut Fight<DruidAgent>, spell: SpellId) {
    let dot = fight.spells[spell]
        .dot
        .expect("Hurricane has a channel dot");
    fight.apply_dot(dot);
}

impl Hurricane {
    /// The channel's `OnTick`: cast the tick spell at the dot's unit, the druid, so the cast
    /// counts against the druid in the metrics. Its damage targets come from the encounter.
    pub(crate) fn on_channel_tick(&self, fight: &mut Fight<DruidAgent>, dot_side: Side) {
        fight.cast(self.tick, dot_side);
    }

    /// The tick spell's `ApplyEffects`: `CalcAndDealAoeDamage` with `OutcomeMagicHitAndCrit`.
    pub(crate) fn apply_tick(&self, fight: &mut Fight<DruidAgent>, spell: SpellId) {
        let sides: Vec<Side> = fight.target_sides().collect();
        for side in sides {
            let result = fight.calc_damage(spell, side, self.tick_base);
            fight.deal_damage(spell, result, false);
        }
    }
}
