//! Dark Sacrifice (1277324 to 1277328), from Go sim/priest/dark_sacrifice.go: a free self-only
//! periodic effect whose ticks each restore the client base plus Spirit over a divisor. The
//! cooldown manager uses it once the whole gain fits in the mana bar. Go reads Spirit at each
//! tick and check, which a stat aura can change.

use crate::core::fight::{Agent, Fight, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct DarkSacrifice {
    pub(crate) spell: SpellId,
    tick_base: f64,
    /// The prepared Spirit and its divisor.
    spirit: f64,
    spirit_divisor: f64,
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
    no_threat: bool,
) -> Result<DarkSacrifice, String> {
    let dot = fight.spells[spell]
        .dot
        .ok_or("Dark Sacrifice has no periodic effect")?;
    let ticks = f64::from(fight.dots[dot].base_tick_count);
    let metrics = fight.new_mana_metrics(metrics_action_id.clone());
    fight.resources[metrics].no_threat = no_threat;
    Ok(DarkSacrifice {
        spell,
        tick_base,
        spirit,
        spirit_divisor,
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

    /// Go's amount closure: the base plus Spirit over the divisor.
    fn amount<A: Agent>(&self, fight: &Fight<A>) -> f64 {
        self.tick_base + fight.player_spirit(self.spirit) / self.spirit_divisor
    }

    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>) {
        let amount = self.amount(fight);
        fight.add_mana(amount, self.metrics);
    }

    /// The major cooldown's `ShouldActivate`.
    pub(crate) fn should_activate<A: Agent>(&self, fight: &Fight<A>) -> bool {
        fight.player.powers.max_mana - fight.player.mana >= self.amount(fight) * self.ticks
    }
}
