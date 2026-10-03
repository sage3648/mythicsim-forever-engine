//! Dark Sacrifice (1277324 to 1277328), from Go sim/priest/dark_sacrifice.go: a free self-only
//! periodic effect whose ticks each restore the client base plus Spirit over a divisor. The
//! cooldown manager uses it once the whole gain fits in the mana bar. Spirit is fixed in
//! scope: the gate rejects temporary changes to it.

use crate::core::fight::{Agent, Fight, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct DarkSacrifice {
    pub(crate) spell: SpellId,
    tick: f64,
    ticks: f64,
    metrics: usize,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    tick_base: f64,
    spirit: f64,
    spirit_divisor: f64,
    metrics_action_id: &crate::contracts::prepared_v2::ActionId,
) -> Result<DarkSacrifice, String> {
    let dot = fight.spells[spell]
        .dot
        .ok_or("Dark Sacrifice has no periodic effect")?;
    let ticks = f64::from(fight.dots[dot].base_tick_count);
    let metrics = fight.new_mana_metrics(metrics_action_id.clone());
    Ok(DarkSacrifice {
        spell,
        tick: tick_base + spirit / spirit_divisor,
        ticks,
        metrics,
    })
}

impl DarkSacrifice {
    /// The cast's `ApplyEffects`: the self-only effect starts.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        let dot = fight.spells[self.spell].dot.expect("bound with a dot");
        fight.apply_dot(dot);
    }

    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.add_mana(self.tick, self.metrics);
    }

    /// The major cooldown's `ShouldActivate`.
    pub(crate) fn should_activate<A: Agent>(&self, fight: &Fight<A>) -> bool {
        fight.config.max_mana - fight.player.mana >= self.tick * self.ticks
    }
}
